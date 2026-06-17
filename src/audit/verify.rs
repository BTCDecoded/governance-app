//! Audit Log Verification
//!
//! Provides functions to verify the integrity of audit logs using cryptographic hashing

use crate::audit::entry::AuditLogEntry;
use crate::error::GovernanceError;

/// Verify the integrity of an audit log entry
pub fn verify_entry(entry: &AuditLogEntry) -> Result<bool, GovernanceError> {
    // Recalculate the hash
    let calculated_hash = calculate_entry_hash(entry)?;

    // Compare with stored hash
    Ok(calculated_hash == entry.this_log_hash)
}

/// Calculate hash for an audit log entry
/// This must match the hash calculation in AuditLogEntry::calculate_hash()
pub fn calculate_entry_hash(entry: &AuditLogEntry) -> Result<String, GovernanceError> {
    // Use the same canonical string format as AuditLogEntry::calculate_hash()
    Ok(entry.calculate_hash())
}

/// Verify the hash chain of audit log entries
pub fn verify_hash_chain(entries: &[AuditLogEntry]) -> Result<bool, GovernanceError> {
    if entries.is_empty() {
        return Ok(true);
    }

    // Verify each entry's hash
    for entry in entries {
        if !verify_entry(entry)? {
            return Ok(false);
        }
    }

    // Verify chain links (each entry's previous_hash matches previous entry's hash)
    for i in 1..entries.len() {
        if entries[i].previous_log_hash != entries[i - 1].this_log_hash {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Verify a specific entry in the chain
pub fn verify_entry_in_chain(
    entry: &AuditLogEntry,
    previous_entry: Option<&AuditLogEntry>,
) -> Result<bool, GovernanceError> {
    // Verify entry hash
    if !verify_entry(entry)? {
        return Ok(false);
    }

    // Verify chain link if previous entry exists
    if let Some(prev) = previous_entry {
        if entry.previous_log_hash != prev.this_log_hash {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Verify an audit log (alias for verify_hash_chain)
pub fn verify_audit_log(entries: &[AuditLogEntry]) -> Result<bool, GovernanceError> {
    verify_hash_chain(entries)
}

/// Verify an audit log from a file
pub fn verify_audit_log_file(file_path: &str) -> Result<bool, GovernanceError> {
    use serde_json;
    use std::fs;

    let content = fs::read_to_string(file_path)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to read audit log file: {e}")))?;

    let entries: Vec<AuditLogEntry> = serde_json::from_str(&content)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse audit log: {e}")))?;

    verify_hash_chain(&entries)
}

/// Load audit log entries from a file
pub fn load_audit_log_from_file(file_path: &str) -> Result<Vec<AuditLogEntry>, GovernanceError> {
    use serde_json;
    use std::fs;

    let content = fs::read_to_string(file_path)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to read audit log file: {e}")))?;

    let entries: Vec<AuditLogEntry> = serde_json::from_str(&content)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse audit log: {e}")))?;

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::entry::AuditLogEntry;
    use std::collections::HashMap;

    fn create_test_entry(previous_hash: &str) -> AuditLogEntry {
        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        AuditLogEntry::new(
            "test-job".to_string(),
            "test_type".to_string(),
            "governance-01".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            previous_hash.to_string(),
            metadata,
        )
    }

    #[test]
    fn test_verify_entry_valid() {
        let entry = create_test_entry("sha256:previous");
        let verified = verify_entry(&entry).unwrap();
        assert!(verified, "Valid entry should verify");
    }

    #[test]
    fn test_verify_entry_tampered() {
        let mut entry = create_test_entry("sha256:previous");
        entry.job_id = "tampered-job".to_string();
        let verified = verify_entry(&entry).unwrap();
        assert!(!verified, "Tampered entry should not verify");
    }

    #[test]
    fn test_verify_hash_chain_valid() {
        let entry1 = create_test_entry("sha256:genesis");
        let entry2 = create_test_entry(&entry1.this_log_hash);
        let entry3 = create_test_entry(&entry2.this_log_hash);

        let entries = vec![entry1, entry2, entry3];
        let verified = verify_hash_chain(&entries).unwrap();
        assert!(verified, "Valid chain should verify");
    }

    #[test]
    fn test_verify_hash_chain_broken_link() {
        let entry1 = create_test_entry("sha256:genesis");
        let mut entry2 = create_test_entry("sha256:wrong_previous"); // Wrong previous hash
        entry2.job_id = "entry2".to_string();
        let entry3 = create_test_entry(&entry2.this_log_hash);

        let entries = vec![entry1, entry2, entry3];
        let verified = verify_hash_chain(&entries).unwrap();
        assert!(!verified, "Chain with broken link should not verify");
    }

    #[test]
    fn test_verify_hash_chain_empty() {
        let entries = vec![];
        let verified = verify_hash_chain(&entries).unwrap();
        assert!(verified, "Empty chain should verify");
    }

    #[test]
    fn test_verify_entry_in_chain_with_previous() {
        let entry1 = create_test_entry("sha256:genesis");
        let entry2 = create_test_entry(&entry1.this_log_hash);

        let verified = verify_entry_in_chain(&entry2, Some(&entry1)).unwrap();
        assert!(verified, "Entry with valid previous should verify");
    }

    #[test]
    fn test_verify_entry_in_chain_wrong_previous() {
        let entry1 = create_test_entry("sha256:genesis");
        let mut entry2 = create_test_entry("sha256:wrong");
        entry2.job_id = "entry2".to_string();

        let verified = verify_entry_in_chain(&entry2, Some(&entry1)).unwrap();
        assert!(!verified, "Entry with wrong previous should not verify");
    }

    #[test]
    fn test_verify_entry_in_chain_no_previous() {
        let entry = create_test_entry("sha256:genesis");
        let verified = verify_entry_in_chain(&entry, None).unwrap();
        assert!(
            verified,
            "Entry without previous should verify if hash is valid"
        );
    }

    #[test]
    fn test_calculate_entry_hash_deterministic() {
        let entry = create_test_entry("sha256:previous");

        let hash1 = calculate_entry_hash(&entry).unwrap();
        let hash2 = calculate_entry_hash(&entry).unwrap();

        assert_eq!(hash1, hash2, "Hash calculation should be deterministic");
    }

    #[test]
    fn test_calculate_entry_hash_different_entries() {
        let entry1 = create_test_entry("sha256:previous");
        let mut entry2 = create_test_entry("sha256:previous");
        entry2.job_id = "different-job".to_string();

        let hash1 = calculate_entry_hash(&entry1).unwrap();
        let hash2 = calculate_entry_hash(&entry2).unwrap();

        assert_ne!(
            hash1, hash2,
            "Different entries should have different hashes"
        );
    }
}
