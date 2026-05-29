import os
import logging
import httpx

logger = logging.getLogger(__name__)

_RANK_COLUMNS = frozenset({"doubts_solved", "questions_solved"})

_URL      = os.environ["TURSO_DATABASE_URL"].replace("libsql://", "https://")
_TOKEN    = os.environ["TURSO_AUTH_TOKEN"]
_ENDPOINT = f"{_URL}/v2/pipeline"
_HEADERS  = {
    "Authorization": f"Bearer {_TOKEN}",
    "Content-Type":  "application/json",
}


def _to_arg(v):
    if v is None:
        return {"type": "null"}
    if isinstance(v, bool):
        return {"type": "integer", "value": "1" if v else "0"}
    if isinstance(v, int):
        return {"type": "integer", "value": str(v)}
    if isinstance(v, float):
        return {"type": "real", "value": str(v)}
    return {"type": "text", "value": str(v)}


def _from_cell(cell):
    t, v = cell["type"], cell.get("value")
    if t == "null" or v is None:
        return None
    if t == "integer":
        return int(v)
    if t == "real":
        return float(v)
    return v


class _Result:
    def __init__(self, cols, rows):
        names = [c["name"] for c in cols]
        self.description = [(n,) for n in names]
        self._rows = [
            {name: _from_cell(cell) for name, cell in zip(names, row)}
            for row in rows
        ]

    def fetchone(self):
        return self._rows[0] if self._rows else None

    def fetchall(self):
        return self._rows


async def _run(*stmts):
    """Send one or more SQL statements in a single pipeline request."""
    requests_body = [
        {"type": "execute", "stmt": s} for s in stmts
    ] + [{"type": "close"}]

    async with httpx.AsyncClient() as client:
        resp = await client.post(_ENDPOINT, headers=_HEADERS, json={"requests": requests_body})
    resp.raise_for_status()

    results = resp.json()["results"]
    out = []
    for r in results:
        if r["type"] == "error":
            raise RuntimeError(r["error"]["message"])
        if r["response"]["type"] != "execute":
            continue
        res = r["response"]["result"]
        out.append(_Result(res["cols"], res["rows"]))
    return out


async def _exec(sql, params=()):
    stmt = {"sql": sql, "args": [_to_arg(p) for p in params]}
    return (await _run(stmt))[0]



async def init_db() -> None:
    await _run(
        {"sql": """
            CREATE TABLE IF NOT EXISTS bot_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
        """},
        {"sql": """
            CREATE TABLE IF NOT EXISTS user_stats (
                user_id              TEXT    PRIMARY KEY,
                username             TEXT    NOT NULL,
                doubts_solved        INTEGER NOT NULL DEFAULT 0,
                questions_attempted  INTEGER NOT NULL DEFAULT 0,
                questions_solved     INTEGER NOT NULL DEFAULT 0,
                questions_skipped    INTEGER NOT NULL DEFAULT 0,
                points               REAL    NOT NULL DEFAULT 0.0,
                total_time_taken     REAL             NOT NULL DEFAULT 0.0
            )
        """},
        {"sql": "CREATE INDEX IF NOT EXISTS idx_doubts_solved    ON user_stats (doubts_solved DESC)"},
        {"sql": "CREATE INDEX IF NOT EXISTS idx_questions_solved ON user_stats (questions_solved DESC)"},
        {"sql": """
            CREATE TABLE IF NOT EXISTS questions (
                question_id    INTEGER PRIMARY KEY AUTOINCREMENT,
                set_id         INTEGER,
                question_image TEXT,
                answer_text    TEXT,
                solution_image TEXT,
                subject        TEXT,
                topic          TEXT
            )
        """},
    )
    logger.info("db: tables verified")


async def set_started_at() -> None:
    from datetime import datetime, timezone
    now = datetime.now(timezone.utc).isoformat()
    await _exec(
        "INSERT INTO bot_meta (key, value) VALUES ('started_at', ?) "
        "ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        (now,),
    )


async def increment_doubts(user_id: int, username: str, count: int = 1) -> int:
    row = (await _exec("""
        INSERT INTO user_stats (user_id, username, doubts_solved)
        VALUES (?, ?, ?)
        ON CONFLICT (user_id) DO UPDATE SET
            username      = excluded.username,
            doubts_solved = user_stats.doubts_solved + ?
        RETURNING doubts_solved
    """, (str(user_id), username, count, count))).fetchone()
    return row["doubts_solved"]


async def get_user(user_id: int) -> dict | None:
    return (await _exec(
        "SELECT * FROM user_stats WHERE user_id = ?", (str(user_id),)
    )).fetchone()


async def get_user_with_ranks(user_id: int) -> dict | None:
    """Fetch a user row together with both leaderboard ranks in one round-trip."""
    row = (await _exec("""
        WITH target AS (
            SELECT * FROM user_stats WHERE user_id = ?
        )
        SELECT
            t.*,
            (SELECT COUNT(*) + 1 FROM user_stats
             WHERE doubts_solved    > t.doubts_solved)    AS doubts_rank,
            (SELECT COUNT(*) + 1 FROM user_stats
             WHERE questions_solved > t.questions_solved) AS quiz_rank
        FROM target t
    """, (str(user_id),))).fetchone()
    return row


async def get_leaderboard_doubts(excluded: tuple[str, ...], limit: int = 10) -> list[dict]:
    placeholders = ",".join("?" * len(excluded))
    return (await _exec(
        f"SELECT * FROM user_stats WHERE username NOT IN ({placeholders}) ORDER BY doubts_solved DESC LIMIT ?",
        (*excluded, limit),
    )).fetchall()


async def get_leaderboard_quiz(excluded: tuple[str, ...], limit: int = 10) -> list[dict]:
    placeholders = ",".join("?" * len(excluded))
    return (await _exec(
        f"SELECT * FROM user_stats WHERE username NOT IN ({placeholders}) ORDER BY questions_solved DESC LIMIT ?",
        (*excluded, limit),
    )).fetchall()
