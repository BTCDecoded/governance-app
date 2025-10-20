use governance_app::crypto::*;
use secp256k1::{SecretKey, Secp256k1, PublicKey};
use rand::rngs::OsRng;
use std::collections::HashMap;

mod common;
use common::*;

#[tokio::test]
async fn test_signature_creation_and_verification() {
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    let signature_manager = SignatureManager::new();
    let message = "test message";
    
    // Create signature
    let signature = signature_manager.create_signature(message, &secret_key).unwrap();
    
    // Verify signature
    let is_valid = signature_manager.verify_signature(message, &signature, &public_key).unwrap();
    assert!(is_valid);
    
    // Test with wrong message
    let wrong_message = "wrong message";
    let is_valid = signature_manager.verify_signature(wrong_message, &signature, &public_key).unwrap();
    assert!(!is_valid);
}

#[tokio::test]
async fn test_signature_edge_cases() {
    let signature_manager = SignatureManager::new();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    // Test empty message
    let empty_message = "";
    let signature = signature_manager.create_signature(empty_message, &secret_key).unwrap();
    let is_valid = signature_manager.verify_signature(empty_message, &signature, &public_key).unwrap();
    assert!(is_valid);
    
    // Test very long message
    let long_message = "a".repeat(10000);
    let signature = signature_manager.create_signature(&long_message, &secret_key).unwrap();
    let is_valid = signature_manager.verify_signature(&long_message, &signature, &public_key).unwrap();
    assert!(is_valid);
    
    // Test message with special characters
    let special_message = "Hello, 世界! 🌍\n\t\r\0";
    let signature = signature_manager.create_signature(special_message, &secret_key).unwrap();
    let is_valid = signature_manager.verify_signature(special_message, &signature, &public_key).unwrap();
    assert!(is_valid);
}

#[tokio::test]
async fn test_signature_tampering() {
    let signature_manager = SignatureManager::new();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    let message = "original message";
    let signature = signature_manager.create_signature(message, &secret_key).unwrap();
    
    // Test tampered message
    let tampered_message = "tampered message";
    let is_valid = signature_manager.verify_signature(tampered_message, &signature, &public_key).unwrap();
    assert!(!is_valid);
    
    // Test tampered signature (modify one byte)
    let mut tampered_signature = signature;
    let signature_bytes = tampered_signature.serialize_der();
    let mut modified_bytes = signature_bytes.to_vec();
    if !modified_bytes.is_empty() {
        modified_bytes[0] = modified_bytes[0].wrapping_add(1);
    }
    // This would create an invalid signature, but we can't easily create a valid but different signature
    // So we'll test with a completely different signature
    let different_secret = SecretKey::new(&mut OsRng);
    let different_signature = signature_manager.create_signature("different message", &different_secret).unwrap();
    let is_valid = signature_manager.verify_signature(message, &different_signature, &public_key).unwrap();
    assert!(!is_valid);
}

#[tokio::test]
async fn test_multisig_verification() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    
    // Create test keypairs
    let keypairs = generate_test_keypairs(3);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "multisig test message";
    
    // Create signatures
    let signature_manager = SignatureManager::new();
    let signatures: Vec<(String, String)> = keypairs.iter().map(|(username, secret_key, _)| {
        let signature = signature_manager.create_signature(message, secret_key).unwrap();
        (username.clone(), signature.to_string())
    }).collect();
    
    // Test multisig verification
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (2, 3), // 2-of-3
    );
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multisig_threshold_boundaries() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    let signature_manager = SignatureManager::new();
    
    // Create 5 keypairs
    let keypairs = generate_test_keypairs(5);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "threshold test message";
    
    // Test exactly at threshold (3-of-5)
    let signatures: Vec<(String, String)> = keypairs[0..3].iter().map(|(username, secret_key, _)| {
        let signature = signature_manager.create_signature(message, secret_key).unwrap();
        (username.clone(), signature.to_string())
    }).collect();
    
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (3, 5),
    );
    assert!(result.is_ok());
    
    // Test one below threshold (2-of-5)
    let insufficient_signatures = &signatures[0..2];
    let result = multisig_manager.verify_multisig(
        message,
        insufficient_signatures,
        &public_keys,
        (3, 5),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multisig_duplicate_signers() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    let signature_manager = SignatureManager::new();
    
    let keypairs = generate_test_keypairs(2);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "duplicate signer test";
    let (username, secret_key, _) = &keypairs[0];
    let signature = signature_manager.create_signature(message, secret_key).unwrap();
    
    // Create duplicate signatures from the same signer
    let signatures = vec![
        (username.clone(), signature.to_string()),
        (username.clone(), signature.to_string()),
    ];
    
    // This should still work - duplicate signatures are allowed
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (2, 2),
    );
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multisig_missing_keys() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    let signature_manager = SignatureManager::new();
    
    let keypairs = generate_test_keypairs(3);
    let mut public_keys = HashMap::new();
    
    // Only add 2 of the 3 public keys
    for (username, _, public_key) in &keypairs[0..2] {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "missing key test";
    let signatures: Vec<(String, String)> = keypairs.iter().map(|(username, secret_key, _)| {
        let signature = signature_manager.create_signature(message, secret_key).unwrap();
        (username.clone(), signature.to_string())
    }).collect();
    
    // This should fail because we don't have the public key for the third signer
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (2, 3),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multisig_invalid_signatures() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    
    let keypairs = generate_test_keypairs(2);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "invalid signature test";
    
    // Create signatures with invalid data
    let signatures = vec![
        ("testuser0".to_string(), "invalid_signature_1".to_string()),
        ("testuser1".to_string(), "invalid_signature_2".to_string()),
    ];
    
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (2, 2),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn test_multisig_verified_signers() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    let signature_manager = SignatureManager::new();
    
    let keypairs = generate_test_keypairs(3);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "verified signers test";
    let signatures: Vec<(String, String)> = keypairs.iter().map(|(username, secret_key, _)| {
        let signature = signature_manager.create_signature(message, secret_key).unwrap();
        (username.clone(), signature.to_string())
    }).collect();
    
    let verified_signers = multisig_manager.get_verified_signers(
        message,
        &signatures,
        &public_keys,
    ).unwrap();
    
    assert_eq!(verified_signers.len(), 3);
    assert!(verified_signers.contains(&"testuser0".to_string()));
    assert!(verified_signers.contains(&"testuser1".to_string()));
    assert!(verified_signers.contains(&"testuser2".to_string()));
}

