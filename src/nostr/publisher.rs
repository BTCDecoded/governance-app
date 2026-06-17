//! Nostr Status Publisher
//!
//! Publishes hourly governance status updates to Nostr relays
//! with server health, audit log information, and verification hashes.

use ::hex;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike, Utc};
use nostr_sdk::prelude::*;
use sha2::{Digest, Sha256};
use std::fs;
use tracing::{info, warn};

use crate::audit::logger::AuditLogger;
use crate::database::Database;
use crate::nostr::client::NostrClient;
use crate::nostr::events::{GovernanceStatus, ServerHealth};

/// Status publisher for governance infrastructure
pub struct StatusPublisher {
    client: NostrClient,
    database: Database,
    server_id: String,
    binary_path: String,
    config_path: String,
    audit_log_path: Option<String>,
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
        audit_log_path: Option<String>,
    ) -> Self {
        Self {
            client,
            database,
            server_id,
            binary_path,
            config_path,
            audit_log_path,
            start_time: Utc::now(),
        }
    }

    /// Publish current governance status
    pub async fn publish_status(&self) -> Result<()> {
        info!(
            "Publishing governance status for server: {}",
            self.server_id
        );

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
        let content =
            fs::read(file_path).map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = hasher.finalize();

        Ok(format!("sha256:{}", hex::encode(hash)))
    }

    /// Get server health information
    async fn get_server_health(&self) -> Result<ServerHealth> {
        // Calculate uptime
        let uptime_hours = (Utc::now() - self.start_time).num_hours() as u64;

        // Get last merged PR information from database
        let (last_merge_pr, last_merge_time) = match self.database.get_last_merged_pr().await {
            Ok(Some((pr_number, timestamp))) => (pr_number, timestamp),
            Ok(None) => (None, None),
            Err(e) => {
                warn!("Failed to get last merged PR: {}", e);
                (None, None)
            }
        };

        // Count merges today
        let merges_today = match self.database.count_merges_today().await {
            Ok(count) => count,
            Err(e) => {
                warn!("Failed to count merges today: {}", e);
                0
            }
        };

        // Get relay status from Nostr client
        // Note: Relay status tracking is already implemented in NostrClient
        let relay_status = self.client.get_relay_status().await;

        Ok(ServerHealth {
            uptime_hours,
            last_merge_pr,
            last_merge: last_merge_time,
            merges_today: merges_today as i64,
            relay_status,
        })
    }

    /// Get audit log information
    /// Returns (merkle_root, entry_count) for the audit log
    async fn get_audit_log_info(&self) -> Result<(Option<String>, Option<u64>)> {
        // If audit logging is not enabled or path not configured, return None
        let log_path = match &self.audit_log_path {
            Some(path) => path,
            None => return Ok((None, None)),
        };

        // Create audit logger to read entries
        let logger = match AuditLogger::new(log_path.clone()) {
            Ok(l) => l,
            Err(e) => {
                warn!(
                    "Failed to create audit logger: {}. Audit log info unavailable.",
                    e
                );
                return Ok((None, None));
            }
        };

        // Get all entries
        let entries = match logger.get_all_entries().await {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    "Failed to read audit log entries: {}. Audit log info unavailable.",
                    e
                );
                return Ok((None, None));
            }
        };

        // If no entries, return empty state
        if entries.is_empty() {
            return Ok((
                Some(
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                ),
                Some(0),
            ));
        }

        // Calculate Merkle root from all entries
        let merkle_root = Self::calculate_merkle_root(&entries)?;
        let entry_count = entries.len() as u64;

        Ok((Some(merkle_root), Some(entry_count)))
    }

    /// Calculate Merkle root from audit log entries
    fn calculate_merkle_root(entries: &[crate::audit::entry::AuditLogEntry]) -> Result<String> {
        use sha2::{Digest, Sha256};

        if entries.is_empty() {
            return Ok(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            );
        }

        // Hash each entry using its this_log_hash (which is already a hash of the entry)
        // Extract the hex part (after "sha256:") for Merkle tree calculation
        let mut hashes: Vec<[u8; 32]> = entries
            .iter()
            .map(|e| {
                // Extract hex from "sha256:hexstring"
                let hex_str = e
                    .this_log_hash
                    .strip_prefix("sha256:")
                    .unwrap_or(&e.this_log_hash);
                let hash_bytes = hex::decode(hex_str).unwrap_or_else(|_| {
                    // Fallback: hash the entry's canonical string
                    let canonical = e.canonical_string();
                    Sha256::digest(canonical.as_bytes()).to_vec()
                });
                // Ensure we have exactly 32 bytes
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes[..32.min(hash_bytes.len())]);
                hash
            })
            .collect();

        // Build Merkle tree
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in hashes.chunks(2) {
                if chunk.len() == 2 {
                    // Combine two hashes
                    let combined = [chunk[0].as_slice(), chunk[1].as_slice()].concat();
                    let hash_vec = Sha256::digest(&combined).to_vec();
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&hash_vec[..32.min(hash_vec.len())]);
                    next_level.push(hash);
                } else {
                    // Odd number, duplicate last hash
                    let combined = [chunk[0].as_slice(), chunk[0].as_slice()].concat();
                    let hash_vec = Sha256::digest(&combined).to_vec();
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&hash_vec[..32.min(hash_vec.len())]);
                    next_level.push(hash);
                }
            }
            hashes = next_level;
        }

        Ok(format!("sha256:{}", hex::encode(hashes[0])))
    }

    /// Calculate next OTS anchor date (first day of next month)
    fn calculate_next_ots_anchor(&self) -> DateTime<Utc> {
        let now = Utc::now();
        let date = now.date_naive();
        let next_month_date = if date.month() == 12 {
            date.with_year(date.year() + 1)
                .unwrap()
                .with_month(1)
                .unwrap()
        } else {
            date.with_month(date.month() + 1).unwrap()
        };
        next_month_date
            .with_day(1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
    }

    /// Create Nostr event from governance status
    fn create_nostr_event(&self, status: GovernanceStatus) -> Result<Event> {
        let content = status
            .to_json()
            .map_err(|e| anyhow!("Failed to serialize status: {}", e))?;

        let current_month = Utc::now().format("%Y-%m").to_string();

        let tags = vec![
            Tag::Generic(
                TagKind::Custom("d".into()),
                vec!["governance-status".to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("server".into()),
                vec![self.server_id.clone()],
            ),
            Tag::Generic(
                TagKind::Custom("authorized_by".into()),
                vec![format!("registry-{}", current_month)],
            ),
            Tag::Generic(
                TagKind::Custom("btcdecoded".into()),
                vec!["governance-infrastructure".to_string()],
            ),
            Tag::Generic(
                TagKind::Custom("t".into()),
                vec!["bitcoin".to_string(), "governance".to_string()],
            ),
        ];

        let event = EventBuilder::new(Kind::Custom(30078), content, tags)
            .to_event(&self.client.keys)
            .map_err(|e| anyhow!("Failed to create Nostr event: {}", e))?;

        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_sdk::prelude::Keys;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_hash_calculation() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        // Generate valid Nostr keys for testing
        let keys = Keys::generate();
        let nsec = keys.secret_key().unwrap().display_secret().to_string();

        let publisher = StatusPublisher {
            client: NostrClient::new(nsec, vec![]).await.unwrap(),
            database: Database::new_in_memory().await.unwrap(),
            server_id: "test".to_string(),
            binary_path: test_file.to_string_lossy().to_string(),
            config_path: "".to_string(),
            audit_log_path: None,
            start_time: Utc::now(),
        };

        let hash = publisher
            .calculate_file_hash(&test_file.to_string_lossy())
            .unwrap();
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), 71); // "sha256:" + 64 hex chars
    }

    #[tokio::test]
    async fn test_next_ots_anchor_calculation() {
        // Generate valid Nostr keys for testing
        let keys = Keys::generate();
        let nsec = keys.secret_key().unwrap().display_secret().to_string();

        let publisher = StatusPublisher {
            client: NostrClient::new(nsec, vec![]).await.unwrap(),
            database: Database::new_in_memory().await.unwrap(),
            server_id: "test".to_string(),
            binary_path: "".to_string(),
            config_path: "".to_string(),
            audit_log_path: None,
            start_time: Utc::now(),
        };

        let next_anchor = publisher.calculate_next_ots_anchor();
        assert_eq!(next_anchor.day(), 1);
        assert_eq!(next_anchor.hour(), 0);
        assert_eq!(next_anchor.minute(), 0);
        assert_eq!(next_anchor.second(), 0);
    }
}
