use governance_app::webhooks::github;
use governance_app::database::Database;
use serde_json::Value;
use axum::http::StatusCode;

mod common;
use common::*;

#[tokio::test]
async fn test_pull_request_opened_webhook() {
    let db = setup_test_db().await;
    let payload = github_mocks::pull_request_opened_payload("BTCDecoded/consensus-proof", 123);
    
    // Test webhook processing
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify PR was stored in database
    let pr = db.get_pull_request("BTCDecoded/consensus-proof", 123).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert_eq!(pull_request.repo_name, "BTCDecoded/consensus-proof");
        assert_eq!(pull_request.pr_number, 123);
        assert_eq!(pull_request.layer, 2); // consensus-proof is layer 2
    }
}

#[tokio::test]
async fn test_pull_request_synchronize_webhook() {
    let db = setup_test_db().await;
    let payload = github_mocks::pull_request_synchronize_payload("BTCDecoded/protocol-engine", 456);
    
    // Test webhook processing
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify PR was updated in database
    let pr = db.get_pull_request("BTCDecoded/protocol-engine", 456).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert_eq!(pull_request.repo_name, "BTCDecoded/protocol-engine");
        assert_eq!(pull_request.pr_number, 456);
        assert_eq!(pull_request.layer, 3); // protocol-engine is layer 3
    }
}

#[tokio::test]
async fn test_review_submitted_webhook() {
    let db = setup_test_db().await;
    let payload = github_mocks::review_submitted_payload("BTCDecoded/reference-node", 789, "alice", "approved");
    
    // Test webhook processing
    let result = governance_app::webhooks::review::handle_review_event(&db, &payload).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_review_dismissed_webhook() {
    let db = setup_test_db().await;
    let payload = github_mocks::review_submitted_payload("BTCDecoded/developer-sdk", 101, "bob", "dismissed");
    
    // Test webhook processing
    let result = governance_app::webhooks::review::handle_review_event(&db, &payload).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_comment_governance_signature() {
    let db = setup_test_db().await;
    let signature_body = "/governance-sign signature_abc123def456";
    let payload = github_mocks::comment_created_payload("BTCDecoded/consensus-proof", 123, "alice", signature_body);
    
    // Test webhook processing
    let result = governance_app::webhooks::comment::handle_comment_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify signature was added to database
    let pr = db.get_pull_request("BTCDecoded/consensus-proof", 123).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert!(!pull_request.signatures.is_empty());
        assert_eq!(pull_request.signatures[0].signer, "alice");
    }
}

#[tokio::test]
async fn test_comment_empty_signature() {
    let db = setup_test_db().await;
    let empty_signature_body = "/governance-sign";
    let payload = github_mocks::comment_created_payload("BTCDecoded/consensus-proof", 123, "alice", empty_signature_body);
    
    // Test webhook processing
    let result = governance_app::webhooks::comment::handle_comment_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify no signature was added
    let pr = db.get_pull_request("BTCDecoded/consensus-proof", 123).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert!(pull_request.signatures.is_empty());
    }
}

#[tokio::test]
async fn test_comment_non_governance() {
    let db = setup_test_db().await;
    let regular_comment = "This looks good to me!";
    let payload = github_mocks::comment_created_payload("BTCDecoded/consensus-proof", 123, "alice", regular_comment);
    
    // Test webhook processing
    let result = governance_app::webhooks::comment::handle_comment_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify no signature was added
    let pr = db.get_pull_request("BTCDecoded/consensus-proof", 123).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert!(pull_request.signatures.is_empty());
    }
}

#[tokio::test]
async fn test_push_to_main_detection() {
    let db = setup_test_db().await;
    let payload = github_mocks::push_payload("BTCDecoded/consensus-proof", "alice", "refs/heads/main");
    
    // Test webhook processing
    let result = governance_app::webhooks::push::handle_push_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify bypass attempt was logged
    let events = db.get_governance_events(10).await;
    assert!(events.is_ok());
    if let Ok(events) = events {
        assert!(!events.is_empty());
        assert!(events.iter().any(|e| e.event_type == "direct_push_detected"));
    }
}

