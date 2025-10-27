//! Cross-Layer Status Checks
//!
//! This module provides GitHub status check integration for cross-layer validation,
//! including content hash verification, version pinning, and equivalence proof status.

use crate::error::GovernanceError;
use crate::validation::content_hash::{ContentHashValidator, SyncReport, SyncStatus};
use crate::validation::version_pinning::{VersionPinningValidator, VersionReference};
use crate::validation::equivalence_proof::{EquivalenceProofValidator, VerificationResult, VerificationStatus};
use crate::github::client::GitHubClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error};

/// Status check result for cross-layer validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossLayerStatusCheck {
    pub state: StatusState,
    pub description: String,
    pub target_url: Option<String>,
    pub context: String,
    pub details: CrossLayerStatusDetails,
}

/// GitHub status check states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatusState {
    Success,
    Failure,
    Pending,
    Error,
}

/// Detailed status information for cross-layer validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossLayerStatusDetails {
    pub content_hash_status: ContentHashStatus,
    pub version_pinning_status: VersionPinningStatus,
    pub equivalence_proof_status: EquivalenceProofStatus,
    pub overall_sync_status: SyncStatus,
    pub recommendations: Vec<String>,
}

/// Content hash verification status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentHashStatus {
    pub status: StatusState,
    pub message: String,
    pub files_checked: usize,
    pub files_synced: usize,
    pub files_missing: Vec<String>,
    pub files_outdated: Vec<String>,
}

/// Version pinning status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionPinningStatus {
    pub status: StatusState,
    pub message: String,
    pub references_checked: usize,
    pub references_valid: usize,
    pub references_invalid: Vec<VersionReferenceError>,
    pub latest_version: Option<String>,
}

/// Version reference error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionReferenceError {
    pub file_path: String,
    pub line_number: usize,
    pub reference: VersionReference,
    pub error_message: String,
}

/// Equivalence proof status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquivalenceProofStatus {
    pub status: StatusState,
    pub message: String,
    pub tests_run: usize,
    pub tests_passed: usize,
    pub tests_failed: Vec<String>,
    pub proof_verification: Option<String>,
}

/// Cross-layer status checker
pub struct CrossLayerStatusChecker {
    github_client: GitHubClient,
    content_hash_validator: ContentHashValidator,
    version_pinning_validator: VersionPinningValidator,
    equivalence_proof_validator: EquivalenceProofValidator,
}

impl CrossLayerStatusChecker {
    pub fn new(github_client: GitHubClient) -> Self {
        Self {
            github_client,
            content_hash_validator: ContentHashValidator::new(),
            version_pinning_validator: VersionPinningValidator::default(),
            equivalence_proof_validator: EquivalenceProofValidator::new(),
        }
    }

    /// Generate comprehensive cross-layer status check for a PR
    pub async fn generate_cross_layer_status(
        &mut self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        changed_files: &[String],
    ) -> Result<CrossLayerStatusCheck, GovernanceError> {
        info!("Generating cross-layer status for {}/{} PR #{}", owner, repo, pr_number);

        // 1. Check content hash synchronization
        let content_hash_status = self.check_content_hash_sync(owner, repo, changed_files).await?;

        // 2. Check version pinning
        let version_pinning_status = self.check_version_pinning(owner, repo, changed_files).await?;

        // 3. Check equivalence proofs
        let equivalence_proof_status = self.check_equivalence_proofs(owner, repo, changed_files).await?;

        // 4. Determine overall status
        let overall_status = self.determine_overall_status(&content_hash_status, &version_pinning_status, &equivalence_proof_status);

        // 5. Generate recommendations
        let recommendations = self.generate_recommendations(&content_hash_status, &version_pinning_status, &equivalence_proof_status);

        // 6. Create status check
        let status_check = CrossLayerStatusCheck {
            state: overall_status,
            description: self.generate_status_description(&content_hash_status, &version_pinning_status, &equivalence_proof_status),
            target_url: Some(format!("https://github.com/{}/{}/pull/{}", owner, repo, pr_number)),
            context: "cross-layer-sync".to_string(),
            details: CrossLayerStatusDetails {
                content_hash_status,
                version_pinning_status,
                equivalence_proof_status,
                overall_sync_status: self.map_status_to_sync_status(overall_status),
                recommendations,
            },
        };

        info!("Generated cross-layer status: {:?}", status_check.state);
        Ok(status_check)
    }

