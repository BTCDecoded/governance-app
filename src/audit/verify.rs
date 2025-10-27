//! Audit Log Verification
//!
//! Provides utilities for verifying audit log integrity and hash chains.

use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::audit::entry::AuditLogEntry;

/// Verify complete audit log hash chain
pub fn verify_audit_log(entries: &[AuditLogEntry]) -> Result<bool> {
    if entries.is_empty() {
        return Err(anyhow!("Empty audit log"));
    }

    // Check first entry (should be genesis)
    let first_entry = &entries[0];
    if first_entry.job_type != "genesis" {
        return Err(anyhow!("First entry must be genesis"));
    }

    // Verify each entry's hash
    for (i, entry) in entries.iter().enumerate() {
        if !entry.verify_hash() {
            return Err(anyhow!("Invalid hash in entry {}", i));
        }
    }

    // Verify hash chain
    for i in 1..entries.len() {
        let prev_entry = &entries[i - 1];
        let curr_entry = &entries[i];

        if curr_entry.previous_log_hash != prev_entry.this_log_hash {
            return Err(anyhow!(
                "Hash chain broken at entry {}: expected {}, got {}",
                i,
                prev_entry.this_log_hash,
                curr_entry.previous_log_hash
            ));
        }
    }

    // Verify timestamps are monotonic
    for i in 1..entries.len() {
        if entries[i].timestamp < entries[i - 1].timestamp {
            return Err(anyhow!(
                "Non-monotonic timestamp at entry {}: {} < {}",
                i,
                entries[i].timestamp,
                entries[i - 1].timestamp
            ));
        }
    }

    info!("Audit log verification successful: {} entries", entries.len());
    Ok(true)
}

/// Load audit log from file
pub fn load_audit_log_from_file(path: &str) -> Result<Vec<AuditLogEntry>> {
    let file = File::open(path)
        .map_err(|e| anyhow!("Failed to open audit log file: {}", e))?;

    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| anyhow!("Failed to read line {}: {}", line_num + 1, e))?;
        
        if line.trim().is_empty() {
            continue;
        }

        let entry: AuditLogEntry = serde_json::from_str(&line)
            .map_err(|e| anyhow!("Failed to parse entry at line {}: {}", line_num + 1, e))?;

        entries.push(entry);
    }

    debug!("Loaded {} entries from {}", entries.len(), path);
    Ok(entries)
}

/// Verify audit log file
pub fn verify_audit_log_file(path: &str) -> Result<bool> {
    info!("Verifying audit log file: {}", path);

    if !Path::new(path).exists() {
        return Err(anyhow!("Audit log file does not exist: {}", path));
    }

    let entries = load_audit_log_from_file(path)?;
    verify_audit_log(&entries)
}

/// Verify audit log file and return detailed results
pub fn verify_audit_log_detailed(path: &str) -> Result<VerificationResult> {
    info!("Detailed verification of audit log file: {}", path);

    if !Path::new(path).exists() {
        return Ok(VerificationResult {
            is_valid: false,
            entry_count: 0,
            error_message: Some(format!("File does not exist: {}", path)),
            hash_chain_valid: false,
            timestamps_monotonic: false,
        });
    }

    let entries = match load_audit_log_from_file(path) {
        Ok(entries) => entries,
        Err(e) => {
            return Ok(VerificationResult {
                is_valid: false,
                entry_count: 0,
                error_message: Some(format!("Failed to load entries: {}", e)),
                hash_chain_valid: false,
                timestamps_monotonic: false,
            });
        }
    };

    let entry_count = entries.len();
    let mut hash_chain_valid = true;
    let mut timestamps_monotonic = true;
    let mut error_message = None;

    // Check hash chain
    for i in 1..entries.len() {
        if entries[i].previous_log_hash != entries[i - 1].this_log_hash {
            hash_chain_valid = false;
            error_message = Some(format!(
                "Hash chain broken at entry {}: expected {}, got {}",
                i,
                entries[i - 1].this_log_hash,
                entries[i].previous_log_hash
            ));
            break;
        }
    }

    // Check timestamps
    for i in 1..entries.len() {
        if entries[i].timestamp < entries[i - 1].timestamp {
            timestamps_monotonic = false;
            if error_message.is_none() {
                error_message = Some(format!(
                    "Non-monotonic timestamp at entry {}: {} < {}",
                    i,
                    entries[i].timestamp,
                    entries[i - 1].timestamp
                ));
            }
            break;
        }
    }

    // Check individual entry hashes
    for (i, entry) in entries.iter().enumerate() {
        if !entry.verify_hash() {
            if error_message.is_none() {
                error_message = Some(format!("Invalid hash in entry {}", i));
            }
            break;
        }
    }

    let is_valid = hash_chain_valid && timestamps_monotonic && error_message.is_none();

    Ok(VerificationResult {
        is_valid,
        entry_count,
        error_message,
        hash_chain_valid,
        timestamps_monotonic,
    })
}