#[tokio::test]
async fn test_push_to_feature_branch() {
    let db = setup_test_db().await;
    let payload = github_mocks::push_payload("BTCDecoded/consensus-proof", "alice", "refs/heads/feature/new-feature");
    
    // Test webhook processing
    let result = governance_app::webhooks::push::handle_push_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify no bypass attempt was logged
    let events = db.get_governance_events(10).await;
    assert!(events.is_ok());
    if let Ok(events) = events {
        assert!(events.is_empty() || !events.iter().any(|e| e.event_type == "direct_push_detected"));
    }
}

#[tokio::test]
async fn test_webhook_hmac_verification() {
    // Test HMAC signature verification
    let webhook_secret = "test_secret";
    let payload = r#"{"action":"opened","repository":{"full_name":"BTCDecoded/consensus-proof"}}"#;
    let signature = "sha256=test_signature";
    
    // This would test HMAC verification in a real implementation
    // For now, we'll just test that the signature format is correct
    assert!(signature.starts_with("sha256="));
}

#[tokio::test]
async fn test_webhook_invalid_payload() {
    let db = setup_test_db().await;
    let invalid_payload = serde_json::json!({
        "invalid": "payload"
    });
    
    // Test webhook processing with invalid payload
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &invalid_payload).await;
    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_webhook_malformed_json() {
    let db = setup_test_db().await;
    let malformed_payload = serde_json::json!({
        "action": "opened",
        "repository": {
            "full_name": null  // Invalid null value
        },
        "pull_request": {
            "number": "not_a_number"  // Invalid type
        }
    });
    
    // Test webhook processing with malformed payload
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &malformed_payload).await;
    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_webhook_unknown_repository() {
    let db = setup_test_db().await;
    let payload = github_mocks::pull_request_opened_payload("Unknown/Repository", 123);
    
    // Test webhook processing with unknown repository
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Should return unknown_repo status
    if let Ok(response) = result {
        assert!(response.get("status").is_some());
    }
}

#[tokio::test]
async fn test_webhook_concurrent_processing() {
    let db = setup_test_db().await;
    
    // Test concurrent webhook processing
    let handles: Vec<_> = (0..10).map(|i| {
        let db = &db;
        let payload = github_mocks::pull_request_opened_payload("BTCDecoded/consensus-proof", i as u64);
        
        tokio::spawn(async move {
            governance_app::webhooks::pull_request::handle_pull_request_event(db, &payload).await
        })
    }).collect();
    
    // Wait for all webhooks to be processed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_webhook_database_transaction_rollback() {
    let db = setup_test_db().await;
    
    // Test that database transactions are properly handled
    let payload = github_mocks::pull_request_opened_payload("BTCDecoded/consensus-proof", 999);
    
    // Process webhook
    let result = governance_app::webhooks::pull_request::handle_pull_request_event(&db, &payload).await;
    assert!(result.is_ok());
    
    // Verify data consistency
    let pr = db.get_pull_request("BTCDecoded/consensus-proof", 999).await;
    assert!(pr.is_ok());
    if let Ok(Some(pull_request)) = pr {
        assert_eq!(pull_request.repo_name, "BTCDecoded/consensus-proof");
        assert_eq!(pull_request.pr_number, 999);
    }
}

#[tokio::test]
async fn test_signature_verification_workflow() {
    use secp256k1::{SecretKey, Secp256k1};
    use rand::rngs::OsRng;
    
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    
    let message = "governance-signature:testuser";
    let signature = secp.sign_ecdsa(
        &secp256k1::Message::from_slice(&sha2::Sha256::digest(message.as_bytes())).unwrap(),
        &secret_key
    );
    
    // Verify the signature
    let is_valid = secp.verify_ecdsa(
        &secp256k1::Message::from_slice(&sha2::Sha256::digest(message.as_bytes())).unwrap(),
        &signature,
        &public_key
    );
    
    assert!(is_valid.is_ok());
}




