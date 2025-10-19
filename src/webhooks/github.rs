use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::webhooks::{pull_request, review, comment, push};

pub async fn handle_webhook(
    State((config, database)): State<(crate::config::AppConfig, crate::database::Database)>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
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
            pull_request::handle_pull_request_event(&database, &payload).await
        }
        "submitted" => {
            review::handle_review_event(&database, &payload).await
        }
        "created" => {
            comment::handle_comment_event(&database, &payload).await
        }
        _ => {
            warn!("Unhandled webhook event: {}", event_name);
            Ok(Json(serde_json::json!({"status": "ignored"})))
        }
    }
}




