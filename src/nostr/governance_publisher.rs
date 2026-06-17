//! Governance Action Publisher
//!
//! Publishes governance action events (merges, releases, etc.) to Nostr
//! with full layer + tier information and signature details.

use anyhow::{Result, anyhow};
use chrono::Utc;
use nostr_sdk::prelude::*;
use tracing::info;

use crate::nostr::client::NostrClient;
use crate::nostr::events::{
    CombinedRequirement, GovernanceActionEvent, KeyholderSignature, LayerRequirement,
    TierRequirement,
};

/// Publisher for governance action events
pub struct GovernanceActionPublisher {
    client: NostrClient,
    governance_config: String,   // e.g., "commons_mainnet"
    zap_address: Option<String>, // Lightning address for donations
}

impl GovernanceActionPublisher {
    /// Create new governance action publisher
    pub fn new(
        client: NostrClient,
        governance_config: String,
        zap_address: Option<String>,
    ) -> Self {
        Self {
            client,
            governance_config,
            zap_address,
        }
    }

    /// Publish a governance action event (merge, release, etc.)
    pub async fn publish_action(
        &self,
        action: &str, // "merge" | "release" | "budget" | "keyholder_change"
        governance_tier: u32,
        governance_layer: u32,
        repository: &str,
        final_signatures: &str, // e.g., "6-of-7"
        final_review_days: u32,
        commit_hash: Option<&str>,
        pr_number: Option<i32>,
        description: &str,
        layer_req: LayerRequirement,
        tier_req: TierRequirement,
        combined_req: CombinedRequirement,
        signatures: Vec<KeyholderSignature>,
        review_period_ends: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        info!(
            "Publishing governance action: {} for {}/PR#{}",
            action,
            repository,
            pr_number.unwrap_or(0)
        );

        // Create governance action event
        let action_event = GovernanceActionEvent {
            description: description.to_string(),
            pr_url: pr_number
                .map(|n| format!("https://github.com/BTCDecoded/{repository}/pull/{n}")),
            layer_requirement: layer_req,
            tier_requirement: tier_req,
            combined_requirement: combined_req.clone(),
            signatures,
            review_period_ends,
        };

        // Create Nostr event
        let event = self.create_nostr_event(
            action,
            governance_tier,
            governance_layer,
            repository,
            final_signatures,
            final_review_days,
            commit_hash,
            pr_number,
            &action_event,
        )?;

        // Publish to relays
        self.client.publish_event(event).await?;

        info!("Successfully published governance action event");
        Ok(())
    }