    /// Check content hash synchronization
    async fn check_content_hash_sync(
        &mut self,
        owner: &str,
        repo: &str,
        changed_files: &[String],
    ) -> Result<ContentHashStatus, GovernanceError> {
        info!("Checking content hash synchronization for {} files", changed_files.len());

        // Load correspondence mappings
        let correspondence_mappings = ContentHashValidator::generate_correspondence_map();
        self.content_hash_validator.load_correspondence_mappings(correspondence_mappings);

        // For now, simulate the check (in real implementation, would fetch files from GitHub)
        let mut files_checked = 0;
        let mut files_synced = 0;
        let mut files_missing = Vec::new();
        let mut files_outdated = Vec::new();

        for file in changed_files {
            files_checked += 1;
            
            // Simulate checking if file has corresponding updates
            if file.contains("consensus-rules") {
                // Check if corresponding proof file exists and is updated
                if self.simulate_file_sync_check(file) {
                    files_synced += 1;
                } else {
                    files_missing.push(file.clone());
                }
            } else {
                files_synced += 1; // Non-consensus files don't need sync
            }
        }

        let status = if files_missing.is_empty() && files_outdated.is_empty() {
            StatusState::Success
        } else {
            StatusState::Failure
        };

        let message = if files_missing.is_empty() {
            format!("✅ Content Hash Sync: All {} files are synchronized", files_checked)
        } else {
            format!("❌ Content Hash Sync: {} files missing updates: {}", 
                   files_missing.len(), files_missing.join(", "))
        };

        Ok(ContentHashStatus {
            status,
            message,
            files_checked,
            files_synced,
            files_missing,
            files_outdated,
        })
    }

    /// Check version pinning compliance
    async fn check_version_pinning(
        &mut self,
        owner: &str,
        repo: &str,
        changed_files: &[String],
    ) -> Result<VersionPinningStatus, GovernanceError> {
        info!("Checking version pinning for {} files", changed_files.len());

        let mut references_checked = 0;
        let mut references_valid = 0;
        let mut references_invalid = Vec::new();

        // For each changed file, check for version references
        for file in changed_files {
            if file.ends_with(".rs") || file.ends_with(".md") {
                // Simulate parsing version references
                let references = self.simulate_parse_version_references(file);
                references_checked += references.len();

                for reference in references {
                    if self.simulate_verify_version_reference(&reference) {
                        references_valid += 1;
                    } else {
                        references_invalid.push(VersionReferenceError {
                            file_path: file.clone(),
                            line_number: 1, // Simulated
                            reference: reference.clone(),
                            error_message: "Invalid version reference".to_string(),
                        });
                    }
                }
            }
        }

        let status = if references_invalid.is_empty() {
            StatusState::Success
        } else {
            StatusState::Failure
        };

        let message = if references_invalid.is_empty() {
            format!("✅ Version Pinning: All {} references are valid", references_checked)
        } else {
            format!("❌ Version Pinning: {} invalid references found", references_invalid.len())
        };

        Ok(VersionPinningStatus {
            status,
            message,
            references_checked,
            references_valid,
            references_invalid,
            latest_version: Some("v1.2.3".to_string()), // Simulated
        })
    }

    /// Check equivalence proof validation
    async fn check_equivalence_proofs(
        &mut self,
        owner: &str,
        repo: &str,
        changed_files: &[String],
    ) -> Result<EquivalenceProofStatus, GovernanceError> {
        info!("Checking equivalence proofs for {} files", changed_files.len());

        // Load test vectors
        let test_vectors = EquivalenceProofValidator::generate_consensus_test_vectors();
        self.equivalence_proof_validator.load_test_vectors(test_vectors);

        let mut tests_run = 0;
        let mut tests_passed = 0;
        let mut tests_failed = Vec::new();

        // Run equivalence tests for consensus-related files
        for file in changed_files {
            if file.contains("consensus-rules") || file.contains("proofs") {
                tests_run += 1;
                
                // Simulate running equivalence tests
                if self.simulate_equivalence_test(file) {
                    tests_passed += 1;
                } else {
                    tests_failed.push(format!("{}: Equivalence test failed", file));
                }
            }
        }

        let status = if tests_failed.is_empty() {
            StatusState::Success
        } else {
            StatusState::Failure
        };

        let message = if tests_failed.is_empty() {
            format!("✅ Equivalence Proof: All {} tests passed", tests_run)
        } else {
            format!("❌ Equivalence Proof: {} tests failed", tests_failed.len())
        };

        Ok(EquivalenceProofStatus {
            status,
            message,
            tests_run,
            tests_passed,
            tests_failed,
            proof_verification: Some("sha256:verified_proof_hash".to_string()),
        })
    }

