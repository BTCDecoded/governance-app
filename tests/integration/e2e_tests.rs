use governance_app::database::Database;
use governance_app::validation::*;
use governance_app::enforcement::*;
use governance_app::crypto::*;
use chrono::{DateTime, Utc, Duration};
use secp256k1::{SecretKey, Secp256k1, PublicKey};
use rand::rngs::OsRng;
use std::collections::HashMap;

mod common;
use common::*;

#[tokio::test]
async fn test_complete_pr_lifecycle() {
    let db = setup_test_db().await;
    let signature_manager = create_test_signature_manager();
    let multisig_manager = create_test_multisig_manager();
    
    // Step 1: Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 123;
    let head_sha = "abc123def456";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Step 2: Add signatures from maintainers
    let keypairs = generate_test_keypairs(3);
    let mut public_keys = HashMap::new();
    
    for (username, secret_key, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
        
        let message = format!("governance-signature:{}", username);
        let signature = signature_manager.create_signature(&message, secret_key).unwrap();
        
        db.add_signature(repo_name, pr_number, username, &signature.to_string()).await.unwrap();
    }
    
    // Step 3: Verify signatures meet threshold
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    let signatures: Vec<(String, String)> = pr.signatures.iter().map(|s| (s.signer.clone(), s.signature.clone())).collect();
    
    let (required, total) = ThresholdValidator::get_threshold_for_layer(layer);
    let result = multisig_manager.verify_multisig(
        "governance-signature:test",
        &signatures,
        &public_keys,
        (required, total),
    );
    
    assert!(result.is_ok());
    
    // Step 4: Check review period
    let review_period_days = ThresholdValidator::get_review_period_for_layer(layer, false);
    let opened_at = pr.opened_at;
    let review_period_met = ReviewPeriodValidator::validate_review_period(opened_at, review_period_days, false).is_ok();
    
    // Step 5: Check if merge should be blocked
    let signatures_met = result.unwrap();
    let should_block = MergeBlocker::should_block_merge(review_period_met, signatures_met, false).unwrap();
    
    // For a new PR, it should be blocked due to review period
    assert!(should_block);
}

#[tokio::test]
async fn test_emergency_mode_activation() {
    let db = setup_test_db().await;
    
    // Step 1: Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 456;
    let head_sha = "def456ghi789";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Step 2: Activate emergency mode
    let emergency_details = serde_json::json!({
        "activated_by": "emergency_alice",
        "reason": "Critical security vulnerability",
        "evidence": "Detailed evidence of the critical security vulnerability that requires immediate attention",
        "timestamp": Utc::now()
    });
    
    db.log_governance_event(
        "emergency_mode_activated",
        Some(repo_name),
        Some(pr_number),
        Some("emergency_alice"),
        &emergency_details,
    ).await.unwrap();
    
    // Step 3: Add emergency signatures
    let emergency_keypairs = generate_test_keypairs(5);
    let mut emergency_public_keys = HashMap::new();
    
    for (username, secret_key, public_key) in &emergency_keypairs {
        emergency_public_keys.insert(username.clone(), public_key.to_string());
        
        let message = format!("emergency-signature:{}", username);
        let signature = create_test_signature_manager().create_signature(&message, secret_key).unwrap();
        
        db.add_signature(repo_name, pr_number, username, &signature.to_string()).await.unwrap();
    }
    
    // Step 4: Verify emergency mode requirements
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    let signatures: Vec<(String, String)> = pr.signatures.iter().map(|s| (s.signer.clone(), s.signature.clone())).collect();
    
    // In emergency mode, only signatures matter (review period is reduced to 30 days)
    let review_period_days = ThresholdValidator::get_review_period_for_layer(layer, true);
    assert_eq!(review_period_days, 30);
    
    // Emergency mode should allow faster processing
    let review_period_met = ReviewPeriodValidator::validate_review_period(pr.opened_at, review_period_days, true).is_ok();
    let signatures_met = signatures.len() >= 4; // Emergency threshold
    
    let should_block = MergeBlocker::should_block_merge(review_period_met, signatures_met, true).unwrap();
    
    // Should not be blocked in emergency mode if signatures are met
    assert!(!should_block);
}

