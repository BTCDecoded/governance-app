use serde_json::Value;
use tracing::{info, warn};

use crate::database::Database;

pub async fn handle_pull_request_event(
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

    let head_sha = payload
        .get("pull_request")
        .and_then(|pr| pr.get("head").and_then(|h| h.get("sha")))
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    info!("Processing PR #{} in {}", pr_number, repo_name);

    // Determine layer based on repository
    let layer = match repo_name {
        repo if repo.contains("orange-paper") => 1,
        repo if repo.contains("consensus-proof") => 2,
        repo if repo.contains("protocol-engine") => 3,
        repo if repo.contains("reference-node") => 4,
        repo if repo.contains("developer-sdk") => 5,
        _ => {
            warn!("Unknown repository: {}", repo_name);
            return Ok(axum::response::Json(serde_json::json!({"status": "unknown_repo"})));
        }
    };

    // Store PR in database
    match database.create_pull_request(repo_name, pr_number as i32, head_sha, layer).await {
        Ok(_) => {
            info!("PR #{} stored in database", pr_number);
            Ok(axum::response::Json(serde_json::json!({"status": "stored"})))
        }
        Err(e) => {
            warn!("Failed to store PR: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}