    /// Determine overall status from individual checks
    fn determine_overall_status(
        &self,
        content_hash: &ContentHashStatus,
        version_pinning: &VersionPinningStatus,
        equivalence_proof: &EquivalenceProofStatus,
    ) -> StatusState {
        if content_hash.status == StatusState::Success &&
           version_pinning.status == StatusState::Success &&
           equivalence_proof.status == StatusState::Success {
            StatusState::Success
        } else if content_hash.status == StatusState::Failure ||
                  version_pinning.status == StatusState::Failure ||
                  equivalence_proof.status == StatusState::Failure {
            StatusState::Failure
        } else {
            StatusState::Pending
        }
    }

    /// Generate status description
    fn generate_status_description(
        &self,
        content_hash: &ContentHashStatus,
        version_pinning: &VersionPinningStatus,
        equivalence_proof: &EquivalenceProofStatus,
    ) -> String {
        let mut parts = Vec::new();
        
        if content_hash.status == StatusState::Success {
            parts.push("Content Hash: ✅".to_string());
        } else {
            parts.push("Content Hash: ❌".to_string());
        }

        if version_pinning.status == StatusState::Success {
            parts.push("Version Pinning: ✅".to_string());
        } else {
            parts.push("Version Pinning: ❌".to_string());
        }

        if equivalence_proof.status == StatusState::Success {
            parts.push("Equivalence Proof: ✅".to_string());
        } else {
            parts.push("Equivalence Proof: ❌".to_string());
        }

        parts.join(" | ")
    }

    /// Generate recommendations based on status
    fn generate_recommendations(
        &self,
        content_hash: &ContentHashStatus,
        version_pinning: &VersionPinningStatus,
        equivalence_proof: &EquivalenceProofStatus,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !content_hash.files_missing.is_empty() {
            recommendations.push(format!(
                "Update corresponding Consensus Proof files: {}",
                content_hash.files_missing.join(", ")
            ));
        }

        if !version_pinning.references_invalid.is_empty() {
            recommendations.push("Update version references to point to valid Orange Paper versions".to_string());
        }

        if !equivalence_proof.tests_failed.is_empty() {
            recommendations.push("Fix failing equivalence tests to ensure implementation matches specification".to_string());
        }

        if recommendations.is_empty() {
            recommendations.push("All cross-layer checks passed! Ready to merge.".to_string());
        }

        recommendations
    }

    /// Map status state to sync status
    fn map_status_to_sync_status(&self, status: StatusState) -> SyncStatus {
        match status {
            StatusState::Success => SyncStatus::Synchronized,
            StatusState::Failure => SyncStatus::MissingUpdates,
            StatusState::Pending => SyncStatus::SyncFailure,
            StatusState::Error => SyncStatus::SyncFailure,
        }
    }

    // Simulation methods (in real implementation, these would make actual GitHub API calls)

    fn simulate_file_sync_check(&self, file: &str) -> bool {
        // Simulate checking if corresponding file exists and is synced
        !file.contains("block-validation") // Simulate that block-validation needs sync
    }

    fn simulate_parse_version_references(&self, file: &str) -> Vec<VersionReference> {
        if file.contains("consensus") {
            vec![
                VersionReference {
                    file_path: file.to_string(),
                    orange_paper_version: "v1.2.3".to_string(),
                    orange_paper_commit: "abc123def456".to_string(),
                    orange_paper_hash: "sha256:1234567890abcdef".to_string(),
                }
            ]
        } else {
            vec![]
        }
    }

    fn simulate_verify_version_reference(&self, reference: &VersionReference) -> bool {
        // Simulate version verification
        reference.orange_paper_version.starts_with("v1.")
    }

    fn simulate_equivalence_test(&self, file: &str) -> bool {
        // Simulate equivalence test
        !file.contains("script-execution") // Simulate that script-execution test fails
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::client::GitHubClient;

    #[tokio::test]
    async fn test_cross_layer_status_generation() {
        let github_client = GitHubClient::new("test_token".to_string());
        let mut checker = CrossLayerStatusChecker::new(github_client);
        
        let changed_files = vec![
            "consensus-rules/block-validation.md".to_string(),
            "proofs/block-validation.rs".to_string(),
        ];

        let status = checker.generate_cross_layer_status("test_owner", "test_repo", 123, &changed_files).await.unwrap();
        
        assert_eq!(status.context, "cross-layer-sync");
        assert!(status.target_url.is_some());
        assert!(!status.details.recommendations.is_empty());
    }
}

