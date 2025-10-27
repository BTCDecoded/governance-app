use governance_app::database::Database;
use governance_app::database::models::*;
use chrono::{DateTime, Utc, Duration};
use serde_json::json;

mod common;
use common::*;

#[tokio::test]
async fn test_database_creation_and_migration() {
    let db = setup_test_db().await;
    
    // Test that database was created successfully
    assert!(db.pool.is_closed() == false);
}

#[tokio::test]
async fn test_pull_request_crud() {
    let db = setup_test_db().await;
    
    // Test creating a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    let result = db.create_pull_request(repo_name, pr_number, head_sha, layer).await;
    assert!(result.is_ok());
    
    // Test retrieving the pull request
    let pr = db.get_pull_request(repo_name, pr_number).await;
    assert!(pr.is_ok());
    
    if let Ok(Some(pull_request)) = pr {
        assert_eq!(pull_request.repo_name, repo_name);
        assert_eq!(pull_request.pr_number, pr_number);
        assert_eq!(pull_request.head_sha, head_sha);
        assert_eq!(pull_request.layer, layer);
        assert_eq!(pull_request.governance_status, "pending");
    }
}

#[tokio::test]
async fn test_signature_storage_and_retrieval() {
    let db = setup_test_db().await;
    
    // Create a pull request first
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Add signatures
    let signer1 = "alice";
    let signature1 = "signature_alice_123";
    let signer2 = "bob";
    let signature2 = "signature_bob_456";
    
    db.add_signature(repo_name, pr_number, signer1, signature1).await.unwrap();
    db.add_signature(repo_name, pr_number, signer2, signature2).await.unwrap();
    
    // Retrieve and verify signatures
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    assert_eq!(pr.signatures.len(), 2);
    
    let signers: Vec<String> = pr.signatures.iter().map(|s| s.signer.clone()).collect();
    assert!(signers.contains(&signer1.to_string()));
    assert!(signers.contains(&signer2.to_string()));
}

#[tokio::test]
async fn test_governance_event_logging() {
    let db = setup_test_db().await;
    
    let event_type = "signature_added";
    let repo_name = Some("BTCDecoded/consensus-proof");
    let pr_number = Some(123);
    let maintainer = Some("alice");
    let details = json!({
        "signature": "signature_alice_123",
        "timestamp": Utc::now()
    });
    
    let result = db.log_governance_event(
        event_type,
        repo_name.as_deref(),
        pr_number,
        maintainer.as_deref(),
        &details,
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_signature_additions() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Add signatures concurrently
    let handles: Vec<_> = (0..5).map(|i| {
        let db = &db;
        let repo_name = repo_name.to_string();
        let pr_number = pr_number;
        let signer = format!("signer{}", i);
        let signature = format!("signature_{}", i);
        
        tokio::spawn(async move {
            db.add_signature(&repo_name, pr_number, &signer, &signature).await
        })
    }).collect();
    
    // Wait for all signatures to be added
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
    
    // Verify all signatures were added
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    assert_eq!(pr.signatures.len(), 5);
}

#[tokio::test]
async fn test_review_status_updates() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Update review status
    let reviewer = "alice";
    let state = "approved";
    
    let result = db.update_review_status(repo_name, pr_number, reviewer, state).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_emergency_mode_tracking() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Test emergency mode activation
    let emergency_details = json!({
        "activated_by": "emergency_alice",
        "reason": "Critical security vulnerability",
        "timestamp": Utc::now()
    });
    
    let result = db.log_governance_event(
        "emergency_mode_activated",
        Some(repo_name),
        Some(pr_number),
        Some("emergency_alice"),
        &emergency_details,
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cross_layer_dependencies() {
    let db = setup_test_db().await;
    
    // Test cross-layer rule storage and retrieval
    let rule_data = json!({
        "source_repo": "BTCDecoded/consensus-proof",
        "source_pattern": "src/consensus/**",
        "target_repo": "BTCDecoded/protocol-engine",
        "target_pattern": "src/validation/**",
        "validation_type": "corresponding_file_exists"
    });
    
    let result = db.log_governance_event(
        "cross_layer_rule_created",
        None,
        None,
        None,
        &rule_data,
    ).await;
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_migration_execution() {
    // Test that migrations run successfully
    let db = setup_test_db().await;
    
    // If we get here without error, migrations executed successfully
    assert!(db.pool.is_closed() == false);
}

#[tokio::test]
async fn test_json_serialization_for_signatures() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Add a signature with complex JSON data
    let complex_signature = json!({
        "signer": "alice",
        "signature": "signature_alice_123",
        "timestamp": Utc::now(),
        "metadata": {
            "key_id": "key_123",
            "algorithm": "secp256k1",
            "version": "1.0"
        }
    });
    
    let signature_str = serde_json::to_string(&complex_signature).unwrap();
    let result = db.add_signature(repo_name, pr_number, "alice", &signature_str).await;
    assert!(result.is_ok());
    
    // Verify the signature was stored correctly
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    assert_eq!(pr.signatures.len(), 1);
    
    let stored_signature = &pr.signatures[0];
    assert_eq!(stored_signature.signer, "alice");
}

#[tokio::test]
async fn test_database_connection_pooling() {
    // Test that multiple operations can run concurrently
    let db = setup_test_db().await;
    
    let handles: Vec<_> = (0..10).map(|i| {
        let db = &db;
        let repo_name = format!("BTCDecoded/repo{}", i);
        let pr_number = i as i32;
        let head_sha = format!("sha{}", i);
        let layer = (i % 5) + 1;
        
        tokio::spawn(async move {
            db.create_pull_request(&repo_name, pr_number, &head_sha, layer).await
        })
    }).collect();
    
    // Wait for all operations to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}











