//! Audit Log Entry
//!
//! Defines the structure for tamper-evident audit log entries
//! with cryptographic hash chains.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::debug;

use crate::audit::logger::AuditLogger;

/// Audit log entry with cryptographic hash chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub job_id: String,
    pub job_type: String,
    pub timestamp: DateTime<Utc>,
    pub server_id: String,
    pub inputs_hash: String,
    pub outputs_hash: String,
    pub previous_log_hash: String,
    pub this_log_hash: String,
    pub metadata: HashMap<String, String>,
}

impl AuditLogEntry {
    /// Create a new audit log entry
    pub fn new(
        job_id: String,
        job_type: String,
        server_id: String,
        inputs_hash: String,
        outputs_hash: String,
        previous_log_hash: String,
        metadata: HashMap<String, String>,
    ) -> Self {
        let timestamp = Utc::now();
        
        let mut entry = Self {
            job_id,
            job_type,
            timestamp,
            server_id,
            inputs_hash,
            outputs_hash,
            previous_log_hash,
            this_log_hash: String::new(), // Will be calculated
            metadata,
        };

        // Calculate this entry's hash
        entry.this_log_hash = entry.calculate_hash();
        entry
    }

    /// Create canonical string representation for hashing
    pub fn canonical_string(&self) -> String {
        format!(
            "job_id:{}|job_type:{}|timestamp:{}|server_id:{}|inputs_hash:{}|outputs_hash:{}|previous_log_hash:{}|metadata:{}",
            self.job_id,
            self.job_type,
            self.timestamp.to_rfc3339(),
            self.server_id,
            self.inputs_hash,
            self.outputs_hash,
            self.previous_log_hash,
            self.serialize_metadata()
        )
    }

    /// Calculate SHA256 hash of this entry
    pub fn calculate_hash(&self) -> String {
        let canonical = self.canonical_string();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let hash = hasher.finalize();
        format!("sha256:{}", hex::encode(hash))
    }

    /// Serialize metadata to string for hashing
    fn serialize_metadata(&self) -> String {
        let mut items: Vec<String> = self.metadata
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();
        items.sort(); // Ensure deterministic ordering
        items.join(",")
    }

    /// Verify this entry's hash
    pub fn verify_hash(&self) -> bool {
        self.this_log_hash == self.calculate_hash()
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "{}: {} ({} -> {})",
            self.job_type,
            self.job_id,
            self.inputs_hash,
            self.outputs_hash
        )
    }
}

/// Genesis entry for starting the hash chain
pub fn create_genesis_entry(server_id: String) -> AuditLogEntry {
    let mut metadata = HashMap::new();
    metadata.insert("description".to_string(), "Genesis entry".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    AuditLogEntry::new(
        "genesis".to_string(),
        "genesis".to_string(),
        server_id,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        metadata,
    )
}

/// Job execution wrapper for audit logging
pub async fn execute_with_audit<F, T, E>(
    logger: &mut AuditLogger,
    job_type: &str,
    server_id: &str,
    inputs: &[u8],
    job: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T, E>,
    E: std::fmt::Display,
    T: serde::Serialize + std::fmt::Debug,
{
    // Generate job ID
    let job_id = format!("{}-{}", job_type, Utc::now().timestamp_millis());

    // Hash inputs
    let mut hasher = Sha256::new();
    hasher.update(inputs);
    let inputs_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

    // Get previous log hash
    let previous_log_hash = logger.get_head_hash().await;

    // Execute job
    let result = job().map_err(|e| anyhow!("Job execution failed: {}", e))?;

    // Hash outputs (serialize result to bytes)
    let outputs_bytes = serde_json::to_vec(&result)
        .unwrap_or_else(|_| format!("{:?}", result).into_bytes());
    let mut hasher = Sha256::new();
    hasher.update(&outputs_bytes);
    let outputs_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

    // Create metadata
    let mut metadata = HashMap::new();
    metadata.insert("execution_time".to_string(), Utc::now().to_rfc3339());
    metadata.insert("success".to_string(), "true".to_string());

    // Create audit entry
    let entry = AuditLogEntry::new(
        job_id,
        job_type.to_string(),
        server_id.to_string(),
        inputs_hash,
        outputs_hash,
        previous_log_hash,
        metadata,
    );

    // Append to log
    logger.append_entry(entry).await?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_audit_entry_creation() {
        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        let entry = AuditLogEntry::new(
            "test-job-123".to_string(),
            "test_job".to_string(),
            "governance-01".to_string(),
            "sha256:abc123".to_string(),
            "sha256:def456".to_string(),
            "sha256:prev789".to_string(),
            metadata,
        );

        assert_eq!(entry.job_id, "test-job-123");
        assert_eq!(entry.job_type, "test_job");
        assert_eq!(entry.server_id, "governance-01");
        assert!(entry.verify_hash());
    }

    #[test]
    fn test_canonical_string() {
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        let entry = AuditLogEntry::new(
            "test".to_string(),
            "test_type".to_string(),
            "server".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            "sha256:prev".to_string(),
            metadata,
        );

        let canonical = entry.canonical_string();
        assert!(canonical.contains("test"));
        assert!(canonical.contains("test_type"));
        assert!(canonical.contains("server"));
    }

    #[test]
    fn test_hash_calculation() {
        let entry = AuditLogEntry::new(
            "test".to_string(),
            "test_type".to_string(),
            "server".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            "sha256:prev".to_string(),
            HashMap::new(),
        );

        let hash1 = entry.calculate_hash();
        let hash2 = entry.calculate_hash();
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"));
        assert_eq!(hash1.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_genesis_entry() {
        let genesis = create_genesis_entry("governance-01".to_string());
        assert_eq!(genesis.job_type, "genesis");
        assert_eq!(genesis.server_id, "governance-01");
        assert!(genesis.verify_hash());
    }
}