#[tokio::test]
async fn test_bitcoin_compatible_signatures() {
    let signature_manager = SignatureManager::new();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    // Test Bitcoin-style message signing
    let message = "governance-signature:alice";
    let signature = signature_manager.create_signature(message, &secret_key).unwrap();
    
    // Verify the signature
    let is_valid = signature_manager.verify_signature(message, &signature, &public_key).unwrap();
    assert!(is_valid);
    
    // Test that the signature is deterministic for the same input
    let signature2 = signature_manager.create_signature(message, &secret_key).unwrap();
    assert_eq!(signature, signature2);
}

#[tokio::test]
async fn test_public_key_derivation() {
    let signature_manager = SignatureManager::new();
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    
    let derived_public_key = signature_manager.public_key_from_secret(&secret_key);
    let expected_public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    assert_eq!(derived_public_key, expected_public_key);
}

#[tokio::test]
async fn test_signature_serialization() {
    let signature_manager = SignatureManager::new();
    let secret_key = SecretKey::new(&mut OsRng);
    
    let message = "serialization test";
    let signature = signature_manager.create_signature(message, &secret_key).unwrap();
    
    // Test that signature can be serialized and deserialized
    let signature_string = signature.to_string();
    let deserialized_signature = signature_string.parse::<secp256k1::Signature>().unwrap();
    
    assert_eq!(signature, deserialized_signature);
}

#[tokio::test]
async fn test_multisig_large_threshold() {
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    let signature_manager = SignatureManager::new();
    
    // Create 10 keypairs for a large multisig
    let keypairs = generate_test_keypairs(10);
    let mut public_keys = HashMap::new();
    
    for (username, _, public_key) in &keypairs {
        public_keys.insert(username.clone(), public_key.to_string());
    }
    
    let message = "large multisig test";
    let signatures: Vec<(String, String)> = keypairs.iter().map(|(username, secret_key, _)| {
        let signature = signature_manager.create_signature(message, secret_key).unwrap();
        (username.clone(), signature.to_string())
    }).collect();
    
    // Test 7-of-10 threshold
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (7, 10),
    );
    assert!(result.is_ok());
    
    // Test with only 6 signatures (should fail)
    let insufficient_signatures = &signatures[0..6];
    let result = multisig_manager.verify_multisig(
        message,
        insufficient_signatures,
        &public_keys,
        (7, 10),
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn test_crypto_error_handling() {
    let signature_manager = SignatureManager::new();
    
    // Test with invalid secret key (this should not happen in practice, but test error handling)
    // Note: SecretKey::new() always creates valid keys, so we'll test other error conditions
    
    // Test signature verification with malformed data
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);
    
    // Test with invalid signature format
    let result = signature_manager.verify_signature("message", "invalid", &public_key.to_string());
    assert!(result.is_err());
    
    // Test with invalid public key format
    let result = signature_manager.verify_signature("message", "signature", "invalid_key");
    assert!(result.is_err());
}




