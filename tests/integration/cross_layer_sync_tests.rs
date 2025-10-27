//! Integration tests for cross-layer synchronization
//!
//! These tests verify the complete cross-layer validation workflow,
//! including content hash verification, version pinning, and equivalence proof validation.

use governance_app::error::GovernanceError;
use governance_app::validation::content_hash::{ContentHashValidator, SyncStatus};
use governance_app::validation::version_pinning::{VersionPinningValidator, VersionReference};
use governance_app::validation::equivalence_proof::{EquivalenceProofValidator, VerificationStatus};
use governance_app::github::cross_layer_status::{CrossLayerStatusChecker, StatusState};
use governance_app::github::client::GitHubClient;
use std::collections::HashMap;
use tokio;

/// Test content hash verification with real file data
#[tokio::test]
async fn test_content_hash_verification_integration() {
    let mut validator = ContentHashValidator::new();
    let correspondence_mappings = ContentHashValidator::generate_correspondence_map();
    validator.load_correspondence_mappings(correspondence_mappings);

    // Test data simulating Orange Paper and Consensus Proof files
    let mut orange_files = HashMap::new();
    orange_files.insert(
        "consensus-rules/block-validation.md".to_string(),
        "Block validation rules for Bitcoin consensus".to_string(),
    );
    orange_files.insert(
        "consensus-rules/transaction-validation.md".to_string(),
        "Transaction validation rules for Bitcoin consensus".to_string(),
    );

    let mut consensus_proof_files = HashMap::new();
    consensus_proof_files.insert(
        "proofs/block-validation.rs".to_string(),
        "Block validation rules for Bitcoin consensus".to_string(),
    );
    consensus_proof_files.insert(
        "proofs/transaction-validation.rs".to_string(),
        "Transaction validation rules for Bitcoin consensus".to_string(),
    );

    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(),
        "consensus-rules/transaction-validation.md".to_string(),
    ];

    // Test bidirectional sync
    let sync_report = validator
        .check_bidirectional_sync(&orange_files, &consensus_proof_files, &changed_files)
        .unwrap();

    assert_eq!(sync_report.sync_status, SyncStatus::Synchronized);
    assert_eq!(sync_report.changed_files.len(), 2);
    assert!(sync_report.missing_files.is_empty());
    assert!(sync_report.outdated_files.is_empty());
}

/// Test version pinning validation with real version data
#[tokio::test]
async fn test_version_pinning_validation_integration() {
    let mut validator = VersionPinningValidator::default();

    // Create a mock version manifest
    let manifest = create_mock_version_manifest();
    validator.load_version_manifest(manifest).unwrap();

    // Test valid version reference
    let valid_reference = VersionReference {
        file_path: "src/validation.rs:3".to_string(),
        orange_paper_version: "v1.0.0".to_string(),
        orange_paper_commit: "a1b2c3d4e5f6789012345678901234567890abcd".to_string(),
        orange_paper_hash: "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    };

    let result = validator.verify_version_reference(&valid_reference);
    assert!(result.is_ok());

    // Test invalid version reference
    let invalid_reference = VersionReference {
        file_path: "src/validation.rs:3".to_string(),
        orange_paper_version: "v9.9.9".to_string(),
        orange_paper_commit: "a1b2c3d4e5f6789012345678901234567890abcd".to_string(),
        orange_paper_hash: "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    };

    let result = validator.verify_version_reference(&invalid_reference);
    assert!(result.is_err());
}

/// Test equivalence proof validation with real test vectors
#[tokio::test]
async fn test_equivalence_proof_validation_integration() {
    let mut validator = EquivalenceProofValidator::new();
    let test_vectors = EquivalenceProofValidator::generate_consensus_test_vectors();
    validator.load_test_vectors(test_vectors);

    // Test block validation equivalence
    let result = validator.verify_equivalence_proof("block_validation_001").unwrap();
    assert_eq!(result.overall_status, VerificationStatus::Verified);
    assert!(result.errors.is_empty());

    // Test transaction validation equivalence
    let result = validator.verify_equivalence_proof("tx_validation_001").unwrap();
    assert_eq!(result.overall_status, VerificationStatus::Verified);
    assert!(result.errors.is_empty());

    // Test script execution equivalence
    let result = validator.verify_equivalence_proof("script_execution_001").unwrap();
    assert_eq!(result.overall_status, VerificationStatus::Verified);
    assert!(result.errors.is_empty());
}

/// Test cross-layer status check generation
#[tokio::test]
async fn test_cross_layer_status_generation_integration() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(),
        "proofs/block-validation.rs".to_string(),
        "consensus-rules/transaction-validation.md".to_string(),
        "proofs/transaction-validation.rs".to_string(),
    ];

    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 123, &changed_files)
        .await
        .unwrap();

    assert_eq!(status.context, "cross-layer-sync");
    assert!(status.target_url.is_some());
    assert!(!status.details.recommendations.is_empty());

    // Verify status details
    assert!(status.details.content_hash_status.files_checked > 0);
    assert!(status.details.version_pinning_status.references_checked >= 0);
    assert!(status.details.equivalence_proof_status.tests_run >= 0);
}

