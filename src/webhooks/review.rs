use serde_json::Value;
use tracing::{info, warn};

use crate::database::Database;

pub async fn handle_review_event(
    database: &Database,
    payload: &Value,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    let repo_name = payload
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let pr_number = payload
        .get("pull_request")
        .and_then(|pr| pr.get("number"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let reviewer = payload
        .get("review")
        .and_then(|r| r.get("user"))
        .and_then(|u| u.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or("unknown");

    let state = payload
        .get("review")
        .and_then(|r| r.get("state"))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    info!(
        "Review {} by {} for PR #{} in {}",
        state, reviewer, pr_number, repo_name
    );

    // Update review status in database
    match database
        .update_review_status(repo_name, pr_number as i32, reviewer, state)
        .await
    {
        Ok(_) => {
            info!("Review status updated for PR #{}", pr_number);
            Ok(axum::response::Json(
                serde_json::json!({"status": "updated"}),
            ))
        }
        Err(e) => {
            warn!("Failed to update review status: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
