use serde_json::Value;
use tracing::{error, info, warn};

use crate::crypto::signatures::SignatureManager;
use crate::database::Database;
use crate::governance_review::GovernanceReviewCaseManager;
use crate::governance_review::models::policy;

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

    // Check for tier override command
    if body.starts_with("/governance-tier-override") {
        return handle_tier_override(database, repo_name, pr_number, commenter, body).await;
    }

    // Check for governance review case creation
    if body.starts_with("/governance-review-case") {
        return handle_governance_review_case(database, commenter, body).await;
    }

    // Check for governance signature commands
    if body.starts_with("/governance-sign") {
        let remainder = body.strip_prefix("/governance-sign").unwrap_or("").trim();

        // Parse signature and optional reasoning
        // Format: /governance-sign <signature> "reasoning" or /governance-sign <signature>
        let (signature, reasoning) = if remainder.contains('"') {
            // Extract signature (before first quote) and reasoning (between quotes)
            if let Some(quote_start) = remainder.find('"') {
                let sig = remainder[..quote_start].trim();
                // Find the closing quote
                let after_quote = &remainder[quote_start + 1..];
                if let Some(quote_end) = after_quote.find('"') {
                    let reason = &after_quote[..quote_end];
                    (sig, Some(reason))
                } else {
                    // Unmatched quote - treat as signature only
                    (remainder.trim(), None)
                }
            } else {
                (remainder.trim(), None)
            }
        } else {
            (remainder.trim(), None)
        };

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

            // Verify signature using blvm-sdk
            let signature_manager = SignatureManager::new();
            let message = format!("PR #{pr_number} in {repo_name}");

            match signature_manager.verify_governance_signature(
                &message,
                signature,
                &maintainer.public_key,
            ) {
                Ok(true) => {
                    info!("Valid signature from {} for PR #{}", commenter, pr_number);

                    // Check if maintainer has reviewed this PR (transparency only, not blocking)
                    let has_reviewed = database
                        .has_maintainer_reviewed(repo_name, pr_number as i32, commenter)
                        .await
                        .unwrap_or(false);

                    if !has_reviewed {
                        warn!(
                            "Maintainer {} signed PR #{} without GitHub review (transparency only, not blocking)",
                            commenter, pr_number
                        );
                    }

                    // Store the verified signature with reasoning
                    match database
                        .add_signature(repo_name, pr_number as i32, commenter, signature, reasoning)
                        .await
                    {
                        Ok(_) => {
                            info!("Verified signature added for PR #{}", pr_number);

                            // Log whether signature has review link
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
                                        "maintainer_layer": maintainer.layer,
                                        "reasoning": reasoning,
                                        "has_review": has_reviewed
                                    }),
                                )
                                .await;

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
                                        "maintainer_layer": maintainer.layer,
                                        "reasoning": reasoning
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
    } else if body.starts_with("/governance-challenge") {
        return handle_challenge_command(database, repo_name, pr_number, commenter, body).await;
    } else {
        info!("Non-governance comment, ignoring");
        Ok(axum::response::Json(
            serde_json::json!({"status": "ignored"}),
        ))
    }
}