/// Test cross-layer validation with mixed results
#[tokio::test]
async fn test_cross_layer_validation_mixed_results() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    // Files that will trigger mixed results (some pass, some fail)
    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(), // Will fail content hash
        "consensus-rules/script-execution.md".to_string(), // Will fail equivalence proof
        "consensus-rules/transaction-validation.md".to_string(), // Will pass
    ];

    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 124, &changed_files)
        .await
        .unwrap();

    // Should be failure due to mixed results
    assert_eq!(status.state, StatusState::Failure);
    assert!(status.description.contains("❌"));

    // Should have recommendations for fixing issues
    assert!(!status.details.recommendations.is_empty());
    assert!(status.details.recommendations.iter().any(|r| r.contains("Update corresponding")));
}

/// Test cross-layer validation with all failures
#[tokio::test]
async fn test_cross_layer_validation_all_failures() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    // Files that will trigger all failures
    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(), // Will fail content hash
        "consensus-rules/script-execution.md".to_string(), // Will fail equivalence proof
    ];

    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 125, &changed_files)
        .await
        .unwrap();

    // Should be failure
    assert_eq!(status.state, StatusState::Failure);
    assert!(status.description.contains("❌"));

    // Should have multiple recommendations
    assert!(status.details.recommendations.len() > 1);
}

/// Test cross-layer validation with all successes
#[tokio::test]
async fn test_cross_layer_validation_all_successes() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    // Files that will trigger all successes
    let changed_files = vec![
        "consensus-rules/transaction-validation.md".to_string(), // Will pass all checks
        "proofs/transaction-validation.rs".to_string(),
    ];

    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 126, &changed_files)
        .await
        .unwrap();

    // Should be success
    assert_eq!(status.state, StatusState::Success);
    assert!(status.description.contains("✅"));

    // Should have success recommendation
    assert!(status.details.recommendations.iter().any(|r| r.contains("Ready to merge")));
}

/// Test error handling for invalid inputs
#[tokio::test]
async fn test_cross_layer_validation_error_handling() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    // Empty changed files should still work
    let changed_files = vec![];
    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 127, &changed_files)
        .await
        .unwrap();

    assert_eq!(status.state, StatusState::Success);
    assert_eq!(status.details.content_hash_status.files_checked, 0);
}

/// Test performance with large number of files
#[tokio::test]
async fn test_cross_layer_validation_performance() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    // Generate large number of files
    let mut changed_files = Vec::new();
    for i in 0..100 {
        changed_files.push(format!("consensus-rules/rule-{}.md", i));
        changed_files.push(format!("proofs/rule-{}.rs", i));
    }

    let start = std::time::Instant::now();
    let status = checker
        .generate_cross_layer_status("test_owner", "test_repo", 128, &changed_files)
        .await
        .unwrap();
    let duration = start.elapsed();

    // Should complete within reasonable time (less than 5 seconds)
    assert!(duration.as_secs() < 5);
    assert_eq!(status.details.content_hash_status.files_checked, 200);
}

/// Test concurrent validation requests
#[tokio::test]
async fn test_cross_layer_validation_concurrent() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);

    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(),
        "proofs/block-validation.rs".to_string(),
    ];

    // Run multiple concurrent validations
    let futures: Vec<_> = (0..10)
        .map(|i| {
            let mut checker = CrossLayerStatusChecker::new(GitHubClient::new("test_token".to_string()));
            async move {
                checker
                    .generate_cross_layer_status("test_owner", "test_repo", 200 + i, &changed_files)
                    .await
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    // All should succeed
    for result in results {
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.context, "cross-layer-sync");
    }
}

/// Helper function to create mock version manifest
fn create_mock_version_manifest() -> governance_app::validation::version_pinning::VersionManifest {
    use governance_app::validation::version_pinning::{VersionManifest, VersionManifestEntry, VersionSignature};
    use chrono::Utc;

    VersionManifest {
        repository: "orange-paper".to_string(),
        created_at: Utc::now(),
        versions: vec![
            VersionManifestEntry {
                version: "v1.0.0".to_string(),
                commit_sha: "a1b2c3d4e5f6789012345678901234567890abcd".to_string(),
                content_hash: "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
                created_at: Utc::now() - chrono::Duration::days(1),
                signatures: vec![
                    VersionSignature {
                        maintainer_id: "maintainer1".to_string(),
                        signature: "test_signature_1".to_string(),
                        public_key: "test_public_key_1".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                    VersionSignature {
                        maintainer_id: "maintainer2".to_string(),
                        signature: "test_signature_2".to_string(),
                        public_key: "test_public_key_2".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                    VersionSignature {
                        maintainer_id: "maintainer3".to_string(),
                        signature: "test_signature_3".to_string(),
                        public_key: "test_public_key_3".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                    VersionSignature {
                        maintainer_id: "maintainer4".to_string(),
                        signature: "test_signature_4".to_string(),
                        public_key: "test_public_key_4".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                    VersionSignature {
                        maintainer_id: "maintainer5".to_string(),
                        signature: "test_signature_5".to_string(),
                        public_key: "test_public_key_5".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                    VersionSignature {
                        maintainer_id: "maintainer6".to_string(),
                        signature: "test_signature_6".to_string(),
                        public_key: "test_public_key_6".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    },
                ],
                ots_timestamp: Some("bitcoin:test_timestamp".to_string()),
                is_stable: true,
                is_latest: true,
            }
        ],
        latest_version: "v1.0.0".to_string(),
        manifest_hash: "sha256:test_manifest_hash".to_string(),
    }
}

