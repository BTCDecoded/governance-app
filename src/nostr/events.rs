//! Nostr Event Types for Governance Status
//!
//! Defines the structure of governance status events published to Nostr.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Governance status event published to Nostr
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatus {
    pub server_id: String,
    pub timestamp: DateTime<Utc>,
    pub hashes: Hashes,
    pub health: ServerHealth,
    pub next_ots_anchor: DateTime<Utc>,
    pub audit_log_head: Option<String>,
    pub audit_log_length: Option<u64>,
}

/// File hashes for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hashes {
    pub binary: String,  // sha256:...
    pub config: String,  // sha256:...
}

/// Server health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub uptime_hours: u64,
    pub last_merge_pr: Option<i32>,
    pub last_merge: Option<DateTime<Utc>>,
    pub merges_today: i64,
    pub relay_status: std::collections::HashMap<String, bool>,
}

impl GovernanceStatus {
    /// Create a new governance status event
    pub fn new(
        server_id: String,
        binary_hash: String,
        config_hash: String,
        uptime_hours: u64,
        last_merge_pr: Option<i32>,
        last_merge: Option<DateTime<Utc>>,
        merges_today: i64,
        next_ots_anchor: DateTime<Utc>,
        relay_status: std::collections::HashMap<String, bool>,
        audit_log_head: Option<String>,
        audit_log_length: Option<u64>,
    ) -> Self {
        Self {
            server_id,
            timestamp: Utc::now(),
            hashes: Hashes {
                binary: binary_hash,
                config: config_hash,
            },
            health: ServerHealth {
                uptime_hours,
                last_merge_pr,
                last_merge,
                merges_today,
                relay_status,
            },
            next_ots_anchor,
            audit_log_head,
            audit_log_length,
        }
    }

    /// Serialize to JSON for Nostr event content
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "Server {}: {}h uptime, {} merges today, next OTS: {}",
            self.server_id,
            self.health.uptime_hours,
            self.health.merges_today,
            self.next_ots_anchor.format("%Y-%m-%d")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_governance_status_creation() {
        let relay_status = HashMap::new();
        let status = GovernanceStatus::new(
            "governance-01".to_string(),
            "sha256:abc123".to_string(),
            "sha256:def456".to_string(),
            24,
            Some(123),
            Some(Utc::now()),
            5,
            Utc::now(),
            relay_status,
            Some("sha256:head123".to_string()),
            Some(1000),
        );

        assert_eq!(status.server_id, "governance-01");
        assert_eq!(status.hashes.binary, "sha256:abc123");
        assert_eq!(status.health.uptime_hours, 24);
        assert_eq!(status.health.merges_today, 5);
    }

    #[test]
    fn test_json_serialization() {
        let relay_status = HashMap::new();
        let status = GovernanceStatus::new(
            "test-server".to_string(),
            "sha256:test".to_string(),
            "sha256:config".to_string(),
            1,
            None,
            None,
            0,
            Utc::now(),
            relay_status,
            None,
            None,
        );

        let json = status.to_json().unwrap();
        assert!(json.contains("test-server"));
        assert!(json.contains("sha256:test"));
    }

    #[test]
    fn test_summary() {
        let relay_status = HashMap::new();
        let status = GovernanceStatus::new(
            "governance-01".to_string(),
            "sha256:abc".to_string(),
            "sha256:def".to_string(),
            48,
            Some(456),
            Some(Utc::now()),
            3,
            Utc::now(),
            relay_status,
            None,
            None,
        );

        let summary = status.summary();
        assert!(summary.contains("governance-01"));
        assert!(summary.contains("48h uptime"));
        assert!(summary.contains("3 merges today"));
    }
}
