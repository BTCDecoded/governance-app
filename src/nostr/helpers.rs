//! Nostr Integration Helpers
//!
//! Helper functions for publishing governance events to Nostr

use anyhow::Result;

use crate::config::AppConfig;
use crate::database::Database;
use crate::nostr::{
    CombinedRequirement, GovernanceActionPublisher, KeyholderSignature, LayerRequirement,
    NostrClient, TierRequirement,
};
use crate::validation::threshold::ThresholdValidator;

/// Publish governance action event when PR is merged
pub async fn publish_merge_action(
    config: &AppConfig,
    database: &Database,
    repository: &str,
    pr_number: i32,
    commit_hash: &str,
    layer: i32,
    tier: u32,
) -> Result<()> {
    if !config.nostr.enabled {
        return Ok(()); // Nostr disabled, skip
    }

    // Get PR info from database
    let pr_info = database.get_pull_request(repository, pr_number).await?;
    let pr = match pr_info {
        Some(p) => p,
        None => {
            tracing::warn!(
                "PR #{} not found in database, skipping Nostr publish",
                pr_number
            );
            return Ok(());
        }
    };

    // Get layer and tier requirements
    let (layer_sigs_req, layer_sigs_total) = ThresholdValidator::get_threshold_for_layer(layer);
    let layer_review = ThresholdValidator::get_review_period_for_layer(layer, false);

    let (tier_sigs_req, tier_sigs_total) = ThresholdValidator::get_tier_threshold(tier);
    let tier_review = ThresholdValidator::get_tier_review_period(tier);

    // Get combined requirements
    let (final_sigs_req, final_sigs_total, final_review_days) =
        ThresholdValidator::get_combined_requirements(layer, tier);
    let source = ThresholdValidator::get_requirement_source(layer, tier);

    // Create requirement structs
    let layer_req = LayerRequirement {
        layer: layer as u32,
        signatures: format!("{layer_sigs_req}-of-{layer_sigs_total}"),
        review_days: layer_review as u32,
    };

    let tier_req = TierRequirement {
        tier,
        signatures: format!("{tier_sigs_req}-of-{tier_sigs_total}"),
        review_days: tier_review as u32,
    };

    let combined_req = CombinedRequirement {
        signatures: format!("{final_sigs_req}-of-{final_sigs_total}"),
        review_days: final_review_days as u32,
        source: source.clone(),
    };

    // Get signatures from database
    let signatures = get_signatures_from_db(database, repository, pr_number).await?;

    // Create Nostr client and publisher
    let nsec = std::fs::read_to_string(&config.nostr.server_nsec_path)
        .map_err(|e| anyhow::anyhow!("Failed to read Nostr key: {}", e))?;

    let client = NostrClient::new(nsec, config.nostr.relays.clone()).await?;
    let publisher = GovernanceActionPublisher::new(
        client,
        config.nostr.governance_config.clone(),
        config.nostr.zap_address.clone(),
    );

    // Publish action
    publisher
        .publish_action(
            "merge",
            tier,
            layer as u32,
            repository,
            &combined_req.signatures,
            combined_req.review_days,
            Some(commit_hash),
            Some(pr_number),
            &format!("Merge PR #{}: {}", pr_number, pr.head_sha),
            layer_req,
            tier_req,
            combined_req.clone(),
            signatures,
            None, // review_period_ends - already merged
        )
        .await?;

    Ok(())
}

/// Get signatures from database for a PR
async fn get_signatures_from_db(
    database: &Database,
    repository: &str,
    pr_number: i32,
) -> Result<Vec<KeyholderSignature>> {
    // Get PR to access signatures
    let pr = database.get_pull_request(repository, pr_number).await?;

    match pr {
        Some(p) => {
            // Convert database signatures to Nostr format
            let mut nostr_sigs = Vec::new();
            for sig in &p.signatures {
                // Determine keyholder type from maintainer registry
                let keyholder_type = determine_keyholder_type(database, &sig.signer)
                    .await
                    .unwrap_or_else(|_| "maintainer".to_string()); // Default to maintainer on error

                nostr_sigs.push(KeyholderSignature {
                    keyholder: sig.signer.clone(),
                    keyholder_type,
                    signature: sig.signature.clone(),
                    timestamp: sig.timestamp.timestamp(),
                });
            }
            Ok(nostr_sigs)
        }
        None => Ok(vec![]),
    }
}

