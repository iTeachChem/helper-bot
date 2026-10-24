//! Turso over its HTTP pipeline API (no libsql client needed).

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context as _};
use serde_json::{json, Value as Json};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
}

impl Value {
    pub fn as_i64(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Float(f) => *f as i64,
            Value::Text(s) => s.parse().unwrap_or(0),
            Value::Null => 0,
        }
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Int(i) => *i as f64,
            Value::Float(f) => *f,
            Value::Text(s) => s.parse().unwrap_or(0.0),
            Value::Null => 0.0,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Value::Text(s) => s,
            _ => "",
        }
    }

    fn to_arg(&self) -> Json {
        match self {
            Value::Null => json!({"type": "null"}),
            Value::Int(i) => json!({"type": "integer", "value": i.to_string()}),
            Value::Float(f) => json!({"type": "float", "value": f}),
            Value::Text(s) => json!({"type": "text", "value": s}),
        }
    }

    fn from_cell(cell: &Json) -> Value {
        let v = &cell["value"];
        match cell["type"].as_str() {
            Some("integer") => v
                .as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_i64())
                .map_or(Value::Null, Value::Int),
            Some("float") => v
                .as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                .map_or(Value::Null, Value::Float),
            Some("text") | Some("blob") => v.as_str().map_or(Value::Null, |s| Value::Text(s.into())),
            _ => Value::Null,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.into())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

pub type Row = HashMap<String, Value>;

#[derive(Clone)]
pub struct Db {
    http: reqwest::Client,
    endpoint: String,
    token: String,
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS bot_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS user_stats (
        user_id              TEXT    PRIMARY KEY,
        username             TEXT    NOT NULL,
        doubts_solved        INTEGER NOT NULL DEFAULT 0,
        questions_attempted  INTEGER NOT NULL DEFAULT 0,
        questions_solved     INTEGER NOT NULL DEFAULT 0,
        questions_skipped    INTEGER NOT NULL DEFAULT 0,
        points               REAL    NOT NULL DEFAULT 0.0,
        total_time_taken     REAL    NOT NULL DEFAULT 0.0
    )",
    "CREATE INDEX IF NOT EXISTS idx_doubts_solved    ON user_stats (doubts_solved DESC)",
    "CREATE INDEX IF NOT EXISTS idx_questions_solved ON user_stats (questions_solved DESC)",
    "CREATE TABLE IF NOT EXISTS questions (
        question_id    INTEGER PRIMARY KEY AUTOINCREMENT,
        set_id         INTEGER,
        question_image TEXT,
        answer_text    TEXT,
        solution_image TEXT,
        subject        TEXT,
        topic          TEXT
    )",
];

impl Db {
    pub fn from_env(http: reqwest::Client) -> anyhow::Result<Self> {
        let url = std::env::var("TURSO_DATABASE_URL").context("TURSO_DATABASE_URL is not set")?;
        let token = std::env::var("TURSO_AUTH_TOKEN").context("TURSO_AUTH_TOKEN is not set")?;
        let url = url.replace("libsql://", "https://");
        Ok(Self { http, endpoint: format!("{}/v2/pipeline", url.trim_end_matches('/')), token })
    }

