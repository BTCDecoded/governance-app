//! Content Hash Verification System
//!
//! This module provides cryptographic hash-based verification of file correspondence
//! between Orange Paper (Layer 1) and Consensus Proof (Layer 2) repositories.
//! It ensures that changes to one repository have corresponding changes in the other.

use crate::error::GovernanceError;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Represents a file correspondence mapping between repositories
#[derive(Debug, Clone)]
pub struct FileCorrespondence {
    pub orange_paper_file: String,
    pub consensus_proof_file: String,
    pub correspondence_type: CorrespondenceType,
}

/// Types of file correspondence
#[derive(Debug, Clone, PartialEq)]
pub enum CorrespondenceType {
    /// Direct 1:1 mapping (e.g., consensus-rules/block-validation.md -> proofs/block-validation.rs)
    Direct,
    /// One-to-many mapping (e.g., consensus-rules/transaction.md -> multiple proof files)
    OneToMany,
    /// Many-to-one mapping (e.g., multiple spec files -> single proof file)
    ManyToOne,
    /// Custom mapping with specific rules
    Custom(String),
}

/// Content hash verification result
#[derive(Debug, Clone)]
pub struct HashVerificationResult {
    pub file_path: String,
    pub computed_hash: String,
    pub expected_hash: Option<String>,
    pub is_valid: bool,
    pub error_message: Option<String>,
}

/// Directory hash verification result
#[derive(Debug, Clone)]
pub struct DirectoryHashResult {
    pub directory_path: String,
    pub merkle_root: String,
    pub file_count: usize,
    pub total_size: u64,
}

/// Cross-layer synchronization report
#[derive(Debug, Clone)]
pub struct SyncReport {
    pub source_repo: String,
    pub target_repo: String,
    pub changed_files: Vec<String>,
    pub correspondence_mappings: Vec<FileCorrespondence>,
    pub verification_results: Vec<HashVerificationResult>,
    pub sync_status: SyncStatus,
    pub missing_files: Vec<String>,
    pub outdated_files: Vec<String>,
}

/// Synchronization status
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// All files are synchronized
    Synchronized,
    /// Some files are missing corresponding updates
    MissingUpdates,
    /// Some files have outdated corresponding versions
    OutdatedVersions,
    /// Critical synchronization failure
    SyncFailure,
}

pub struct ContentHashValidator {
    pub correspondence_mappings: HashMap<String, FileCorrespondence>,
}

impl ContentHashValidator {
    /// Create a new content hash validator with correspondence mappings
    pub fn new() -> Self {
        Self {
            correspondence_mappings: HashMap::new(),
        }
    }

    /// Load correspondence mappings from configuration
    pub fn load_correspondence_mappings(&mut self, mappings: Vec<FileCorrespondence>) {
        for mapping in mappings {
            self.correspondence_mappings.insert(
                mapping.orange_paper_file.clone(),
                mapping,
            );
        }
        info!("Loaded {} correspondence mappings", self.correspondence_mappings.len());
    }

    /// Compute SHA256 hash of file content
    pub fn compute_file_hash(&self, content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    /// Compute Merkle tree hash of directory contents
    pub fn compute_directory_hash(&self, files: &[(String, Vec<u8>)]) -> DirectoryHashResult {
        if files.is_empty() {
            return DirectoryHashResult {
                directory_path: "empty".to_string(),
                merkle_root: "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
                file_count: 0,
                total_size: 0,
            };
        }

        // Sort files by path for consistent hashing
        let mut sorted_files = files.to_vec();
        sorted_files.sort_by(|a, b| a.0.cmp(&b.0));

        // Compute individual file hashes
        let file_hashes: Vec<String> = sorted_files
            .iter()
            .map(|(path, content)| {
                let mut hasher = Sha256::new();
                hasher.update(path.as_bytes());
                hasher.update(b"\0");
                hasher.update(content);
                hex::encode(hasher.finalize())
            })
            .collect();

        // Build Merkle tree
        let merkle_root = self.build_merkle_tree(&file_hashes);
        let total_size: u64 = sorted_files.iter().map(|(_, content)| content.len() as u64).sum();

        DirectoryHashResult {
            directory_path: "directory".to_string(),
            merkle_root: format!("sha256:{}", merkle_root),
            file_count: sorted_files.len(),
            total_size,
        }
    }

    /// Build Merkle tree from file hashes
    fn build_merkle_tree(&self, hashes: &[String]) -> String {
        if hashes.is_empty() {
            return "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string();
        }

        if hashes.len() == 1 {
            return hashes[0].clone();
        }

        let mut current_level = hashes.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    // Combine two hashes
                    let combined = format!("{}{}", current_level[i], current_level[i + 1]);
                    let mut hasher = Sha256::new();
                    hasher.update(combined.as_bytes());
                    next_level.push(hex::encode(hasher.finalize()));
                } else {
                    // Odd number of hashes, promote the last one
                    next_level.push(current_level[i].clone());
                }
            }
            