/// Determine keyholder type from maintainer registry
/// Checks if signer is a maintainer or emergency keyholder
async fn determine_keyholder_type(
    database: &Database,
    signer: &str,
) -> Result<String, anyhow::Error> {
    // First check if it's a maintainer
    if let Ok(Some(_)) = database.get_maintainer_by_username(signer).await {
        return Ok("maintainer".to_string());
    }

    // Then check if it's an emergency keyholder
    let emergency_keyholders = database
        .get_emergency_keyholders()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get emergency keyholders: {}", e))?;

    if emergency_keyholders
        .iter()
        .any(|ek| ek.github_username == signer)
    {
        return Ok("emergency".to_string());
    }

    // Default to maintainer if not found (for backward compatibility)
    Ok("maintainer".to_string())
}

/// Publish review period notification when PR enters review period
pub async fn publish_review_period_notification(
    config: &AppConfig,
    repository: &str,
    pr_number: i32,
    layer: i32,
    tier: u32,
    review_period_ends: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    if !config.nostr.enabled {
        return Ok(()); // Nostr disabled, skip
    }

    // Get combined requirements
    let (final_sigs_req, final_sigs_total, final_review_days) =
        ThresholdValidator::get_combined_requirements(layer, tier);

    // Create Nostr client
    let nsec = std::fs::read_to_string(&config.nostr.server_nsec_path)
        .map_err(|e| anyhow::anyhow!("Failed to read Nostr key: {}", e))?;

    let client = NostrClient::new(nsec, config.nostr.relays.clone()).await?;
    let keys = &client.keys;

    // Create review period notification event (Kind 30023 - Long-form)
    let content = format!(
        "# Governance Review Period Started\n\n\
        PR #{} in {} is now under review.\n\n\
        **Layer:** {}\n\
        **Tier:** {}\n\
        **Required Signatures:** {}-of-{}\n\
        **Review Period:** {} days\n\
        **Review Ends:** {}\n\n\
        [View PR](https://github.com/BTCDecoded/{}/pull/{})",
        pr_number,
        repository,
        layer,
        tier,
        final_sigs_req,
        final_sigs_total,
        final_review_days,
        review_period_ends.format("%Y-%m-%d %H:%M UTC"),
        repository,
        pr_number
    );

    let tags = vec![
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("d".into()),
            vec![format!("btc-commons-review-{}", pr_number)],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("t".into()),
            vec!["governance-review".to_string()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("governance_tier".into()),
            vec![tier.to_string()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("governance_layer".into()),
            vec![layer.to_string()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("repository".into()),
            vec![repository.to_string()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("governance_config".into()),
            vec![config.nostr.governance_config.clone()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("review_ends".into()),
            vec![review_period_ends.timestamp().to_string()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("pr".into()),
            vec![format!(
                "https://github.com/BTCDecoded/{}/pull/{}",
                repository, pr_number
            )],
        ),
    ];

    let event = nostr_sdk::prelude::EventBuilder::new(
        nostr_sdk::prelude::Kind::LongFormTextNote,
        content,
        tags,
    )
    .to_event(keys)
    .map_err(|e| anyhow::anyhow!("Failed to create Nostr event: {}", e))?;

    client.publish_event(event).await?;

    Ok(())
}

/// Publish keyholder announcement (Kind 0 - Metadata)
/// Note: In practice, keyholders publish their own announcements using their own keys.
/// This helper creates the event structure with logo/picture support.
pub fn create_keyholder_announcement_event(
    config: &AppConfig,
    keyholder: &crate::nostr::KeyholderAnnouncement,
) -> Result<nostr_sdk::prelude::Event> {
    // Add logo/picture if configured
    let announcement = keyholder.clone();
    // Note: picture field removed from KeyholderAnnouncement, logo_url is handled elsewhere

    let content = announcement
        .to_json()
        .map_err(|e| anyhow::anyhow!("Failed to serialize keyholder announcement: {}", e))?;

    let mut tags = vec![
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("governance_config".into()),
            vec![config.nostr.governance_config.clone()],
        ),
        nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("keyholder_type".into()),
            vec![keyholder.keyholder_type.clone()],
        ),
    ];

    if let Some(layer) = keyholder.layer {
        tags.push(nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("layer".into()),
            vec![layer.to_string()],
        ));
    }

    if let Some(zap) = &keyholder.zap_address {
        tags.push(nostr_sdk::prelude::Tag::Generic(
            nostr_sdk::prelude::TagKind::Custom("zap".into()),
            vec![zap.clone()],
        ));
    }

    // For Kind 0, the keyholder's own keys must be used
    // This function just creates the event structure
    // The keyholder would sign it with their own keys
    let keys = nostr_sdk::prelude::Keys::generate(); // Placeholder - actual keys come from keyholder

    let event =
        nostr_sdk::prelude::EventBuilder::new(nostr_sdk::prelude::Kind::Metadata, content, tags)
            .to_event(&keys)
            .map_err(|e| anyhow::anyhow!("Failed to create Nostr event: {}", e))?;

    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::nostr::KeyholderAnnouncement;

    async fn setup_test_config() -> AppConfig {
        AppConfig {
            nostr: crate::config::NostrConfig {
                enabled: false, // Disable for unit tests
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_publish_merge_action_disabled() {
        let config = setup_test_config().await;
        let db = Database::new_in_memory().await.unwrap();

        // Should return Ok immediately when Nostr is disabled
        let result =
            publish_merge_action(&config, &db, "BTCDecoded/blvm-consensus", 1, "abc123", 2, 3)
                .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_publish_review_period_notification_disabled() {
        let config = setup_test_config().await;

        // Should return Ok immediately when Nostr is disabled
        let result = publish_review_period_notification(
            &config,
            "BTCDecoded/blvm-consensus",
            1,
            2,
            3,
            chrono::Utc::now(),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_keyholder_announcement_event() {
        let config = setup_test_config().await;
        let keyholder = KeyholderAnnouncement {
            name: "alice".to_string(),
            about: "Test maintainer".to_string(),
            role: "maintainer".to_string(),
            governance_pubkey: "test_pubkey".to_string(),
            jurisdiction: None,
            backup_contact: None,
            joined: chrono::Utc::now().timestamp(),
            layer: Some(2),
            keyholder_type: "maintainer".to_string(),
            zap_address: Some("alice@example.com".to_string()),
        };

        let result = create_keyholder_announcement_event(&config, &keyholder);
        assert!(result.is_ok());

        let event = result.unwrap();
        assert_eq!(event.kind, nostr_sdk::prelude::Kind::Metadata);
    }

    #[tokio::test]
    async fn test_create_keyholder_announcement_event_without_layer() {
        let config = setup_test_config().await;
        let keyholder = KeyholderAnnouncement {
            name: "bob".to_string(),
            about: "Emergency keyholder".to_string(),
            role: "emergency".to_string(),
            governance_pubkey: "test_pubkey2".to_string(),
            jurisdiction: None,
            backup_contact: None,
            joined: chrono::Utc::now().timestamp(),
            layer: None,
            keyholder_type: "emergency_keyholder".to_string(),
            zap_address: None,
        };

        let result = create_keyholder_announcement_event(&config, &keyholder);
        assert!(result.is_ok());
    }
}