    async fn run(&self, stmts: Vec<Json>) -> anyhow::Result<Vec<Vec<Row>>> {
        let mut requests: Vec<Json> = stmts.into_iter().map(|s| json!({"type": "execute", "stmt": s})).collect();
        requests.push(json!({"type": "close"}));

        let resp = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .json(&json!({ "requests": requests }))
            .send()
            .await
            .map_err(|e| {
                tracing::error!("db: network error reaching Turso: {e}");
                e
            })?;
        let status = resp.status();
        if !status.is_success() {
            tracing::error!("db: HTTP {} from Turso", status.as_u16());
            bail!("Turso returned HTTP {status}");
        }
        let body: Json = resp.json().await?;

        let mut out = Vec::new();
        for r in body["results"].as_array().ok_or_else(|| anyhow!("Turso: missing results"))? {
            if r["type"] == "error" {
                let msg = r["error"]["message"].as_str().unwrap_or("unknown error");
                tracing::error!("db: Turso returned query error: {msg}");
                bail!("{msg}");
            }
            if r["response"]["type"] != "execute" {
                continue;
            }
            let res = &r["response"]["result"];
            let names: Vec<String> = res["cols"]
                .as_array()
                .map(|c| c.iter().map(|c| c["name"].as_str().unwrap_or_default().to_string()).collect())
                .unwrap_or_default();
            let rows = res["rows"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .map(|row| {
                            names
                                .iter()
                                .cloned()
                                .zip(row.as_array().into_iter().flatten().map(Value::from_cell))
                                .collect::<Row>()
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(rows);
        }
        Ok(out)
    }

    async fn exec(&self, sql: &str, params: &[Value]) -> anyhow::Result<Vec<Row>> {
        let stmt = json!({"sql": sql, "args": params.iter().map(Value::to_arg).collect::<Vec<_>>()});
        self.run(vec![stmt]).await?.into_iter().next().ok_or_else(|| anyhow!("Turso: empty result"))
    }

    pub async fn init(&self) -> anyhow::Result<()> {
        self.run(SCHEMA.iter().map(|sql| json!({ "sql": sql })).collect()).await?;
        tracing::info!("db: tables verified");
        Ok(())
    }

    pub async fn set_started_at(&self) -> anyhow::Result<()> {
        // same shape as python's datetime.isoformat(): 2026-01-01T00:00:00.000000+00:00
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false);
        self.exec(
            "INSERT INTO bot_meta (key, value) VALUES ('started_at', ?) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            &[now.as_str().into()],
        )
        .await?;
        tracing::info!("db: started_at set to {now}");
        Ok(())
    }

    pub async fn increment_doubts(&self, user_id: u64, username: &str, count: i64) -> anyhow::Result<i64> {
        let rows = self
            .exec(
                "INSERT INTO user_stats (user_id, username, doubts_solved)
                 VALUES (?, ?, ?)
                 ON CONFLICT (user_id) DO UPDATE SET
                     username      = excluded.username,
                     doubts_solved = user_stats.doubts_solved + ?
                 RETURNING doubts_solved",
                &[user_id.to_string().into(), username.into(), count.into(), count.into()],
            )
            .await?;
        Ok(rows.first().and_then(|r| r.get("doubts_solved")).map_or(0, Value::as_i64))
    }

    pub async fn get_user_with_ranks(&self, user_id: u64) -> anyhow::Result<Option<Row>> {
        let rows = self
            .exec(
                "WITH target AS (
                     SELECT * FROM user_stats WHERE user_id = ?
                 )
                 SELECT
                     t.*,
                     (SELECT COUNT(*) + 1 FROM user_stats
                      WHERE doubts_solved    > t.doubts_solved)    AS doubts_rank,
                     (SELECT COUNT(*) + 1 FROM user_stats
                      WHERE questions_solved > t.questions_solved) AS quiz_rank
                 FROM target t",
                &[user_id.to_string().into()],
            )
            .await?;
        Ok(rows.into_iter().next())
    }

    /// `column` must be one of the ranked columns; it is never user input.
    pub async fn get_leaderboard(&self, column: &'static str, excluded: &[String], limit: i64) -> anyhow::Result<Vec<Row>> {
        debug_assert!(matches!(column, "doubts_solved" | "questions_solved"));
        let placeholders = vec!["?"; excluded.len()].join(",");
        let sql = format!(
            "SELECT * FROM user_stats WHERE username NOT IN ({placeholders}) ORDER BY {column} DESC LIMIT ?"
        );
        let mut params: Vec<Value> = excluded.iter().map(|s| s.as_str().into()).collect();
        params.push(limit.into());
        self.exec(&sql, &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hrana_cells() {
        assert_eq!(Value::from_cell(&json!({"type": "integer", "value": "42"})), Value::Int(42));
        assert_eq!(Value::from_cell(&json!({"type": "float", "value": 1.5})), Value::Float(1.5));
        assert_eq!(Value::from_cell(&json!({"type": "text", "value": "hi"})), Value::Text("hi".into()));
        assert_eq!(Value::from_cell(&json!({"type": "null"})), Value::Null);
    }

    #[test]
    fn encodes_args() {
        assert_eq!(Value::Int(7).to_arg(), json!({"type": "integer", "value": "7"}));
        assert_eq!(Value::from("x").to_arg(), json!({"type": "text", "value": "x"}));
    }
}
