use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::webhooks::{pull_request, review, comment};

pub async fn handle_webhook(
    State((_config, database)): State<(crate::config::AppConfig, crate::database::Database)>,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let event_type = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let event_name = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    info!("Received webhook: {} - {}", event_name, event_type);

    match event_name {
        "opened" | "synchronize" | "reopened" => {
            match pull_request::handle_pull_request_event(&database, &payload).await {
                Ok(response) => (StatusCode::OK, response),
                Err(status) => (status, Json(serde_json::json!({"error": "failed"})))
            }
        }
        "submitted" => {
            match review::handle_review_event(&database, &payload).await {
                Ok(response) => (StatusCode::OK, response),
                Err(status) => (status, Json(serde_json::json!({"error": "failed"})))
            }
        }
        "created" => {
            match comment::handle_comment_event(&database, &payload).await {
                Ok(response) => (StatusCode::OK, response),
                Err(status) => (status, Json(serde_json::json!({"error": "failed"})))
            }
        }
        _ => {
            warn!("Unhandled webhook event: {}", event_name);
            (StatusCode::OK, Json(serde_json::json!({"status": "ignored"})))
        }
    }
}




