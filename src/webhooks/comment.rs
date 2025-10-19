use serde_json::Value;
use tracing::{info, warn};

use crate::database::Database;

pub async fn handle_comment_event(
    database: &Database,
    payload: &Value,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    let repo_name = payload
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let pr_number = payload
        .get("issue")
        .and_then(|i| i.get("number"))
        .and_then(|n| n.as_u64())
        .unwrap_or(0);

    let commenter = payload
        .get("comment")
        .and_then(|c| c.get("user"))
        .and_then(|u| u.get("login"))
        .and_then(|l| l.as_str())
        .unwrap_or("unknown");

    let body = payload
        .get("comment")
        .and_then(|c| c.get("body"))
        .and_then(|b| b.as_str())
        .unwrap_or("");

    info!("Comment by {} on PR #{} in {}", commenter, pr_number, repo_name);

    // Check for governance signature commands
    if body.starts_with("/governance-sign") {
        let signature = body
            .strip_prefix("/governance-sign")
            .unwrap_or("")
            .trim();

        if !signature.is_empty() {
            info!("Processing governance signature from {}", commenter);
            
            match database.add_signature(repo_name, pr_number as i32, commenter, signature).await {
                Ok(_) => {
                    info!("Signature added for PR #{}", pr_number);
                    Ok(axum::response::Json(serde_json::json!({"status": "signature_added"})))
                }
                Err(e) => {
                    warn!("Failed to add signature: {}", e);
                    Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            warn!("Empty signature provided by {}", commenter);
            Ok(axum::response::Json(serde_json::json!({"status": "empty_signature"})))
        }
    } else {
        info!("Non-governance comment, ignoring");
        Ok(axum::response::Json(serde_json::json!({"status": "ignored"})))
    }
}