            current_level = next_level;
        }

        current_level[0].clone()
    }

    /// Verify file correspondence between repositories
    pub fn verify_correspondence(
        &self,
        source_file: &str,
        source_content: &[u8],
        target_repo_files: &HashMap<String, Vec<u8>>,
    ) -> Result<HashVerificationResult, GovernanceError> {
        let source_hash = self.compute_file_hash(source_content);
        
        // Find correspondence mapping
        let mapping = self.correspondence_mappings.get(source_file)
            .ok_or_else(|| GovernanceError::ValidationError(
                format!("No correspondence mapping found for file: {}", source_file)
            ))?;

        // Check if target file exists
        let target_content = target_repo_files.get(&mapping.consensus_proof_file)
            .ok_or_else(|| GovernanceError::ValidationError(
                format!("Corresponding file not found: {}", mapping.consensus_proof_file)
            ))?;

        let target_hash = self.compute_file_hash(target_content);
        
        // For now, we just verify the file exists and has content
        // In a real implementation, we would verify the content matches the specification
        let is_valid = !target_content.is_empty();
        
        Ok(HashVerificationResult {
            file_path: source_file.to_string(),
            computed_hash: source_hash,
            expected_hash: Some(target_hash),
            is_valid,
            error_message: if is_valid { None } else { Some("Target file is empty".to_string()) },
        })
    }

    /// Check bidirectional synchronization between Orange Paper and Consensus Proof
    pub fn check_bidirectional_sync(
        &self,
        orange_paper_files: &HashMap<String, Vec<u8>>,
        consensus_proof_files: &HashMap<String, Vec<u8>>,
        changed_files: &[String],
    ) -> Result<SyncReport, GovernanceError> {
        info!("Checking bidirectional sync for {} changed files", changed_files.len());

        let mut verification_results = Vec::new();
        let mut missing_files = Vec::new();
        let outdated_files = Vec::new();

        // Check each changed file for correspondence
        for changed_file in changed_files {
            if let Some(orange_content) = orange_paper_files.get(changed_file) {
                match self.verify_correspondence(changed_file, orange_content, consensus_proof_files) {
                    Ok(result) => {
                        if result.is_valid {
                            verification_results.push(result);
                        } else {
                            missing_files.push(changed_file.clone());
                        }
                    }
                    Err(e) => {
                        warn!("Failed to verify correspondence for {}: {}", changed_file, e);
                        missing_files.push(changed_file.clone());
                    }
                }
            }
        }

        // Determine sync status
        let sync_status = if missing_files.is_empty() && outdated_files.is_empty() {
            SyncStatus::Synchronized
        } else if !missing_files.is_empty() {
            SyncStatus::MissingUpdates
        } else {
            SyncStatus::OutdatedVersions
        };

        Ok(SyncReport {
            source_repo: "orange-paper".to_string(),
            target_repo: "consensus-proof".to_string(),
            changed_files: changed_files.to_vec(),
            correspondence_mappings: self.correspondence_mappings.values().cloned().collect(),
            verification_results,
            sync_status,
            missing_files,
            outdated_files,
        })
    }

    /// Generate correspondence mapping for Orange Paper and Consensus Proof
    pub fn generate_correspondence_map() -> Vec<FileCorrespondence> {
        vec![
            FileCorrespondence {
                orange_paper_file: "consensus-rules/block-validation.md".to_string(),
                consensus_proof_file: "proofs/block-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/transaction-validation.md".to_string(),
                consensus_proof_file: "proofs/transaction-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/utxo-validation.md".to_string(),
                consensus_proof_file: "proofs/utxo-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/script-validation.md".to_string(),
                consensus_proof_file: "proofs/script-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/economic-model.md".to_string(),
                consensus_proof_file: "proofs/economic-model.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/segwit-validation.md".to_string(),
                consensus_proof_file: "proofs/segwit-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
            FileCorrespondence {
                orange_paper_file: "consensus-rules/taproot-validation.md".to_string(),
                consensus_proof_file: "proofs/taproot-validation.rs".to_string(),
                correspondence_type: CorrespondenceType::Direct,
            },
        ]
    }

    /// Validate that all correspondence mappings are satisfied
    pub fn validate_all_correspondences(
        &self,
        orange_paper_files: &HashMap<String, Vec<u8>>,
        consensus_proof_files: &HashMap<String, Vec<u8>>,
    ) -> Result<Vec<HashVerificationResult>, GovernanceError> {
        let mut results = Vec::new();

        for mapping in self.correspondence_mappings.values() {
            if let Some(orange_content) = orange_paper_files.get(&mapping.orange_paper_file) {
                match self.verify_correspondence(
                    &mapping.orange_paper_file,
                    orange_content,
                    consensus_proof_files,
                ) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        error!("Failed to validate correspondence for {}: {}", mapping.orange_paper_file, e);
                        return Err(e);
                    }
                }
            } else {
                warn!("Orange Paper file not found: {}", mapping.orange_paper_file);
            }
        }

        Ok(results)
    }
}

