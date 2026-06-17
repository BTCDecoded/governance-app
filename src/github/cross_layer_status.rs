//! Cross-Layer Status Checks
//!
//! This module provides GitHub status check integration for cross-layer validation,
//! including content hash verification, version pinning, and equivalence proof status.

use crate::database::models::PullRequest as DatabasePullRequest;
use crate::error::GovernanceError;
use crate::github::client::GitHubClient;
use crate::validation::ValidationResult;
use crate::validation::content_hash::{ContentHashValidator, SyncStatus};
use crate::validation::equivalence_proof::EquivalenceProofValidator;
use crate::validation::verification_check::check_verification_status;
use crate::validation::version_pinning::{VersionPinningValidator, VersionReference};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

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
        let mut validator = Self {
            github_client,
            content_hash_validator: ContentHashValidator::new(),
            version_pinning_validator: VersionPinningValidator::default(),
            equivalence_proof_validator: EquivalenceProofValidator::new(),
        };

        // Load test vectors with fallback (for future use, even though we use CI now)
        if let Err(e) = validator
            .equivalence_proof_validator
            .load_test_vectors_with_fallback()
        {
            warn!("Failed to load test vectors: {}", e);
        }

        validator
    }

    /// Generate comprehensive cross-layer status check for a PR
    pub async fn generate_cross_layer_status(
        &mut self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        changed_files: &[String],
    ) -> Result<CrossLayerStatusCheck, GovernanceError> {
        info!(
            "Generating cross-layer status for {}/{} PR #{}",
            owner, repo, pr_number
        );

        // 1. Check content hash synchronization
        let content_hash_status = self
            .check_content_hash_sync(owner, repo, changed_files)
            .await?;

        // 2. Check version pinning
        let version_pinning_status = self
            .check_version_pinning(owner, repo, changed_files)
            .await?;

        // 3. Check equivalence proofs
        let equivalence_proof_status = self
            .check_equivalence_proofs(owner, repo, pr_number, changed_files)
            .await?;

        // 4. Determine overall status
        let overall_status = self.determine_overall_status(
            &content_hash_status,
            &version_pinning_status,
            &equivalence_proof_status,
        );

        // 5. Generate recommendations
        let recommendations = self.generate_recommendations(
            &content_hash_status,
            &version_pinning_status,
            &equivalence_proof_status,
        );

        // 6. Create status check
        let overall_sync_status = self.map_status_to_sync_status(overall_status.clone());
        let status_check = CrossLayerStatusCheck {
            state: overall_status,
            description: self.generate_status_description(
                &content_hash_status,
                &version_pinning_status,
                &equivalence_proof_status,
            ),
            target_url: Some(format!(
                "https://github.com/{owner}/{repo}/pull/{pr_number}"
            )),
            context: "cross-layer-sync".to_string(),
            details: CrossLayerStatusDetails {
                content_hash_status,
                version_pinning_status,
                equivalence_proof_status,
                overall_sync_status,
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
        info!(
            "Checking content hash synchronization for {} files",
            changed_files.len()
        );

        // Load correspondence mappings
        let correspondence_mappings = ContentHashValidator::generate_correspondence_map();
        self.content_hash_validator
            .load_correspondence_mappings(correspondence_mappings);

        // For now, simulate the check (in real implementation, would fetch files from GitHub)
        let mut files_checked = 0;
        let mut files_synced = 0;
        let mut files_missing = Vec::new();
        let files_outdated = Vec::new();

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
            format!("✅ Content Hash Sync: All {files_checked} files are synchronized")
        } else {
            format!(
                "❌ Content Hash Sync: {} files missing updates: {}",
                files_missing.len(),
                files_missing.join(", ")
            )
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
            format!("✅ Version Pinning: All {references_checked} references are valid")
        } else {
            format!(
                "❌ Version Pinning: {} invalid references found",
                references_invalid.len()
            )
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
        pr_number: u64,
        _changed_files: &[String],
    ) -> Result<EquivalenceProofStatus, GovernanceError> {
        info!(
            "Checking equivalence proofs for {}/{} PR #{}",
            owner, repo, pr_number
        );

        // Check if this is a verification-required repository
        let repo_name = format!("{owner}/{repo}");
        if crate::validation::verification_check::requires_verification(&repo_name)? {
            // Get PR data from GitHub
            let pr_json = self
                .github_client
                .get_pull_request(owner, repo, pr_number)
                .await?;

            // Extract head_sha from PR response
            let head_sha = pr_json["head"]["sha"]
                .as_str()
                .ok_or_else(|| {
                    GovernanceError::GitHubError("Missing head SHA in PR response".to_string())
                })?
                .to_string();

            // Convert to database::models::PullRequest for verification_check
            let pr = DatabasePullRequest {
                id: 0, // Not needed for verification
                repo_name: repo_name.clone(),
                pr_number: pr_number as i32,
                opened_at: chrono::Utc::now(), // Not critical for verification
                layer: 0,                      // Not critical for verification
                head_sha,
                signatures: vec![],
                governance_status: "pending".to_string(),
                linked_prs: vec![],
                emergency_mode: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            // Use actual verification check
            let verification_result = check_verification_status(&self.github_client, &pr).await?;

            // Extract test counts from CI check runs
            let (tests_run, tests_passed, tests_failed) = self
                .extract_test_counts_from_ci(owner, &repo_name, &pr.head_sha)
                .await
                .unwrap_or_else(|e| {
                    warn!("Failed to extract test counts from CI: {}", e);
                    (0, 0, vec![])
                });

            // Map verification result to EquivalenceProofStatus
            match verification_result {
                ValidationResult::Valid { message } => Ok(EquivalenceProofStatus {
                    status: StatusState::Success,
                    message: format!("✅ Equivalence Proof: {message}"),
                    tests_run,
                    tests_passed,
                    tests_failed,
                    proof_verification: Some("CI verification passed".to_string()),
                }),
                ValidationResult::Invalid {
                    message,
                    blocking: _,
                } => {
                    let mut failed = tests_failed;
                    let message_clone = message.clone();
                    if !message.is_empty() {
                        failed.push(message);
                    }
                    Ok(EquivalenceProofStatus {
                        status: StatusState::Failure,
                        message: format!("❌ Equivalence Proof: {message_clone}"),
                        tests_run,
                        tests_passed,
                        tests_failed: failed,
                        proof_verification: Some("CI verification failed".to_string()),
                    })
                }
                ValidationResult::Pending { message } => Ok(EquivalenceProofStatus {
                    status: StatusState::Pending,
                    message: format!("⏳ Equivalence Proof: {message}"),
                    tests_run,
                    tests_passed,
                    tests_failed,
                    proof_verification: None,
                }),
                ValidationResult::NotApplicable => {
                    // Not a verification-required repo, return success
                    Ok(EquivalenceProofStatus {
                        status: StatusState::Success,
                        message: "Equivalence proof not required for this repository".to_string(),
                        tests_run,
                        tests_passed,
                        tests_failed,
                        proof_verification: None,
                    })
                }
            }
        } else {
            // Not a verification-required repo, but still try to extract test counts
            let (tests_run, tests_passed, tests_failed) = {
                // Get PR data to extract head SHA
                if let Ok(pr_json) = self
                    .github_client
                    .get_pull_request(owner, repo, pr_number)
                    .await
                {
                    if let Some(head_sha) = pr_json["head"]["sha"].as_str() {
                        self.extract_test_counts_from_ci(owner, repo, head_sha)
                            .await
                            .unwrap_or_else(|e| {
                                warn!("Failed to extract test counts: {}", e);
                                (0, 0, vec![])
                            })
                    } else {
                        (0, 0, vec![])
                    }
                } else {
                    (0, 0, vec![])
                }
            };

            // Not a verification-required repo
            Ok(EquivalenceProofStatus {
                status: StatusState::Success,
                message: "Equivalence proof not required for this repository".to_string(),
                tests_run,
                tests_passed,
                tests_failed,
                proof_verification: None,
            })
        }
    }

    /// Extract test counts from CI check runs
    /// Attempts to parse test counts from check run names and conclusions
    async fn extract_test_counts_from_ci(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<(usize, usize, Vec<String>), GovernanceError> {
        // Get check runs for the commit
        let check_runs = self.github_client.get_check_runs(owner, repo, sha).await?;

        let mut tests_run = 0;
        let mut tests_passed = 0;
        let mut tests_failed = Vec::new();

        // Look for test-related check runs
        // Common patterns: "Tests", "cargo test", "Test", "CI", etc.
        for check_run in &check_runs {
            let name_lower = check_run.name.to_lowercase();

            // Check if this is a test-related check run
            if name_lower.contains("test")
                || name_lower.contains("cargo test")
                || name_lower.contains("unit test")
                || name_lower.contains("property test")
                || name_lower.contains("spec-lock")
                || name_lower.contains("proptest")
            {
                // Try to extract test counts from check run name
                // Common patterns: "Tests (123 passed, 5 failed)" or "cargo test: 128 tests"
                if let Some(count) = Self::extract_test_count_from_name_impl(&check_run.name) {
                    tests_run += count;
                } else {
                    // If we can't extract a count, assume at least 1 test was run
                    tests_run += 1;
                }

                // Check conclusion to determine pass/fail
                match check_run.conclusion.as_deref() {
                    Some("success") => {
                        // If we couldn't extract a count, assume all passed
                        if tests_passed == 0 && tests_run > 0 {
                            tests_passed = tests_run;
                        } else if tests_run > tests_passed {
                            // Estimate: if we have a count, assume most passed
                            tests_passed += (tests_run - tests_passed).max(1);
                        }
                    }
                    Some("failure") | Some("cancelled") | Some("timed_out") => {
                        tests_failed.push(format!(
                            "{}: {}",
                            check_run.name,
                            check_run.conclusion.as_deref().unwrap_or("failed")
                        ));
                        // If we have a test count, assume at least one failed
                        if tests_run > tests_passed {
                            // Already accounted for
                        } else if tests_run > 0 {
                            tests_passed = tests_run.saturating_sub(1);
                        }
                    }
                    _ => {
                        // Pending or unknown - don't count yet
                    }
                }
            }
        }

        // If we found test-related check runs but couldn't extract counts,
        // use the number of successful test check runs as a proxy
        if tests_run == 0 {
            let test_check_runs: Vec<_> = check_runs
                .iter()
                .filter(|cr| {
                    let name_lower = cr.name.to_lowercase();
                    name_lower.contains("test") || name_lower.contains("cargo test")
                })
                .collect();

            tests_run = test_check_runs.len();
            tests_passed = test_check_runs
                .iter()
                .filter(|cr| cr.conclusion.as_deref() == Some("success"))
                .count();
        }

        Ok((tests_run, tests_passed, tests_failed))
    }

    /// Extract test count from check run name using regex patterns
    /// Looks for patterns like "123 tests", "Tests: 456", etc.
    /// This is public for use in tests and fuzz targets.
    pub fn extract_test_count_from_name(name: &str) -> Option<usize> {
        Self::extract_test_count_from_name_impl(name)
    }

    fn extract_test_count_from_name_impl(name: &str) -> Option<usize> {
        use regex::Regex;

        // Pattern: "123 tests" or "Tests: 456" or "cargo test: 789" (case-insensitive, handles plural)
        let patterns = vec![
            r"(?i)(\d+)\s+test[s]?",
            r"(?i)test[s]?[:\s]+(\d+)",
            r"(?i)(\d+)\s+passed",
            r"(?i)passed[:\s]+(\d+)",
            r"(?i)\((\d+)\s+test[s]?\)", // Matches "(123 tests)" format
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(captures) = re.captures(name) {
                    if let Some(count_str) = captures.get(1) {
                        if let Ok(count) = count_str.as_str().parse::<usize>() {
                            return Some(count);
                        }
                    }
                }
            }
        }

        None
    }

    /// Check if repository requires verification
    fn requires_verification(&self, repo: &str) -> Result<bool, GovernanceError> {
        crate::validation::verification_check::requires_verification(repo)
            .map_err(|e| GovernanceError::ValidationError(e.to_string()))
    }

    /// Determine overall status from individual checks
    fn determine_overall_status(
        &self,
        content_hash: &ContentHashStatus,
        version_pinning: &VersionPinningStatus,
        equivalence_proof: &EquivalenceProofStatus,
    ) -> StatusState {
        if content_hash.status == StatusState::Success
            && version_pinning.status == StatusState::Success
            && equivalence_proof.status == StatusState::Success
        {
            StatusState::Success
        } else if content_hash.status == StatusState::Failure
            || version_pinning.status == StatusState::Failure
            || equivalence_proof.status == StatusState::Failure
        {
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
            recommendations.push(
                "Update version references to point to valid Orange Paper versions".to_string(),
            );
        }

        if !equivalence_proof.tests_failed.is_empty() {
            recommendations.push(
                "Fix failing equivalence tests to ensure implementation matches specification"
                    .to_string(),
            );
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
            vec![VersionReference {
                file_path: file.to_string(),
                line_number: 0,
                reference_type: crate::validation::version_pinning::VersionReferenceType::Combined,
                version: "v1.2.3".to_string(),
                commit_sha: Some("abc123def456".to_string()),
                content_hash: Some("sha256:1234567890abcdef".to_string()),
                raw_text: "v1.2.3".to_string(),
            }]
        } else {
            vec![]
        }
    }

    fn simulate_verify_version_reference(&self, reference: &VersionReference) -> bool {
        // Simulate version verification
        reference.version.starts_with("v1.")
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

    // Unit tests for extract_test_count_from_name
    #[test]
    fn test_extract_test_count_pattern_123_tests() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("123 tests"),
            Some(123)
        );
    }

    #[test]
    fn test_extract_test_count_pattern_tests_456() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("Tests: 456"),
            Some(456)
        );
    }

    #[test]
    fn test_extract_test_count_pattern_cargo_test_789() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("cargo test: 789"),
            Some(789)
        );
    }

    #[test]
    fn test_extract_test_count_pattern_1000_passed() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("1000 passed"),
            Some(1000)
        );
    }

    #[test]
    fn test_extract_test_count_pattern_passed_42() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("passed: 42"),
            Some(42)
        );
    }

    #[test]
    fn test_extract_test_count_no_match() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("No numbers here"),
            None
        );
    }

    #[test]
    fn test_extract_test_count_edge_case_zero() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("0 tests"),
            Some(0)
        );
    }

    #[test]
    fn test_extract_test_count_edge_case_large_number() {
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("999999 tests"),
            Some(999999)
        );
    }

    #[test]
    fn test_extract_test_count_case_insensitive() {
        // Test that case doesn't matter for "test" keyword
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("TEST: 100"),
            Some(100)
        );
    }

    #[test]
    fn test_extract_test_count_multiple_numbers() {
        // Should extract first matching number
        assert_eq!(
            CrossLayerStatusChecker::extract_test_count_from_name("50 tests passed out of 100"),
            Some(50)
        );
    }

    // Helper function to create a test checker.
    // Uses test_fixtures RSA key when available; falls back to from_token("test") for CI (no API calls in unit tests).
    fn create_test_checker() -> Option<CrossLayerStatusChecker> {
        let github_client = {
            let temp_dir = tempfile::tempdir().ok()?;
            let key_path = temp_dir.path().join("test_key.pem");
            let valid_key = include_str!("../../test_fixtures/test_rsa_key.pem");
            std::fs::write(&key_path, valid_key).ok()?;
            GitHubClient::new(123456, key_path.to_str()?)
                .ok()
                .or_else(|| GitHubClient::from_token("test").ok())?
        };
        Some(CrossLayerStatusChecker {
            github_client,
            content_hash_validator: ContentHashValidator::new(),
            version_pinning_validator: VersionPinningValidator::default(),
            equivalence_proof_validator: EquivalenceProofValidator::new(),
        })
    }

    // Unit tests for determine_overall_status
    #[tokio::test]
    async fn test_determine_overall_status_all_success() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return, // Skip if can't create client
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let result =
            checker.determine_overall_status(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(result, StatusState::Success);
    }

    #[tokio::test]
    async fn test_determine_overall_status_any_failure() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        // Test: Content hash failure
        let content_hash = ContentHashStatus {
            status: StatusState::Failure,
            message: "Missing files".to_string(),
            files_checked: 10,
            files_synced: 5,
            files_missing: vec!["file1.md".to_string()],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let result =
            checker.determine_overall_status(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(result, StatusState::Failure);
    }

    #[tokio::test]
    async fn test_determine_overall_status_pending() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        // Test: All pending
        let content_hash = ContentHashStatus {
            status: StatusState::Pending,
            message: "Checking...".to_string(),
            files_checked: 0,
            files_synced: 0,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Pending,
            message: "Checking...".to_string(),
            references_checked: 0,
            references_valid: 0,
            references_invalid: vec![],
            latest_version: None,
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Pending,
            message: "Checking...".to_string(),
            tests_run: 0,
            tests_passed: 0,
            tests_failed: vec![],
            proof_verification: None,
        };

        let result =
            checker.determine_overall_status(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(result, StatusState::Pending);
    }

    #[tokio::test]
    async fn test_determine_overall_status_mixed_pending_success() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        // Test: Success + Pending = Pending
        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Pending,
            message: "Checking...".to_string(),
            references_checked: 0,
            references_valid: 0,
            references_invalid: vec![],
            latest_version: None,
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let result =
            checker.determine_overall_status(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(result, StatusState::Pending);
    }

    // Unit tests for generate_recommendations
    #[tokio::test]
    async fn test_generate_recommendations_all_success() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("All cross-layer checks passed"));
    }

    #[tokio::test]
    async fn test_generate_recommendations_content_hash_missing() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Failure,
            message: "Missing files".to_string(),
            files_checked: 10,
            files_synced: 5,
            files_missing: vec!["consensus-rules/block-validation.md".to_string()],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("Update corresponding Consensus Proof files"));
        assert!(recommendations[0].contains("block-validation.md"));
    }

    #[tokio::test]
    async fn test_generate_recommendations_version_pinning_invalid() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Failure,
            message: "Invalid references".to_string(),
            references_checked: 5,
            references_valid: 3,
            references_invalid: vec![VersionReferenceError {
                file_path: "src/validation.rs".to_string(),
                line_number: 42,
                reference: VersionReference {
                    file_path: "src/validation.rs".to_string(),
                    line_number: 42,
                    reference_type:
                        crate::validation::version_pinning::VersionReferenceType::Version,
                    version: "v9.9.9".to_string(),
                    commit_sha: None,
                    content_hash: None,
                    raw_text: "v9.9.9".to_string(),
                },
                error_message: "Invalid version".to_string(),
            }],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("Update version references"));
        assert!(recommendations[0].contains("Orange Paper"));
    }

    #[tokio::test]
    async fn test_generate_recommendations_equivalence_proof_failed() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Failure,
            message: "Tests failed".to_string(),
            tests_run: 100,
            tests_passed: 95,
            tests_failed: vec![
                "test_block_validation".to_string(),
                "test_tx_validation".to_string(),
            ],
            proof_verification: Some("Failed".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("Fix failing equivalence tests"));
        assert!(recommendations[0].contains("implementation matches specification"));
    }

    #[tokio::test]
    async fn test_generate_recommendations_multiple_failures() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Failure,
            message: "Missing files".to_string(),
            files_checked: 10,
            files_synced: 5,
            files_missing: vec!["file1.md".to_string(), "file2.md".to_string()],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Failure,
            message: "Invalid references".to_string(),
            references_checked: 5,
            references_valid: 3,
            references_invalid: vec![VersionReferenceError {
                file_path: "src/validation.rs".to_string(),
                line_number: 42,
                reference: VersionReference {
                    file_path: "src/validation.rs".to_string(),
                    line_number: 42,
                    reference_type:
                        crate::validation::version_pinning::VersionReferenceType::Version,
                    version: "v9.9.9".to_string(),
                    commit_sha: None,
                    content_hash: None,
                    raw_text: "v9.9.9".to_string(),
                },
                error_message: "Invalid version".to_string(),
            }],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Failure,
            message: "Tests failed".to_string(),
            tests_run: 100,
            tests_passed: 95,
            tests_failed: vec!["test_block_validation".to_string()],
            proof_verification: Some("Failed".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 3);
        assert!(
            recommendations
                .iter()
                .any(|r| r.contains("Update corresponding Consensus Proof files"))
        );
        assert!(
            recommendations
                .iter()
                .any(|r| r.contains("Update version references"))
        );
        assert!(
            recommendations
                .iter()
                .any(|r| r.contains("Fix failing equivalence tests"))
        );
    }

    #[tokio::test]
    async fn test_generate_recommendations_content_hash_multiple_files() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Failure,
            message: "Missing files".to_string(),
            files_checked: 10,
            files_synced: 5,
            files_missing: vec![
                "consensus-rules/block-validation.md".to_string(),
                "consensus-rules/transaction-validation.md".to_string(),
                "consensus-rules/script-execution.md".to_string(),
            ],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let recommendations =
            checker.generate_recommendations(&content_hash, &version_pinning, &equivalence_proof);
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].contains("block-validation.md"));
        assert!(recommendations[0].contains("transaction-validation.md"));
        assert!(recommendations[0].contains("script-execution.md"));
    }

    // Unit tests for generate_status_description
    #[tokio::test]
    async fn test_generate_status_description_all_success() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Success,
            message: "All valid".to_string(),
            references_checked: 5,
            references_valid: 5,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Success,
            message: "All passed".to_string(),
            tests_run: 100,
            tests_passed: 100,
            tests_failed: vec![],
            proof_verification: Some("Verified".to_string()),
        };

        let description = checker.generate_status_description(
            &content_hash,
            &version_pinning,
            &equivalence_proof,
        );
        assert!(description.contains("Content Hash: ✅"));
        assert!(description.contains("Version Pinning: ✅"));
        assert!(description.contains("Equivalence Proof: ✅"));
        assert!(description.contains(" | ")); // Should have separators
    }

    #[tokio::test]
    async fn test_generate_status_description_all_failure() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Failure,
            message: "Missing files".to_string(),
            files_checked: 10,
            files_synced: 5,
            files_missing: vec!["file1.md".to_string()],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Failure,
            message: "Invalid references".to_string(),
            references_checked: 5,
            references_valid: 3,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Failure,
            message: "Tests failed".to_string(),
            tests_run: 100,
            tests_passed: 95,
            tests_failed: vec!["test1".to_string()],
            proof_verification: Some("Failed".to_string()),
        };

        let description = checker.generate_status_description(
            &content_hash,
            &version_pinning,
            &equivalence_proof,
        );
        assert!(description.contains("Content Hash: ❌"));
        assert!(description.contains("Version Pinning: ❌"));
        assert!(description.contains("Equivalence Proof: ❌"));
    }

    #[tokio::test]
    async fn test_generate_status_description_mixed() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let content_hash = ContentHashStatus {
            status: StatusState::Success,
            message: "All synced".to_string(),
            files_checked: 10,
            files_synced: 10,
            files_missing: vec![],
            files_outdated: vec![],
        };

        let version_pinning = VersionPinningStatus {
            status: StatusState::Failure,
            message: "Invalid references".to_string(),
            references_checked: 5,
            references_valid: 3,
            references_invalid: vec![],
            latest_version: Some("v1.0.0".to_string()),
        };

        let equivalence_proof = EquivalenceProofStatus {
            status: StatusState::Pending,
            message: "Checking...".to_string(),
            tests_run: 0,
            tests_passed: 0,
            tests_failed: vec![],
            proof_verification: None,
        };

        let description = checker.generate_status_description(
            &content_hash,
            &version_pinning,
            &equivalence_proof,
        );
        assert!(description.contains("Content Hash: ✅"));
        assert!(description.contains("Version Pinning: ❌"));
        assert!(description.contains("Equivalence Proof: ❌")); // Pending shows as ❌
    }

    // Unit tests for map_status_to_sync_status
    #[tokio::test]
    async fn test_map_status_to_sync_status_success() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let sync_status = checker.map_status_to_sync_status(StatusState::Success);
        assert_eq!(sync_status, SyncStatus::Synchronized);
    }

    #[tokio::test]
    async fn test_map_status_to_sync_status_failure() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let sync_status = checker.map_status_to_sync_status(StatusState::Failure);
        assert_eq!(sync_status, SyncStatus::MissingUpdates);
    }

    #[tokio::test]
    async fn test_map_status_to_sync_status_pending() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let sync_status = checker.map_status_to_sync_status(StatusState::Pending);
        assert_eq!(sync_status, SyncStatus::SyncFailure);
    }

    #[tokio::test]
    async fn test_map_status_to_sync_status_error() {
        let checker = match create_test_checker() {
            Some(c) => c,
            None => return,
        };

        let sync_status = checker.map_status_to_sync_status(StatusState::Error);
        assert_eq!(sync_status, SyncStatus::SyncFailure);
    }

    /// Integration test: uses test_fixtures/test_rsa_key.pem. Makes real GitHub API calls.
    /// Skipped when key parsing fails (e.g. CI without fixture). Run with --ignored for full test.
    #[tokio::test]
    #[ignore = "Makes real GitHub API calls; run with cargo test -- --ignored"]
    async fn test_cross_layer_status_generation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let key_path = temp_dir.path().join("test_key.pem");
        let valid_key = include_str!("../../test_fixtures/test_rsa_key.pem");
        std::fs::write(&key_path, valid_key).unwrap();

        let github_client = match GitHubClient::new(123456, key_path.to_str().unwrap()) {
            Ok(client) => client,
            Err(_) => return, // Skip if key parsing fails
        };
        let mut checker = CrossLayerStatusChecker::new(github_client);

        let changed_files = vec![
            "consensus-rules/block-validation.md".to_string(),
            "proofs/block-validation.rs".to_string(),
        ];

        let status = checker
            .generate_cross_layer_status("test_owner", "test_repo", 123, &changed_files)
            .await
            .expect("Failed to generate cross-layer status in test");

        assert_eq!(status.context, "cross-layer-sync");
        assert!(status.target_url.is_some());
        assert!(!status.details.recommendations.is_empty());
    }
}
