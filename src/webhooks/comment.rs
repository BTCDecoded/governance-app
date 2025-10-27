use serde_json::Value;
use tracing::{info, warn};

use crate::crypto::signatures::SignatureManager;
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

    info!(
        "Comment by {} on PR #{} in {}",
        commenter, pr_number, repo_name
    );

    // Check for governance signature commands
    if body.starts_with("/governance-sign") {
        let signature = body.strip_prefix("/governance-sign").unwrap_or("").trim();

        if !signature.is_empty() {
            info!("Processing governance signature from {}", commenter);

            // Get maintainer public key from database
            let maintainer = match database.get_maintainer_by_username(commenter).await {
                Ok(Some(maintainer)) => maintainer,
                Ok(None) => {
                    warn!("User {} is not a registered maintainer", commenter);
                    return Ok(axum::response::Json(
                        serde_json::json!({"status": "not_maintainer", "error": "User is not a registered maintainer"}),
                    ));
                }
                Err(e) => {
                    warn!("Failed to get maintainer info: {}", e);
                    return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            // Verify signature using developer-sdk
            let signature_manager = SignatureManager::new();
            let message = format!("PR #{} in {}", pr_number, repo_name);

            match signature_manager.verify_governance_signature(&message, signature, &maintainer.public_key) {
                Ok(true) => {
                    info!("Valid signature from {} for PR #{}", commenter, pr_number);
                    
                    // Store the verified signature
                    match database
                        .add_signature(repo_name, pr_number as i32, commenter, signature)
                        .await
                    {
                        Ok(_) => {
                            info!("Verified signature added for PR #{}", pr_number);

                            // Log governance event
                            let _ = database
                                .log_governance_event(
                                    "signature_collected",
                                    Some(repo_name),
                                    Some(pr_number as i32),
                                    Some(commenter),
                                    &serde_json::json!({
                                        "signature": signature,
                                        "message": message,
                                        "verified": true,
                                        "maintainer_layer": maintainer.layer
                                    }),
                                )
                                .await;

                            Ok(axum::response::Json(
                                serde_json::json!({"status": "signature_verified", "verified": true}),
                            ))
                        }
                        Err(e) => {
                            warn!("Failed to add verified signature: {}", e);
                            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
                Ok(false) => {
                    warn!("Invalid signature from {} for PR #{}", commenter, pr_number);
                    
                    // Log failed verification attempt
                    let _ = database
                        .log_governance_event(
                            "signature_verification_failed",
                            Some(repo_name),
                            Some(pr_number as i32),
                            Some(commenter),
                            &serde_json::json!({
                                "signature": signature,
                                "message": message,
                                "reason": "invalid_signature"
                            }),
                        )
                        .await;

                    Ok(axum::response::Json(
                        serde_json::json!({"status": "invalid_signature", "error": "Signature verification failed"}),
                    ))
                }
                Err(e) => {
                    warn!("Signature verification error: {}", e);
                    Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            warn!("Empty signature provided by {}", commenter);
            Ok(axum::response::Json(
                serde_json::json!({"status": "empty_signature"}),
            ))
        }
    } else {
        info!("Non-governance comment, ignoring");
        Ok(axum::response::Json(
            serde_json::json!({"status": "ignored"}),
        ))
    }
}
