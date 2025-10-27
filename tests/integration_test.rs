//! Integration tests for governance-app
//! Tests the core functionality with test data

use governance_app::database::Database;
use governance_app::validation::tier_classification;
use serde_json::json;

#[tokio::test]
async fn test_tier_classification() {
    // Test Tier 1: Routine maintenance
    let routine_payload = json!({
        "pull_request": {
            "title": "Fix typo in README",
            "body": "Simple documentation fix"
        }
    });

    let tier = tier_classification::classify_pr_tier(&routine_payload).await;
    assert_eq!(tier, 1, "Documentation changes should be Tier 1");

    // Test Tier 4: Emergency
    let emergency_payload = json!({
        "pull_request": {
            "title": "EMERGENCY: Critical security fix",
            "body": "This is a critical security vulnerability"
        }
    });

    let tier = tier_classification::classify_pr_tier(&emergency_payload).await;
    assert_eq!(tier, 4, "Emergency keywords should be Tier 4");
}

#[tokio::test]
async fn test_database_operations() {
    // Create in-memory database for testing
    let db = Database::new_in_memory()
        .await
        .expect("Failed to create database");

    // Test creating a pull request
    let result = db.create_pull_request("test/repo", 123, "abc123", 1).await;
    assert!(result.is_ok(), "Should be able to create pull request");

    // Test logging governance event
    let result = db
        .log_governance_event(
            "test_event",
            Some("test/repo"),
            Some(123),
            Some("test_user"),
            &json!({"test": "data"}),
        )
        .await;
    assert!(result.is_ok(), "Should be able to log governance event");
}

#[tokio::test]
async fn test_signature_verification() {
    use developer_sdk::governance::GovernanceKeypair;
    use governance_app::crypto::signatures::SignatureManager;

    // Generate test keypair
    let keypair = GovernanceKeypair::generate().expect("Failed to generate keypair");
    let signature_manager = SignatureManager::new();

    // Test signature creation and verification
    let message = "test message";
    let signature = signature_manager
        .create_governance_signature(message, &keypair)
        .expect("Failed to create signature");

    let public_key = keypair.public_key().to_string();
    let verified = signature_manager
        .verify_governance_signature(message, &signature, &public_key)
        .expect("Failed to verify signature");

    assert!(verified, "Signature should be valid");

    // Test with wrong message
    let wrong_message = "wrong message";
    let verified_wrong = signature_manager
        .verify_governance_signature(wrong_message, &signature, &public_key)
        .expect("Failed to verify signature");

    assert!(
        !verified_wrong,
        "Signature should be invalid for wrong message"
    );
}




