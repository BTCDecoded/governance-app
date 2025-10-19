use governance_app::crypto::*;

#[tokio::test]
async fn test_signature_creation_and_verification() {
    use secp256k1::{SecretKey, Secp256k1};
    use rand::rngs::OsRng;
    
    let secp = Secp256k1::new();
    let secret_key = SecretKey::new(&mut OsRng);
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    
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
async fn test_multisig_verification() {
    use std::collections::HashMap;
    use secp256k1::{SecretKey, Secp256k1};
    use rand::rngs::OsRng;
    
    let secp = Secp256k1::new();
    let multisig_manager = MultisigManager::new();
    
    // Create test keypairs
    let secret1 = SecretKey::new(&mut OsRng);
    let public1 = secp256k1::PublicKey::from_secret_key(&secp, &secret1);
    
    let secret2 = SecretKey::new(&mut OsRng);
    let public2 = secp256k1::PublicKey::from_secret_key(&secp, &secret2);
    
    let message = "multisig test message";
    
    // Create signatures
    let signature_manager = SignatureManager::new();
    let sig1 = signature_manager.create_signature(message, &secret1).unwrap();
    let sig2 = signature_manager.create_signature(message, &secret2).unwrap();
    
    // Set up public keys map
    let mut public_keys = HashMap::new();
    public_keys.insert("user1".to_string(), public1.to_string());
    public_keys.insert("user2".to_string(), public2.to_string());
    
    // Test multisig verification
    let signatures = vec![
        ("user1".to_string(), sig1.to_string()),
        ("user2".to_string(), sig2.to_string()),
    ];
    
    let result = multisig_manager.verify_multisig(
        message,
        &signatures,
        &public_keys,
        (2, 2), // 2-of-2
    );
    
    assert!(result.is_ok());
}