/// Handle challenge command: /governance-challenge <target_type> <target_id> <reason> <signature>
async fn handle_challenge_command(
    database: &Database,
    repo_name: &str,
    pr_number: u64,
    commenter: &str,
    body: &str,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    use crate::governance::challenge::{ChallengeManager, ChallengeTarget};
    use tracing::{info, warn};

    // Parse command: /governance-challenge <target_type> <target_id> "reason" <signature>
    let remainder = body
        .strip_prefix("/governance-challenge")
        .unwrap_or("")
        .trim();

    // Parse target_type and target_id (first two words)
    let parts: Vec<&str> = remainder.split_whitespace().collect();
    if parts.len() < 4 {
        warn!(
            "Invalid challenge format. Expected: /governance-challenge <target_type> <target_id> \"reason\" <signature>"
        );
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Invalid format. Use: /governance-challenge <target_type> <target_id> \"reason\" <signature>"}),
        ));
    }

    let target_type_str = parts[0];
    let target_id = parts[1].to_string();

    // Parse reason (quoted string)
    let reason_start = remainder.find('"');
    let reason_end = if let Some(start) = reason_start {
        remainder[start + 1..].find('"').map(|end| start + 1 + end)
    } else {
        None
    };

    let reason = if let (Some(start), Some(end)) = (reason_start, reason_end) {
        &remainder[start + 1..end]
    } else {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Reason must be in quotes"}),
        ));
    };

    // Parse signature (after reason)
    let signature = &remainder[reason_end.unwrap() + 1..].trim();
    if signature.is_empty() {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Signature is required"}),
        ));
    }

    // Parse target type
    let target_type = match ChallengeTarget::from_str(target_type_str) {
        Ok(t) => t,
        Err(e) => {
            warn!("Invalid challenge target type: {}", target_type_str);
            return Ok(axum::response::Json(
                serde_json::json!({"status": "error", "error": format!("Invalid target type: {}", e)}),
            ));
        }
    };

    // Get database pool
    let pool = database
        .get_sqlite_pool()
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let challenge_manager = ChallengeManager::new(pool.clone());

    // Create challenge
    match challenge_manager
        .create_challenge(
            target_type,
            target_id.clone(),
            commenter.to_string(),
            reason.to_string(),
            signature.to_string(),
        )
        .await
    {
        Ok(challenge_id) => {
            info!(
                "Challenge {} created by {} for {} {}",
                challenge_id, commenter, target_type_str, target_id
            );

            // Log governance event
            let _ = database
                .log_governance_event(
                    "challenge_created",
                    Some(repo_name),
                    Some(pr_number as i32),
                    Some(commenter),
                    &serde_json::json!({
                        "challenge_id": challenge_id,
                        "target_type": target_type_str,
                        "target_id": target_id,
                        "reason": reason
                    }),
                )
                .await;

            Ok(axum::response::Json(serde_json::json!({
                "status": "challenge_created",
                "challenge_id": challenge_id,
                "message": format!("Challenge {} created. Response required within 30 days.", challenge_id)
            })))
        }
        Err(e) => {
            warn!("Failed to create challenge: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle tier override command: /governance-tier-override <tier> "justification"
async fn handle_tier_override(
    database: &Database,
    repo_name: &str,
    pr_number: u64,
    commenter: &str,
    body: &str,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    use tracing::{info, warn};

    // Parse command: /governance-tier-override <tier> "justification"
    let remainder = body
        .strip_prefix("/governance-tier-override")
        .unwrap_or("")
        .trim();

    // Extract tier number and justification
    let parts: Vec<&str> = remainder.splitn(2, '"').collect();
    if parts.len() < 2 {
        warn!(
            "Invalid tier override format. Expected: /governance-tier-override <tier> \"justification\""
        );
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Invalid format. Use: /governance-tier-override <tier> \"justification\""}),
        ));
    }

    let tier_str = parts[0].trim();
    let justification = parts[1].trim_matches('"').trim();

    if justification.is_empty() {
        warn!("Empty justification provided for tier override");
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Justification is required"}),
        ));
    }

    let override_tier: u32 = match tier_str.parse() {
        Ok(t) if (1..=5).contains(&t) => t,
        _ => {
            warn!("Invalid tier number: {}", tier_str);
            return Ok(axum::response::Json(
                serde_json::json!({"status": "error", "error": "Tier must be between 1 and 5"}),
            ));
        }
    };

    // Check if user is a maintainer
    let maintainer = match database.get_maintainer_by_username(commenter).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            warn!("User {} is not a registered maintainer", commenter);
            return Ok(axum::response::Json(
                serde_json::json!({"status": "not_maintainer", "error": "Only maintainers can override tiers"}),
            ));
        }
        Err(e) => {
            warn!("Failed to get maintainer info: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Store tier override
    match database
        .set_tier_override(
            repo_name,
            pr_number as i32,
            override_tier,
            justification,
            commenter,
        )
        .await
    {
        Ok(_) => {
            info!(
                "Tier override set to {} for PR #{} by {}",
                override_tier, pr_number, commenter
            );

            // Log governance event
            let _ = database
                .log_governance_event(
                    "tier_override",
                    Some(repo_name),
                    Some(pr_number as i32),
                    Some(commenter),
                    &serde_json::json!({
                        "override_tier": override_tier,
                        "justification": justification,
                        "maintainer_layer": maintainer.layer
                    }),
                )
                .await;

            Ok(axum::response::Json(serde_json::json!({
                "status": "tier_override_set",
                "override_tier": override_tier,
                "justification": justification
            })))
        }
        Err(e) => {
            warn!("Failed to set tier override: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_governance_review_case(
    database: &Database,
    commenter: &str,
    body: &str,
) -> Result<axum::response::Json<serde_json::Value>, axum::http::StatusCode> {
    // Verify commenter is a maintainer
    let reporter = match database.get_maintainer_by_username(commenter).await {
        Ok(Some(maintainer)) => maintainer,
        Ok(None) => {
            warn!("User {} is not a registered maintainer", commenter);
            return Ok(axum::response::Json(
                serde_json::json!({"status": "error", "error": "User is not a registered maintainer"}),
            ));
        }
        Err(e) => {
            error!("Failed to get maintainer info: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Parse command: /governance-review-case @subject case_type severity "description" [evidence_json]
    let remainder = body
        .strip_prefix("/governance-review-case")
        .unwrap_or("")
        .trim();

    if remainder.is_empty() {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Missing required parameters. Format: /governance-review-case @subject case_type severity \"description\" [evidence_json]"}),
        ));
    }

    // Parse subject (must start with @)
    let parts: Vec<&str> = remainder.split_whitespace().collect();
    if parts.is_empty() || !parts[0].starts_with('@') {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Subject must be specified as @username"}),
        ));
    }

    let subject_username = &parts[0][1..]; // Remove @

    // Get subject maintainer
    let subject = match database.get_maintainer_by_username(subject_username).await {
        Ok(Some(maintainer)) => maintainer,
        Ok(None) => {
            return Ok(axum::response::Json(
                serde_json::json!({"status": "error", "error": format!("Subject @{} is not a registered maintainer", subject_username)}),
            ));
        }
        Err(e) => {
            error!("Failed to get subject maintainer: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Parse case_type and severity
    if parts.len() < 4 {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Missing parameters. Format: /governance-review-case @subject case_type severity \"description\""}),
        ));
    }

    let case_type = parts[1];
    let severity = parts[2];

    // Validate case_type and severity
    if !policy::CASE_TYPES.contains(&case_type) {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": format!("Invalid case_type: {}. Valid types: {:?}", case_type, policy::CASE_TYPES)}),
        ));
    }

    if !policy::SEVERITY_LEVELS.contains(&severity) {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": format!("Invalid severity: {}. Valid levels: {:?}", severity, policy::SEVERITY_LEVELS)}),
        ));
    }

    // Parse description (quoted string)
    let description_start = remainder.find('"');
    let description_end = if let Some(start) = description_start {
        remainder[start + 1..].find('"').map(|end| start + 1 + end)
    } else {
        None
    };

    let description = if let (Some(start), Some(end)) = (description_start, description_end) {
        &remainder[start + 1..end]
    } else {
        return Ok(axum::response::Json(
            serde_json::json!({"status": "error", "error": "Description must be in quotes"}),
        ));
    };

    // Parse evidence (optional JSON after description)
    let evidence_str = &remainder[description_end.unwrap() + 1..].trim();
    let evidence = if evidence_str.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(evidence_str).unwrap_or_else(|_| serde_json::json!({}))
    };

    // Create case (on-platform only per policy)
    let pool = database
        .get_sqlite_pool()
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let case_manager = GovernanceReviewCaseManager::new(pool.clone());

    match case_manager
        .create_case(
            subject.id,
            reporter.id,
            case_type,
            severity,
            description,
            evidence,
            true, // on-platform only
        )
        .await
    {
        Ok(case) => {
            info!(
                "Created governance review case {} by {} for {}",
                case.case_number, commenter, subject_username
            );
            Ok(axum::response::Json(serde_json::json!({
                "status": "ok",
                "case_number": case.case_number,
                "message": format!("Governance review case {} created", case.case_number)
            })))
        }
        Err(e) => {
            error!("Failed to create governance review case: {}", e);
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
