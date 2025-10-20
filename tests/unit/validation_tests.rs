use governance_app::validation::*;
use governance_app::validation::signatures::SignatureValidator;
use governance_app::validation::cross_layer::CrossLayerValidator;
use governance_app::validation::emergency::*;
use chrono::{DateTime, Utc, Duration};
use secp256k1::{SecretKey, Secp256k1, PublicKey};
use rand::rngs::OsRng;
use std::collections::HashMap;
use serde_json::Value;

mod common;
use common::*;

#[tokio::test]
async fn test_review_period_validation() {
    let now = Utc::now();
    let opened_at = now - Duration::days(100); // 100 days ago
    
    // Test normal mode
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, false);
    assert!(result.is_ok());
    
    // Test emergency mode
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, true);
    assert!(result.is_ok());
    
    // Test insufficient time
    let opened_recently = now - Duration::days(10);
    let result = ReviewPeriodValidator::validate_review_period(opened_recently, 90, false);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_review_period_edge_cases() {
    let now = Utc::now();
    
    // Test exactly at the boundary
    let opened_at = now - Duration::days(90);
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, false);
    assert!(result.is_ok());
    
    // Test one day before boundary
    let opened_at = now - Duration::days(89);
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, false);
    assert!(result.is_err());
    
    // Test emergency mode with 30-day requirement
    let opened_at = now - Duration::days(30);
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, true);
    assert!(result.is_ok());
    
    let opened_at = now - Duration::days(29);
    let result = ReviewPeriodValidator::validate_review_period(opened_at, 90, true);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_review_period_calculations() {
    let now = Utc::now();
    let opened_at = now - Duration::days(50);
    
    // Test earliest merge date calculation
    let earliest_merge = ReviewPeriodValidator::get_earliest_merge_date(opened_at, 90, false);
    let expected = opened_at + Duration::days(90);
    assert_eq!(earliest_merge, expected);
    
    // Test remaining days calculation
    let remaining = ReviewPeriodValidator::get_remaining_days(opened_at, 90, false);
    assert_eq!(remaining, 40);
    
    // Test emergency mode calculations
    let earliest_merge_emergency = ReviewPeriodValidator::get_earliest_merge_date(opened_at, 90, true);
    let expected_emergency = opened_at + Duration::days(30);
    assert_eq!(earliest_merge_emergency, expected_emergency);
}

#[tokio::test]
async fn test_threshold_validation() {
    // Test valid threshold
    let result = ThresholdValidator::validate_threshold(5, 4, 7);
    assert!(result.is_ok());
    
    // Test invalid threshold
    let result = ThresholdValidator::validate_threshold(3, 4, 7);
    assert!(result.is_err());
    
    // Test layer-specific thresholds
    let (required, total) = ThresholdValidator::get_threshold_for_layer(1);
    assert_eq!((required, total), (6, 7));
    
    let (required, total) = ThresholdValidator::get_threshold_for_layer(3);
    assert_eq!((required, total), (4, 5));
}

#[tokio::test]
async fn test_threshold_all_layers() {
    // Test all layer thresholds
    for layer in 1..=5 {
        let (required, total) = ThresholdValidator::get_threshold_for_layer(layer);
        assert!(required <= total);
        assert!(required > 0);
        assert!(total > 0);
    }
    
    // Test review periods for all layers
    for layer in 1..=5 {
        let normal_period = ThresholdValidator::get_review_period_for_layer(layer, false);
        let emergency_period = ThresholdValidator::get_review_period_for_layer(layer, true);
        
        assert!(normal_period > 0);
        assert_eq!(emergency_period, 30); // All layers use 30 days in emergency mode
    }
}

#[tokio::test]
async fn test_threshold_status_formatting() {
    let signers = vec!["alice".to_string(), "bob".to_string()];
    let pending = vec!["charlie".to_string(), "dave".to_string()];
    
    let status = ThresholdValidator::format_threshold_status(2, 4, 5, &signers, &pending);
    assert!(status.contains("Required: 4-of-5"));
    assert!(status.contains("Current: 2/5"));
    assert!(status.contains("alice, bob"));
    assert!(status.contains("charlie, dave"));
}

