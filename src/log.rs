//! stdout logging plus a Discord webhook sink for WARN and above.

use std::fmt::Write as _;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const MAX_LEN: usize = 1900;
const MIN_INTERVAL: Duration = Duration::from_secs(2);

struct WebhookLayer {
    tx: mpsc::UnboundedSender<String>,
}

#[derive(Default)]
struct MessageVisitor(String);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        } else {
            let _ = write!(self.0, " {}={value:?}", field.name());
        }
    }
}

fn split(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    chars.chunks(MAX_LEN).map(|c| c.iter().collect()).collect()
}

impl<S: Subscriber> Layer<S> for WebhookLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        // only our own warnings/errors; library noise (serenity, reqwest, ...) stays on stdout
        if *meta.level() > Level::WARN || !meta.target().starts_with("helper_bot") {
            return;
        }
        let mut msg = MessageVisitor::default();
        event.record(&mut msg);
        let line = format!(
            "{} [{}] {}: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            meta.level(),
            meta.target(),
            msg.0
        );
        for chunk in split(&line) {
            let _ = self.tx.send(format!("```\n{chunk}\n```"));
        }
    }
}

async fn worker(url: String, http: reqwest::Client, mut rx: mpsc::UnboundedReceiver<String>) {
    let mut last_sent: Option<Instant> = None;
    while let Some(payload) = rx.recv().await {
        loop {
            if let Some(t) = last_sent {
                tokio::time::sleep_until(t + MIN_INTERVAL).await;
            }
            let resp = http
                .post(&url)
                .json(&serde_json::json!({ "content": payload }))
                .timeout(Duration::from_secs(10))
                .send()
                .await;
            match resp {
                Ok(r) if r.status().as_u16() == 429 => {
                    let retry: f64 = r
                        .json::<serde_json::Value>()
                        .await
                        .ok()
                        .and_then(|j| j["retry_after"].as_f64())
                        .unwrap_or(MIN_INTERVAL.as_secs_f64());
                    tokio::time::sleep(Duration::from_secs_f64(retry.max(0.0))).await;
                    continue;
                }
                Ok(_) => last_sent = Some(Instant::now()),
                Err(_) => {}
            }
            break;
        }
    }
}

/// Must be called from inside the tokio runtime.
pub fn init(http: reqwest::Client) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stdout = tracing_subscriber::fmt::layer().with_target(true).with_ansi(false);

    let webhook = std::env::var("LOG_WEBHOOK").ok().filter(|u| !u.is_empty()).map(|url| {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(worker(url, http, rx));
        WebhookLayer { tx }.with_filter(tracing_subscriber::filter::LevelFilter::WARN)
    });
    let enabled = webhook.is_some();

    tracing_subscriber::registry().with(filter).with(stdout).with(webhook).init();
    if !enabled {
        tracing::warn!("log: LOG_WEBHOOK not set — webhook logging disabled");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_on_char_boundaries() {
        let s = "é".repeat(4000);
        let parts = super::split(&s);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].chars().count(), 1900);
    }
}
