use crate::error::GovernanceError;
use blvm_sdk::governance::{
    GovernanceKeypair, PublicKey as GovernancePublicKey, Signature as GovernanceSignature,
    signatures::sign_message, verify_signature,
};
use secp256k1::{PublicKey, Secp256k1, SecretKey, ecdsa::Signature};
use sha2::{Digest, Sha256};

pub struct SignatureManager {
    secp: Secp256k1<secp256k1::All>,
}

impl SignatureManager {
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
        }
    }

    pub fn create_signature(
        &self,
        message: &str,
        secret_key: &SecretKey,
    ) -> Result<Signature, GovernanceError> {
        let message_hash = Sha256::digest(message.as_bytes());
        let message_hash = secp256k1::Message::from_digest_slice(&message_hash)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {e}")))?;

        Ok(self.secp.sign_ecdsa(&message_hash, secret_key))
    }

    pub fn verify_signature(
        &self,
        message: &str,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> Result<bool, GovernanceError> {
        let message_hash = Sha256::digest(message.as_bytes());
        let message_hash = secp256k1::Message::from_digest_slice(&message_hash)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {e}")))?;

        match self.secp.verify_ecdsa(&message_hash, signature, public_key) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Verify signature using blvm-sdk governance primitives
    pub fn verify_governance_signature(
        &self,
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> Result<bool, GovernanceError> {
        // Parse signature from hex string
        let signature_bytes = hex::decode(signature)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid signature hex: {e}")))?;
        let signature = GovernanceSignature::from_bytes(&signature_bytes)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid signature format: {e}")))?;

        // Parse public key from hex string
        let public_key_bytes = hex::decode(public_key)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid public key hex: {e}")))?;
        let public_key = GovernancePublicKey::from_bytes(&public_key_bytes)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid public key format: {e}")))?;

        // Use blvm-sdk's verify_signature function
        verify_signature(&signature, message.as_bytes(), &public_key).map_err(|e| {
            GovernanceError::CryptoError(format!("Signature verification failed: {e}"))
        })
    }

    /// Create signature using blvm-sdk governance primitives
    pub fn create_governance_signature(
        &self,
        message: &str,
        keypair: &GovernanceKeypair,
    ) -> Result<String, GovernanceError> {
        // Use blvm-sdk's sign_message function
        let signature = sign_message(&keypair.secret_key, message.as_bytes())
            .map_err(|e| GovernanceError::CryptoError(format!("Signature creation failed: {e}")))?;

        Ok(signature.to_string())
    }

    pub fn public_key_from_secret(&self, secret_key: &SecretKey) -> PublicKey {
        PublicKey::from_secret_key(&self.secp, secret_key)
    }

    /// Generate a new governance keypair (`blvm-sdk` raw scalar + compressed pubkey bytes).
    pub fn generate_keypair(&self) -> Result<GovernanceKeypair, GovernanceError> {
        GovernanceKeypair::generate().map_err(|e| {
            GovernanceError::CryptoError(format!("Governance key generation failed: {e}"))
        })
    }
}

impl Default for SignatureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::rand::rngs::OsRng;

    #[test]
    fn test_signature_creation_and_verification() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&manager.secp, &secret_key);

        let message = "test message";
        let signature = manager.create_signature(message, &secret_key).unwrap();

        let verified = manager
            .verify_signature(message, &signature, &public_key)
            .unwrap();
        assert!(verified, "Signature should be valid");
    }

    #[test]
    fn test_signature_verification_fails_wrong_message() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&manager.secp, &secret_key);

        let message1 = "test message";
        let message2 = "different message";
        let signature = manager.create_signature(message1, &secret_key).unwrap();

        let verified = manager
            .verify_signature(message2, &signature, &public_key)
            .unwrap();
        assert!(
            !verified,
            "Signature should be invalid for different message"
        );
    }

    #[test]
    fn test_signature_verification_fails_wrong_key() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key1 = SecretKey::new(&mut rng);
        let secret_key2 = SecretKey::new(&mut rng);
        let public_key2 = PublicKey::from_secret_key(&manager.secp, &secret_key2);

        let message = "test message";
        let signature = manager.create_signature(message, &secret_key1).unwrap();

        let verified = manager
            .verify_signature(message, &signature, &public_key2)
            .unwrap();
        assert!(!verified, "Signature should be invalid for different key");
    }

    #[test]
    fn test_governance_signature_creation_and_verification() {
        let manager = SignatureManager::new();
        let keypair = manager.generate_keypair().unwrap();

        let message = "governance message";
        let signature = manager
            .create_governance_signature(message, &keypair)
            .unwrap();

        let public_key_str = hex::encode(keypair.public_key);
        let verified = manager
            .verify_governance_signature(message, &signature, &public_key_str)
            .unwrap();
        assert!(verified, "Governance signature should be valid");
    }

    #[test]
    fn test_governance_signature_verification_fails_wrong_message() {
        let manager = SignatureManager::new();
        let keypair = manager.generate_keypair().unwrap();

        let message1 = "governance message";
        let message2 = "different message";
        let signature = manager
            .create_governance_signature(message1, &keypair)
            .unwrap();

        let public_key_str = hex::encode(keypair.public_key);
        let verified = manager
            .verify_governance_signature(message2, &signature, &public_key_str)
            .unwrap();
        assert!(
            !verified,
            "Governance signature should be invalid for different message"
        );
    }

    #[test]
    fn test_governance_signature_invalid_hex() {
        let manager = SignatureManager::new();

        let result = manager.verify_governance_signature("message", "invalid_hex", "invalid_hex");
        assert!(result.is_err(), "Should fail with invalid hex");
    }

    #[test]
    fn test_public_key_from_secret() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);

        let public_key = manager.public_key_from_secret(&secret_key);
        let expected = PublicKey::from_secret_key(&manager.secp, &secret_key);

        assert_eq!(public_key, expected, "Public key should match");
    }

    #[test]
    fn test_generate_keypair() {
        let manager = SignatureManager::new();
        let keypair1 = manager.generate_keypair().unwrap();
        let keypair2 = manager.generate_keypair().unwrap();

        // Keypairs should be different
        assert_ne!(
            keypair1.secret_key, keypair2.secret_key,
            "Generated keypairs should be different"
        );
        assert_ne!(
            keypair1.public_key, keypair2.public_key,
            "Generated public keys should be different"
        );

        // But each keypair should be consistent
        let sec1 = SecretKey::from_slice(&keypair1.secret_key).expect("valid governance secret");
        let public_key1 = manager.public_key_from_secret(&sec1);
        assert_eq!(
            public_key1.serialize(),
            keypair1.public_key,
            "Public key should match secret key"
        );
    }

    #[test]
    fn test_signature_determinism() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);

        let message = "test message";
        let signature1 = manager.create_signature(message, &secret_key).unwrap();
        let signature2 = manager.create_signature(message, &secret_key).unwrap();

        // ECDSA signatures are non-deterministic (uses random nonce), so they should be different
        // But both should verify
        let public_key = PublicKey::from_secret_key(&manager.secp, &secret_key);
        assert!(
            manager
                .verify_signature(message, &signature1, &public_key)
                .unwrap()
        );
        assert!(
            manager
                .verify_signature(message, &signature2, &public_key)
                .unwrap()
        );
    }

    #[test]
    fn test_empty_message() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&manager.secp, &secret_key);

        let message = "";
        let signature = manager.create_signature(message, &secret_key).unwrap();
        let verified = manager
            .verify_signature(message, &signature, &public_key)
            .unwrap();
        assert!(verified, "Empty message should work");
    }

    #[test]
    fn test_long_message() {
        let manager = SignatureManager::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&manager.secp, &secret_key);

        let message = "a".repeat(10000);
        let signature = manager.create_signature(&message, &secret_key).unwrap();
        let verified = manager
            .verify_signature(&message, &signature, &public_key)
            .unwrap();
        assert!(verified, "Long message should work");
    }
}
