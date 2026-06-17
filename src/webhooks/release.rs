//! Release event webhook handler

use axum::http::StatusCode;
use serde_json::Value;
use tracing::{error, info, warn};

use crate::build::orchestrator::BuildOrchestrator;
use crate::database::Database;
use crate::error::GovernanceError;

/// Handle release webhook events
pub async fn handle_release_event(
    payload: &Value,
    orchestrator: &BuildOrchestrator,
) -> Result<(StatusCode, Value), GovernanceError> {
    info!("Handling release event");

    // Extract release information
    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let release = payload
        .get("release")
        .ok_or_else(|| GovernanceError::GitHubError("Missing release in payload".to_string()))?;

    let tag_name = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| GovernanceError::GitHubError("Missing tag_name in release".to_string()))?;

    let prerelease = release
        .get("prerelease")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    info!(
        "Release event: action={}, tag={}, prerelease={}",
        action, tag_name, prerelease
    );

    // Only handle published releases
    if action != "published" {
        info!("Ignoring release event with action: {}", action);
        return Ok((
            StatusCode::OK,
            serde_json::json!({"status": "ignored", "reason": format!("Action {} not handled", action)}),
        ));
    }

    // Trigger build orchestration
    match orchestrator
        .handle_release_event(tag_name, prerelease)
        .await
    {
        Ok(_) => {
            info!("Successfully orchestrated builds for release {}", tag_name);
            Ok((
                StatusCode::OK,
                serde_json::json!({
                    "status": "success",
                    "message": format!("Build orchestration started for {}", tag_name),
                }),
            ))
        }
        Err(e) => {
            error!(
                "Failed to orchestrate builds for release {}: {}",
                tag_name, e
            );
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to orchestrate builds: {}", e),
                }),
            ))
        }
    }
}

/// Handle repository_dispatch events (build completion notifications)
pub async fn handle_repository_dispatch(
    payload: &Value,
    orchestrator: &BuildOrchestrator,
    database: &Database,
) -> Result<(StatusCode, Value), GovernanceError> {
    info!("Handling repository_dispatch event");

    let action = payload
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let client_payload = payload
        .get("client_payload")
        .and_then(|v| v.as_object())
        .ok_or_else(|| GovernanceError::GitHubError("Missing client_payload".to_string()))?;

    info!(
        "Repository dispatch: action={}, payload={:?}",
        action, client_payload
    );

    // Handle different dispatch event types
    match action {
        "build-complete" => {
            let repo = client_payload
                .get("repo")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = client_payload
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let release_version = client_payload
                .get("release_version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let workflow_run_id = client_payload
                .get("workflow_run_id")
                .and_then(|v| v.as_u64());
            let error_message = client_payload.get("error_message").and_then(|v| v.as_str());

            info!(
                "Build completed for {} (release {}): {}",
                repo, release_version, status
            );

            // Map GitHub status to our build status
            let build_status = match status {
                "success" => "success",
                "failure" => "failure",
                "cancelled" => "cancelled",
                "timed_out" => "timed_out",
                _ => "failure", // Unknown status treated as failure
            };

            // Update build state in database
            database
                .update_build_status(release_version, repo, build_status, error_message)
                .await?;

            // Check if all builds are complete
            let all_complete = database.are_all_builds_complete(release_version).await?;

            if all_complete {
                info!("All builds complete for release {}", release_version);

                // Proceed to next step: artifact collection and release creation
                // Check if any builds failed - if so, don't proceed with release
                let all_successful = database
                    .get_build_runs_for_release(release_version)
                    .await?
                    .iter()
                    .all(|run| run.1 == "success"); // Access tuple field by index

                if all_successful {
                    info!(
                        "All builds successful for release {} - proceeding with artifact collection",
                        release_version
                    );

                    // Trigger artifact collection and release creation
                    // This is done asynchronously to avoid blocking the webhook response
                    let orchestrator_clone = orchestrator.clone();
                    let release_version_clone = release_version.to_string();

                    tokio::spawn(async move {
                        if let Err(e) = orchestrator_clone
                            .collect_and_create_release(&release_version_clone)
                            .await
                        {
                            error!(
                                "Failed to collect artifacts and create release for {}: {}",
                                release_version_clone, e
                            );
                        } else {
                            info!(
                                "Successfully collected artifacts and created release for {}",
                                release_version_clone
                            );
                        }
                    });
                } else {
                    warn!(
                        "Some builds failed for release {} - skipping artifact collection and release creation",
                        release_version
                    );
                }
            } else {
                info!(
                    "Waiting for remaining builds for release {}",
                    release_version
                );
            }
        }
        "build-request" => {
            // This is handled by the workflow, not the governance app
            info!("Build request received (handled by workflow)");
        }
        _ => {
            warn!("Unknown repository_dispatch action: {}", action);
        }
    }

    Ok((StatusCode::OK, serde_json::json!({"status": "received"})))
}

/// Check if release action should be handled
pub fn should_handle_release_action(action: &str) -> bool {
    action == "published"
}

/// Map GitHub build status to internal build status
pub fn map_build_status(status: &str) -> &str {
    match status {
        "success" => "success",
        "failure" => "failure",
        "cancelled" => "cancelled",
        "timed_out" => "timed_out",
        _ => "failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_handle_release_action_published() {
        assert!(should_handle_release_action("published"));
    }

    #[test]
    fn test_should_handle_release_action_other() {
        assert!(!should_handle_release_action("created"));
        assert!(!should_handle_release_action("edited"));
        assert!(!should_handle_release_action("deleted"));
    }

    #[test]
    fn test_map_build_status() {
        assert_eq!(map_build_status("success"), "success");
        assert_eq!(map_build_status("failure"), "failure");
        assert_eq!(map_build_status("cancelled"), "cancelled");
        assert_eq!(map_build_status("timed_out"), "timed_out");
        assert_eq!(map_build_status("unknown"), "failure");
    }
}