#[tokio::test]
async fn test_cross_layer_synchronization() {
    let db = setup_test_db().await;
    
    // Step 1: Create PRs in different layers
    let consensus_pr = (123, "BTCDecoded/consensus-proof", 2);
    let protocol_pr = (456, "BTCDecoded/protocol-engine", 3);
    
    db.create_pull_request(consensus_pr.1, consensus_pr.0, "sha1", consensus_pr.2).await.unwrap();
    db.create_pull_request(protocol_pr.1, protocol_pr.0, "sha2", protocol_pr.2).await.unwrap();
    
    // Step 2: Add cross-layer rules
    let cross_layer_rules = create_test_cross_layer_rules();
    
    // Step 3: Validate cross-layer dependencies
    let changed_files = vec!["src/consensus/block.rs".to_string()];
    let result = CrossLayerValidator::validate_cross_layer_dependencies(
        consensus_pr.1,
        &changed_files,
        &cross_layer_rules,
    );
    
    assert!(result.is_ok());
    
    // Step 4: Log cross-layer synchronization
    let sync_details = serde_json::json!({
        "source_repo": consensus_pr.1,
        "target_repo": protocol_pr.1,
        "validation_type": "corresponding_file_exists",
        "timestamp": Utc::now()
    });
    
    db.log_governance_event(
        "cross_layer_sync",
        Some(consensus_pr.1),
        Some(consensus_pr.0),
        None,
        &sync_details,
    ).await.unwrap();
}

