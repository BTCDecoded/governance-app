use serde_json::Value;
use tracing::{info, warn};

use crate::database::Database;

pub async fn handle_push_event(
    database: &Database,
    payload: &Value,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    let repo_name = payload
        .get("repository")
        .and_then(|r| r.get("full_name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let pusher = payload
        .get("pusher")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    let ref_name = payload
        .get("ref")
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");

    info!("Push by {} to {} in {}", pusher, ref_name, repo_name);

    // Check if this is a direct push to main/master (potential bypass attempt)
    if ref_name == "refs/heads/main" || ref_name == "refs/heads/master" {
        warn!("Direct push to {} detected - potential governance bypass!", ref_name);
        
        // Log the bypass attempt
        match database.log_governance_event(
            "direct_push_detected",
            Some(repo_name),
            None,
            Some(pusher),
            &serde_json::json!({
                "ref": ref_name,
                "pusher": pusher,
                "timestamp": chrono::Utc::now()
            })
        ).await {
            Ok(_) => {
                info!("Bypass attempt logged for {}", repo_name);
                Ok(axum::response::Json(serde_json::json!({"status": "bypass_logged"})))
            }
            Err(e) => {
                warn!("Failed to log bypass attempt: {}", e);
                Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        info!("Regular push to {}, ignoring", ref_name);
        Ok(axum::response::Json(serde_json::json!({"status": "ignored"})))
    }
}




