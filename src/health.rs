use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

async fn health() -> Json<Value> {
    Json(json!({ "status": "Working ok" }))
}

pub fn router() -> Router {
    // anything other than /health falls through to axum's 404
    Router::new().route("/health", get(health))
}

/// Port: HEALTH_PORT (old name), then PORT (Zerops template), then 8080.
pub async fn serve() {
    let port: u16 = std::env::var("HEALTH_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(8080);
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());

    let listener = match tokio::net::TcpListener::bind((host.as_str(), port)).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("health: could not bind {host}:{port}: {e}");
            return;
        }
    };
    tracing::info!("health server listening on port {port}");
    if let Err(e) = axum::serve(listener, router()).await {
        tracing::error!("health: server stopped: {e}");
    }
}