#[tokio::test]
async fn test_signature_validation() {
    let validator = SignatureValidator::new();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    let message = "test message";
    let signature = secp.sign_ecdsa(
        &secp256k1::Message::from_slice(&sha2::Sha256::digest(message.as_bytes())).unwrap(),
        &secret_key
    );
    
    // Test valid signature
    let result = validator.verify_signature(message, &signature.to_string(), &public_key.to_string());
    assert!(result.is_ok());
    assert!(result.unwrap());
    
    // Test invalid signature
    let wrong_message = "wrong message";
    let result = validator.verify_signature(wrong_message, &signature.to_string(), &public_key.to_string());
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_signature_validation_edge_cases() {
    let validator = SignatureValidator::new();
    
    // Test malformed public key
    let result = validator.verify_signature("message", "signature", "invalid_key");
    assert!(result.is_err());
    
    // Test malformed signature
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    let result = validator.verify_signature("message", "invalid_signature", &public_key.to_string());
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multisig_threshold_validation() {
    let validator = SignatureValidator::new();
    let secp = Secp256k1::new();
    
    // Create test keypairs
    let keypairs = generate_test_keypairs(3);
    let mut maintainer_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        maintainer_keys.insert(username.clone(), public_key.to_string());
    }
    
    // Create signatures
    let message = "governance-signature:test";
    let signatures: Vec<(String, String)> = keypairs.iter().map(|(username, secret_key, _)| {
        let signature = secp.sign_ecdsa(
            &secp256k1::Message::from_slice(&sha2::Sha256::digest(message.as_bytes())).unwrap(),
            secret_key
        );
        (username.clone(), signature.to_string())
    }).collect();
    
    // Test 2-of-3 threshold
    let result = validator.verify_multisig_threshold(&signatures, (2, 3), &maintainer_keys);
    assert!(result.is_ok());
    assert!(result.unwrap());
    
    // Test insufficient signatures
    let insufficient_signatures = &signatures[0..1];
    let result = validator.verify_multisig_threshold(insufficient_signatures, (2, 3), &maintainer_keys);
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_cross_layer_validation() {
    let changed_files = vec![
        "src/consensus/block.rs".to_string(),
        "src/consensus/transaction.rs".to_string(),
    ];
    
    let cross_layer_rules = create_test_cross_layer_rules();
    
    // Test matching pattern
    let result = CrossLayerValidator::validate_cross_layer_dependencies(
        "BTCDecoded/consensus-proof",
        &changed_files,
        &cross_layer_rules,
    );
    assert!(result.is_ok());
    
    // Test non-matching pattern
    let non_matching_files = vec!["src/other/file.rs".to_string()];
    let result = CrossLayerValidator::validate_cross_layer_dependencies(
        "BTCDecoded/consensus-proof",
        &non_matching_files,
        &cross_layer_rules,
    );
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cross_layer_pattern_matching() {
    // Test glob pattern matching
    let files = vec!["src/consensus/block.rs".to_string()];
    
    // Test exact match
    assert!(CrossLayerValidator::matches_pattern(&files, "src/consensus/block.rs"));
    
    // Test wildcard match
    assert!(CrossLayerValidator::matches_pattern(&files, "src/consensus/*"));
    
    // Test double wildcard match
    assert!(CrossLayerValidator::matches_pattern(&files, "src/**"));
    
    // Test non-match
    assert!(!CrossLayerValidator::matches_pattern(&files, "src/other/*"));
}

#[tokio::test]
async fn test_emergency_tier_properties() {
    // Test Critical tier
    assert_eq!(EmergencyTier::Critical.review_period_days(), 0);
    assert_eq!(EmergencyTier::Critical.signature_threshold(), (4, 7));
    assert_eq!(EmergencyTier::Critical.max_duration_days(), 7);
    assert!(!EmergencyTier::Critical.allows_extensions());
    assert!(EmergencyTier::Critical.requires_security_audit());
    
    // Test Urgent tier
    assert_eq!(EmergencyTier::Urgent.review_period_days(), 7);
    assert_eq!(EmergencyTier::Urgent.signature_threshold(), (5, 7));
    assert_eq!(EmergencyTier::Urgent.max_duration_days(), 30);
    assert!(EmergencyTier::Urgent.allows_extensions());
    assert!(!EmergencyTier::Urgent.requires_security_audit());
    
    // Test Elevated tier
    assert_eq!(EmergencyTier::Elevated.review_period_days(), 30);
    assert_eq!(EmergencyTier::Elevated.signature_threshold(), (6, 7));
    assert_eq!(EmergencyTier::Elevated.max_duration_days(), 90);
    assert!(EmergencyTier::Elevated.allows_extensions());
    assert!(!EmergencyTier::Elevated.requires_security_audit());
}

#[tokio::test]
async fn test_emergency_tier_parsing() {
    assert_eq!(EmergencyTier::from_i32(1).unwrap(), EmergencyTier::Critical);
    assert_eq!(EmergencyTier::from_i32(2).unwrap(), EmergencyTier::Urgent);
    assert_eq!(EmergencyTier::from_i32(3).unwrap(), EmergencyTier::Elevated);
    assert!(EmergencyTier::from_i32(4).is_err());
    assert!(EmergencyTier::from_i32(0).is_err());
}

#[tokio::test]
async fn test_emergency_activation_validation() {
    let activation = EmergencyActivation {
        tier: EmergencyTier::Critical,
        activated_by: "emergency_alice".to_string(),
        reason: "Critical security vulnerability".to_string(),
        evidence: "Detailed evidence of the critical security vulnerability that requires immediate attention".to_string(),
        signatures: vec![],
    };
    
    // Test insufficient evidence
    let mut short_evidence = activation.clone();
    short_evidence.evidence = "Short".to_string();
    let result = EmergencyValidator::validate_activation(&short_evidence);
    assert!(result.is_err());
    
    // Test sufficient evidence
    let result = EmergencyValidator::validate_activation(&activation);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_active_emergency_expiration() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Critical,
        activated_by: "emergency_alice".to_string(),
        reason: "Test".to_string(),
        activated_at: Utc::now() - Duration::days(10),
        expires_at: Utc::now() - Duration::days(1),
        extended: false,
        extension_count: 0,
    };
    
    assert!(emergency.is_expired());
    assert!(!emergency.can_extend()); // Critical doesn't allow extensions
    
    let non_expired = ActiveEmergency {
        id: 2,
        tier: EmergencyTier::Urgent,
        activated_by: "emergency_bob".to_string(),
        reason: "Test".to_string(),
        activated_at: Utc::now() - Duration::days(5),
        expires_at: Utc::now() + Duration::days(25),
        extended: false,
        extension_count: 0,
    };
    
    assert!(!non_expired.is_expired());
    assert!(non_expired.can_extend()); // Urgent allows 1 extension
}

#[tokio::test]
async fn test_emergency_extension_validation() {
    let emergency = ActiveEmergency {
        id: 1,
        tier: EmergencyTier::Urgent,
        activated_by: "emergency_alice".to_string(),
        reason: "Test".to_string(),
        activated_at: Utc::now() - Duration::days(5),
        expires_at: Utc::now() + Duration::days(25),
        extended: false,
        extension_count: 0,
    };
    
    let signatures = vec![];
    
    // Test valid extension
    let result = EmergencyValidator::validate_extension(&emergency, &signatures);
    assert!(result.is_ok());
    
    // Test extension on Critical tier (not allowed)
    let critical_emergency = ActiveEmergency {
        tier: EmergencyTier::Critical,
        ..emergency
    };
    
    let result = EmergencyValidator::validate_extension(&critical_emergency, &signatures);
    assert!(result.is_err());
}