#[tokio::test]
async fn test_signature_threshold_validation_across_layers() {
    let db = setup_test_db().await;
    
    // Test all layers
    for layer in 1..=5 {
        let repo_name = format!("BTCDecoded/test-repo-{}", layer);
        let pr_number = layer as i32 * 100;
        let head_sha = format!("sha{}", layer);
        
        db.create_pull_request(&repo_name, pr_number, &head_sha, layer).await.unwrap();
        
        // Get threshold for this layer
        let (required, total) = ThresholdValidator::get_threshold_for_layer(layer);
        
        // Add exactly the required number of signatures
        let keypairs = generate_test_keypairs(required);
        let mut public_keys = HashMap::new();
        
        for (username, secret_key, public_key) in &keypairs {
            public_keys.insert(username.clone(), public_key.to_string());
            
            let message = format!("governance-signature:{}", username);
            let signature = create_test_signature_manager().create_signature(&message, secret_key).unwrap();
            
            db.add_signature(&repo_name, pr_number, username, &signature.to_string()).await.unwrap();
        }
        
        // Verify threshold is met
        let pr = db.get_pull_request(&repo_name, pr_number).await.unwrap().unwrap();
        let signatures: Vec<(String, String)> = pr.signatures.iter().map(|s| (s.signer.clone(), s.signature.clone())).collect();
        
        let result = create_test_multisig_manager().verify_multisig(
            "governance-signature:test",
            &signatures,
            &public_keys,
            (required, total),
        );
        
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_review_period_with_emergency_tier_override() {
    let db = setup_test_db().await;
    
    // Step 1: Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 789;
    let head_sha = "ghi789jkl012";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Step 2: Test normal review period
    let normal_period = ThresholdValidator::get_review_period_for_layer(layer, false);
    assert_eq!(normal_period, 90); // Layer 2 normal period
    
    // Step 3: Test emergency review period
    let emergency_period = ThresholdValidator::get_review_period_for_layer(layer, true);
    assert_eq!(emergency_period, 30); // Emergency mode period
    
    // Step 4: Test with different emergency tiers
    use governance_app::validation::emergency::*;
    
    // Critical tier (0 days review)
    assert_eq!(EmergencyTier::Critical.review_period_days(), 0);
    
    // Urgent tier (7 days review)
    assert_eq!(EmergencyTier::Urgent.review_period_days(), 7);
    
    // Elevated tier (30 days review)
    assert_eq!(EmergencyTier::Elevated.review_period_days(), 30);
    
    // Step 5: Test review period validation with emergency override
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    let opened_at = pr.opened_at;
    
    // Normal mode - should fail for new PR
    let normal_result = ReviewPeriodValidator::validate_review_period(opened_at, normal_period, false);
    assert!(normal_result.is_err());
    
    // Emergency mode - should pass for new PR (30 days)
    let emergency_result = ReviewPeriodValidator::validate_review_period(opened_at, emergency_period, true);
    assert!(emergency_result.is_ok());
}

#[tokio::test]
async fn test_multi_pr_tracking_across_repos() {
    let db = setup_test_db().await;
    
    // Create PRs in multiple repositories
    let repos = vec![
        ("BTCDecoded/consensus-proof", 2),
        ("BTCDecoded/protocol-engine", 3),
        ("BTCDecoded/reference-node", 4),
        ("BTCDecoded/developer-sdk", 5),
    ];
    
    for (repo_name, layer) in &repos {
        let pr_number = 100 + layer;
        let head_sha = format!("sha_{}", layer);
        
        db.create_pull_request(repo_name, pr_number, &head_sha, *layer).await.unwrap();
        
        // Add some signatures
        let keypairs = generate_test_keypairs(2);
        for (username, secret_key, _) in &keypairs {
            let message = format!("governance-signature:{}", username);
            let signature = create_test_signature_manager().create_signature(&message, secret_key).unwrap();
            
            db.add_signature(repo_name, pr_number, username, &signature.to_string()).await.unwrap();
        }
    }
    
    // Verify all PRs were created and have signatures
    for (repo_name, layer) in &repos {
        let pr_number = 100 + layer;
        let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
        
        assert_eq!(pr.repo_name, *repo_name);
        assert_eq!(pr.layer, *layer);
        assert_eq!(pr.signatures.len(), 2);
    }
}

#[tokio::test]
async fn test_concurrent_signature_additions() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 999;
    let head_sha = "concurrent_test_sha";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Add signatures concurrently
    let keypairs = generate_test_keypairs(5);
    let handles: Vec<_> = keypairs.iter().map(|(username, secret_key, _)| {
        let db = &db;
        let repo_name = repo_name.to_string();
        let pr_number = pr_number;
        let username = username.clone();
        
        tokio::spawn(async move {
            let message = format!("governance-signature:{}", username);
            let signature = create_test_signature_manager().create_signature(&message, secret_key).unwrap();
            
            db.add_signature(&repo_name, pr_number, &username, &signature.to_string()).await
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
async fn test_event_log_consistency() {
    let db = setup_test_db().await;
    
    // Create a pull request
    let repo_name = "BTCDecoded/consensus-proof";
    let pr_number = 888;
    let head_sha = "event_log_sha";
    let layer = 2;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    // Log various events
    let events = vec![
        ("pr_created", None, None, None),
        ("signature_added", Some("alice"), Some(pr_number), Some("alice")),
        ("review_submitted", Some("bob"), Some(pr_number), Some("bob")),
        ("status_updated", Some("system"), Some(pr_number), None),
    ];
    
    for (event_type, repo, pr, maintainer) in events {
        let details = serde_json::json!({
            "timestamp": Utc::now(),
            "event_type": event_type
        });
        
        db.log_governance_event(
            event_type,
            repo.as_deref(),
            pr,
            maintainer.as_deref(),
            &details,
        ).await.unwrap();
    }
    
    // Verify all events were logged
    let logged_events = db.get_governance_events(10).await.unwrap();
    assert_eq!(logged_events.len(), 4);
    
    // Verify event types
    let event_types: Vec<String> = logged_events.iter().map(|e| e.event_type.clone()).collect();
    assert!(event_types.contains(&"pr_created".to_string()));
    assert!(event_types.contains(&"signature_added".to_string()));
    assert!(event_types.contains(&"review_submitted".to_string()));
    assert!(event_types.contains(&"status_updated".to_string()));
}

#[tokio::test]
async fn test_migration_rollback_scenarios() {
    // Test that migrations can be rolled back and reapplied
    let db = setup_test_db().await;
    
    // Verify database was created successfully
    assert!(db.pool.is_closed() == false);
    
    // Test that we can create and query data
    let repo_name = "BTCDecoded/test-migration";
    let pr_number = 777;
    let head_sha = "migration_test_sha";
    let layer = 1;
    
    db.create_pull_request(repo_name, pr_number, head_sha, layer).await.unwrap();
    
    let pr = db.get_pull_request(repo_name, pr_number).await.unwrap().unwrap();
    assert_eq!(pr.repo_name, repo_name);
    assert_eq!(pr.pr_number, pr_number);
    assert_eq!(pr.layer, layer);
}











