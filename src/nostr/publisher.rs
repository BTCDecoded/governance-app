//! Nostr Status Publisher
//!
//! Publishes hourly governance status updates to Nostr relays
//! with server health, audit log information, and verification hashes.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc, Datelike, Timelike};
use ::hex;
use nostr_sdk::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use tracing::{debug, error, info, warn};

use crate::database::Database;
use crate::nostr::client::NostrClient;
use crate::nostr::events::{GovernanceStatus, Hashes, ServerHealth};

/// Status publisher for governance infrastructure
pub struct StatusPublisher {
    client: NostrClient,
    database: Database,
    server_id: String,
    binary_path: String,
    config_path: String,
    start_time: DateTime<Utc>,
}

impl StatusPublisher {
    /// Create new status publisher
    pub fn new(
        client: NostrClient,
        database: Database,
        server_id: String,
        binary_path: String,
        config_path: String,
    ) -> Self {
        Self {
            client,
            database,
            server_id,
            binary_path,
            config_path,
            start_time: Utc::now(),
        }
    }

    /// Publish current governance status
    pub async fn publish_status(&self) -> Result<()> {
        info!("Publishing governance status for server: {}", self.server_id);

        // Calculate file hashes
        let binary_hash = self.calculate_file_hash(&self.binary_path)?;
        let config_hash = self.calculate_file_hash(&self.config_path)?;

        // Get server health information
        let health = self.get_server_health().await?;

        // Get audit log information
        let (audit_log_head, audit_log_length) = self.get_audit_log_info().await?;

        // Calculate next OTS anchor date (first day of next month)
        let next_ots_anchor = self.calculate_next_ots_anchor();

        // Create status event
        let status = GovernanceStatus::new(
            self.server_id.clone(),
            binary_hash,
            config_hash,
            health.uptime_hours,
            health.last_merge_pr,
            health.last_merge,
            health.merges_today,
            next_ots_anchor,
            health.relay_status,
            audit_log_head,
            audit_log_length,
        );

        // Create Nostr event
        let event = self.create_nostr_event(status)?;

        // Publish to relays
        self.client.publish_event(event).await?;

        info!("Successfully published governance status");
        Ok(())
    }

    /// Calculate SHA256 hash of a file
    fn calculate_file_hash(&self, file_path: &str) -> Result<String> {
        let content = fs::read(file_path)
            .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();
        
        Ok(format!("sha256:{}", hex::encode(hash)))
    }

    /// Get server health information
    async fn get_server_health(&self) -> Result<ServerHealth> {
        // Calculate uptime
        let uptime_hours = (Utc::now() - self.start_time).num_hours() as u64;

        // TODO: Implement these database methods
        let last_merge: Option<()> = None;
        let last_merge_pr = None;
        let last_merge_time = None;
        let merges_today = 0;

        // TODO: Implement relay status tracking
        let relay_status = HashMap::new();

        Ok(ServerHealth {
            uptime_hours,
            last_merge_pr,
            last_merge: last_merge_time,
            merges_today,
            relay_status,
        })
    }

    /// Get audit log information
    async fn get_audit_log_info(&self) -> Result<(Option<String>, Option<u64>)> {
        // This would be implemented when audit logging is added
        // For now, return None values
        Ok((None, None))
    }

    /// Calculate next OTS anchor date (first day of next month)
    fn calculate_next_ots_anchor(&self) -> DateTime<Utc> {
        let now = Utc::now();
        let next_month = if now.month() == 12 {
            now.with_month(1).unwrap().with_year(now.year() + 1).unwrap()
        } else {
            now.with_month(now.month() + 1).unwrap()
        };
        
        next_month.with_day(1).unwrap().with_hour(0).unwrap()
            .with_minute(0).unwrap().with_second(0).unwrap()
    }

    /// Create Nostr event from governance status
    fn create_nostr_event(&self, status: GovernanceStatus) -> Result<Event> {
        let content = status.to_json()
            .map_err(|e| anyhow!("Failed to serialize status: {}", e))?;

        let current_month = Utc::now().format("%Y-%m").to_string();

        let tags = vec![
            Tag::Generic(TagKind::Custom("d".into()), vec!["governance-status".to_string()]),
            Tag::Generic(TagKind::Custom("server".into()), vec![self.server_id.clone()]),
            Tag::Generic(TagKind::Custom("authorized_by".into()), vec![format!("registry-{}", current_month)]),
            Tag::Generic(TagKind::Custom("btcdecoded".into()), vec!["governance-infrastructure".to_string()]),
            Tag::Generic(TagKind::Custom("t".into()), vec!["bitcoin".to_string(), "governance".to_string()]),
        ];

        let event = EventBuilder::new(
            Kind::Custom(30078),
            content,
            tags,
        ).to_event(&self.client.keys)
            .map_err(|e| anyhow!("Failed to create Nostr event: {}", e))?;

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_file_hash_calculation() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let publisher = StatusPublisher {
            client: NostrClient::new("test".to_string(), vec![]).await.unwrap(),
            database: Database::new_sqlite(":memory:".to_string()).await.unwrap(),
            server_id: "test".to_string(),
            binary_path: test_file.to_string_lossy().to_string(),
            config_path: "".to_string(),
            start_time: Utc::now(),
        };

        let hash = publisher.calculate_file_hash(&test_file.to_string_lossy()).unwrap();
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_next_ots_anchor_calculation() {
        let publisher = StatusPublisher {
            client: NostrClient::new("test".to_string(), vec![]).await.unwrap(),
            database: Database::new_sqlite(":memory:".to_string()).await.unwrap(),
            server_id: "test".to_string(),
            binary_path: "".to_string(),
            config_path: "".to_string(),
            start_time: Utc::now(),
        };

        let next_anchor = publisher.calculate_next_ots_anchor();
        assert_eq!(next_anchor.day(), 1);
        assert_eq!(next_anchor.hour(), 0);
        assert_eq!(next_anchor.minute(), 0);
        assert_eq!(next_anchor.second(), 0);
    }
}