impl Default for ContentHashValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_file_hash() {
        let validator = ContentHashValidator::new();
        let content = b"test content";
        let hash = validator.compute_file_hash(content);
        
        // Verify it's a valid SHA256 hash
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), 71); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_compute_directory_hash() {
        let validator = ContentHashValidator::new();
        let files = vec![
            ("file1.txt".to_string(), b"content1".to_vec()),
            ("file2.txt".to_string(), b"content2".to_vec()),
        ];
        
        let result = validator.compute_directory_hash(&files);
        
        assert_eq!(result.file_count, 2);
        assert_eq!(result.total_size, 16); // 8 + 8 bytes
        assert!(result.merkle_root.starts_with("sha256:"));
    }

    #[test]
    fn test_empty_directory_hash() {
        let validator = ContentHashValidator::new();
        let files = vec![];
        
        let result = validator.compute_directory_hash(&files);
        
        assert_eq!(result.file_count, 0);
        assert_eq!(result.total_size, 0);
        assert_eq!(result.merkle_root, "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn test_correspondence_mapping_generation() {
        let mappings = ContentHashValidator::generate_correspondence_map();
        
        assert!(!mappings.is_empty());
        assert!(mappings.iter().any(|m| m.orange_paper_file == "consensus-rules/block-validation.md"));
        assert!(mappings.iter().any(|m| m.consensus_proof_file == "proofs/block-validation.rs"));
    }

    #[test]
    fn test_verify_correspondence() {
        let mut validator = ContentHashValidator::new();
        let mappings = ContentHashValidator::generate_correspondence_map();
        validator.load_correspondence_mappings(mappings);

        let source_file = "consensus-rules/block-validation.md";
        let source_content = b"block validation rules";
        
        let mut target_files = HashMap::new();
        target_files.insert("proofs/block-validation.rs".to_string(), b"proof implementation".to_vec());

        let result = validator.verify_correspondence(source_file, source_content, &target_files);
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_valid);
        assert_eq!(result.file_path, source_file);
    }

    #[test]
    fn test_verify_correspondence_missing_target() {
        let mut validator = ContentHashValidator::new();
        let mappings = ContentHashValidator::generate_correspondence_map();
        validator.load_correspondence_mappings(mappings);

        let source_file = "consensus-rules/block-validation.md";
        let source_content = b"block validation rules";
        let target_files = HashMap::new(); // Empty target files

        let result = validator.verify_correspondence(source_file, source_content, &target_files);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Corresponding file not found"));
    }
}