    /// Publish a governance proposal event with zap-to-vote support
    /// Returns the event ID for tracking zap votes
    pub async fn publish_proposal(
        &self,
        pr_id: i32,
        tier: u8,
        repository: &str,
        title: &str,
        description: &str,
        voting_window_days: u32,
    ) -> Result<String> {
        info!(
            "Publishing governance proposal: PR#{} in {} (Tier {})",
            pr_id, repository, tier
        );

        // Create proposal content
        let proposal_content = serde_json::json!({
            "title": title,
            "description": description,
            "pr_id": pr_id,
            "repository": repository,
            "tier": tier,
        });

        // Calculate voting window
        let voting_start = Utc::now();
        let voting_end = voting_start
            + chrono::TimeDelta::try_days(voting_window_days as i64).unwrap_or_default();

        // Create event tags
        let mut tags = vec![
            Tag::Generic(
                TagKind::Custom("d".into()),
                vec!["governance-proposal".to_string()],
            ),
            Tag::Generic(TagKind::Custom("pr".into()), vec![pr_id.to_string()]),
            Tag::Generic(TagKind::Custom("tier".into()), vec![tier.to_string()]),
            Tag::Generic(
                TagKind::Custom("repository".into()),
                vec![repository.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("vote_type".into()),
                vec!["support".to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("voting_window".into()),
                vec![voting_start.to_rfc3339(), voting_end.to_rfc3339()],
            ),
        ];

        // Add zap address for voting
        if let Some(zap) = &self.zap_address {
            tags.push(Tag::Generic(
                TagKind::Custom("zap".into()),
                vec![zap.clone()],
            ));
        }

        // Add governance config
        tags.push(Tag::Generic(
            TagKind::Custom("governance_config".into()),
            vec![self.governance_config.clone()],
        ));

        // Create event (kind 30078 for governance status)
        let event = EventBuilder::new(Kind::Custom(30078), proposal_content.to_string(), tags)
            .to_event(&self.client.keys)
            .map_err(|e| anyhow!("Failed to create proposal event: {}", e))?;

        // Publish to relays
        self.client.publish_event(event.clone()).await?;

        let event_id = event.id.to_string();
        info!(
            "Published governance proposal {} (event ID: {}) with zap-to-vote",
            pr_id, event_id
        );

        Ok(event_id)
    }

    /// Create Nostr event from governance action
    fn create_nostr_event(
        &self,
        action: &str,
        governance_tier: u32,
        governance_layer: u32,
        repository: &str,
        final_signatures: &str,
        final_review_days: u32,
        commit_hash: Option<&str>,
        pr_number: Option<i32>,
        action_event: &GovernanceActionEvent,
    ) -> Result<Event> {
        let content = action_event
            .to_json()
            .map_err(|e| anyhow!("Failed to serialize action event: {}", e))?;

        let mut tags = vec![
            Tag::Generic(
                TagKind::Custom("d".into()),
                vec!["btc-commons-governance-action".to_string()],
            ),
            Tag::Generic(TagKind::Custom("action".into()), vec![action.to_string()]),
            Tag::Generic(
                TagKind::Custom("governance_tier".into()),
                vec![governance_tier.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("governance_layer".into()),
                vec![governance_layer.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("repository".into()),
                vec![repository.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("governance_config".into()),
                vec![self.governance_config.clone()],
            ),
            Tag::Generic(
                TagKind::Custom("final_signatures".into()),
                vec![final_signatures.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("final_review_days".into()),
                vec![final_review_days.to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("timestamp".into()),
                vec![Utc::now().timestamp().to_string()],
            ),
        ];

        // Add optional tags
        if let Some(hash) = commit_hash {
            tags.push(Tag::Generic(
                TagKind::Custom("commit_hash".into()),
                vec![hash.to_string()],
            ));
        }

        if let Some(pr) = pr_number {
            tags.push(Tag::Generic(
                TagKind::Custom("pr_number".into()),
                vec![pr.to_string()],
            ));
        }

        // Add zap address if configured
        if let Some(zap) = &self.zap_address {
            tags.push(Tag::Generic(
                TagKind::Custom("zap".into()),
                vec![zap.clone()],
            ));
        }

        // Add Bitcoin Commons tags
        tags.push(Tag::Generic(
            TagKind::Custom("t".into()),
            vec!["btc-commons".to_string(), "governance".to_string()],
        ));

        let event = EventBuilder::new(Kind::Custom(30078), content, tags)
            .to_event(&self.client.keys)
            .map_err(|e| anyhow!("Failed to create Nostr event: {}", e))?;

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_publisher() -> GovernanceActionPublisher {
        // Create a mock client for testing
        // Note: This won't actually connect to relays
        let keys = nostr_sdk::prelude::Keys::generate();
        let nsec = keys.secret_key().unwrap().display_secret().to_string();
        let client = crate::nostr::client::NostrClient::new(nsec, vec![])
            .await
            .unwrap();

        GovernanceActionPublisher::new(client, "test-config".to_string(), None)
    }

    #[tokio::test]
    async fn test_governance_action_publisher_new() {
        let publisher = create_test_publisher().await;
        assert_eq!(publisher.governance_config, "test-config");
        assert!(publisher.zap_address.is_none());
    }

    #[tokio::test]
    async fn test_governance_action_publisher_with_zap() {
        let keys = nostr_sdk::prelude::Keys::generate();
        let nsec = keys.secret_key().unwrap().display_secret().to_string();
        let client = crate::nostr::client::NostrClient::new(nsec, vec![])
            .await
            .unwrap();

        let publisher = GovernanceActionPublisher::new(
            client,
            "test-config".to_string(),
            Some("zap@example.com".to_string()),
        );

        assert_eq!(publisher.governance_config, "test-config");
        assert_eq!(publisher.zap_address, Some("zap@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_governance_action_publisher_governance_config() {
        let publisher = create_test_publisher().await;
        assert_eq!(publisher.governance_config, "test-config");
    }
}