/// Verification result with detailed information
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub is_valid: bool,
    pub entry_count: usize,
    pub error_message: Option<String>,
    pub hash_chain_valid: bool,
    pub timestamps_monotonic: bool,
}

impl VerificationResult {
    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        if self.is_valid {
            format!("✅ Audit log is valid ({} entries)", self.entry_count)
        } else {
            format!(
                "❌ Audit log is invalid ({} entries): {}",
                self.entry_count,
                self.error_message.as_deref().unwrap_or("Unknown error")
            )
        }
    }

    /// Get detailed status
    pub fn detailed_status(&self) -> String {
        format!(
            "Entries: {}\nHash chain: {}\nTimestamps: {}\nError: {}",
            self.entry_count,
            if self.hash_chain_valid { "✅ Valid" } else { "❌ Invalid" },
            if self.timestamps_monotonic { "✅ Monotonic" } else { "❌ Non-monotonic" },
            self.error_message.as_deref().unwrap_or("None")
        )
    }
}

/// Find tampered entries in audit log
pub fn find_tampered_entries(entries: &[AuditLogEntry]) -> Vec<usize> {
    let mut tampered = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        if !entry.verify_hash() {
            tampered.push(i);
        }
    }

    tampered
}

/// Detect gaps in audit log
pub fn detect_gaps(entries: &[AuditLogEntry]) -> Vec<GapInfo> {
    let mut gaps = Vec::new();

    for i in 1..entries.len() {
        let prev_entry = &entries[i - 1];
        let curr_entry = &entries[i];

        // Check for time gaps (more than 1 hour between entries)
        let time_diff = curr_entry.timestamp - prev_entry.timestamp;
        if time_diff.num_hours() > 1 {
            gaps.push(GapInfo {
                start_index: i - 1,
                end_index: i,
                gap_type: GapType::TimeGap,
                description: format!(
                    "Time gap of {} hours between entries {} and {}",
                    time_diff.num_hours(),
                    i - 1,
                    i
                ),
            });
        }

        // Check for hash chain gaps
        if curr_entry.previous_log_hash != prev_entry.this_log_hash {
            gaps.push(GapInfo {
                start_index: i - 1,
                end_index: i,
                gap_type: GapType::HashGap,
                description: format!(
                    "Hash chain gap between entries {} and {}",
                    i - 1,
                    i
                ),
            });
        }
    }

    gaps
}

/// Information about a gap in the audit log
#[derive(Debug, Clone)]
pub struct GapInfo {
    pub start_index: usize,
    pub end_index: usize,
    pub gap_type: GapType,
    pub description: String,
}

/// Type of gap detected
#[derive(Debug, Clone)]
pub enum GapType {
    TimeGap,
    HashGap,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::collections::HashMap;

    #[test]
    fn test_verify_audit_log() {
        let mut entries = Vec::new();
        
        // Create genesis entry
        let genesis = crate::audit::entry::create_genesis_entry("test".to_string());
        entries.push(genesis);

        // Create test entry
        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());
        
        let entry = AuditLogEntry::new(
            "test-job".to_string(),
            "test_type".to_string(),
            "test".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            entries[0].this_log_hash.clone(),
            metadata,
        );
        entries.push(entry);

        let result = verify_audit_log(&entries);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_verification_result() {
        let result = VerificationResult {
            is_valid: true,
            entry_count: 5,
            error_message: None,
            hash_chain_valid: true,
            timestamps_monotonic: true,
        };

        assert!(result.is_valid);
        assert_eq!(result.entry_count, 5);
        assert!(result.summary().contains("✅"));
    }

    #[test]
    fn test_find_tampered_entries() {
        let mut entries = Vec::new();
        
        let genesis = crate::audit::entry::create_genesis_entry("test".to_string());
        entries.push(genesis);

        let mut metadata = HashMap::new();
        let mut entry = AuditLogEntry::new(
            "test-job".to_string(),
            "test_type".to_string(),
            "test".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            entries[0].this_log_hash.clone(),
            metadata,
        );
        
        // Tamper with the hash
        entry.this_log_hash = "sha256:tampered".to_string();
        entries.push(entry);

        let tampered = find_tampered_entries(&entries);
        assert_eq!(tampered.len(), 1);
        assert_eq!(tampered[0], 1);
    }
}
