use crate::error::GovernanceError;
use developer_sdk::governance::{
    signatures::sign_message, verify_signature, GovernanceKeypair,
    PublicKey as GovernancePublicKey, Signature as GovernanceSignature,
};
use secp256k1::{ecdsa::Signature, PublicKey, Secp256k1, SecretKey};
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
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {}", e)))?;

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
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {}", e)))?;

        match self.secp.verify_ecdsa(&message_hash, signature, public_key) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Verify signature using developer-sdk governance primitives
    pub fn verify_governance_signature(
        &self,
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> Result<bool, GovernanceError> {
        // Parse signature from hex string
        let signature_bytes = hex::decode(signature)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid signature hex: {}", e)))?;
        let signature = GovernanceSignature::from_bytes(&signature_bytes).map_err(|e| {
            GovernanceError::CryptoError(format!("Invalid signature format: {}", e))
        })?;

        // Parse public key from hex string
        let public_key_bytes = hex::decode(public_key)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid public key hex: {}", e)))?;
        let public_key = GovernancePublicKey::from_bytes(&public_key_bytes).map_err(|e| {
            GovernanceError::CryptoError(format!("Invalid public key format: {}", e))
        })?;

        // Use developer-sdk's verify_signature function
        verify_signature(&signature, message.as_bytes(), &public_key).map_err(|e| {
            GovernanceError::CryptoError(format!("Signature verification failed: {}", e))
        })
    }

    /// Create signature using developer-sdk governance primitives
    pub fn create_governance_signature(
        &self,
        message: &str,
        keypair: &GovernanceKeypair,
    ) -> Result<String, GovernanceError> {
        // Use developer-sdk's sign_message function
        let signature = sign_message(&keypair.secret_key, message.as_bytes()).map_err(|e| {
            GovernanceError::CryptoError(format!("Signature creation failed: {}", e))
        })?;

        Ok(signature.to_string())
    }

    pub fn public_key_from_secret(&self, secret_key: &SecretKey) -> PublicKey {
        PublicKey::from_secret_key(&self.secp, secret_key)
    }

    /// Generate a new keypair
    pub fn generate_keypair(&self) -> Result<GovernanceKeypair, GovernanceError> {
        use secp256k1::rand::rngs::OsRng;
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&self.secp, &secret_key);
        Ok(GovernanceKeypair {
            secret_key,
            public_key,
        })
    }
}

impl Default for SignatureManager {
    fn default() -> Self {
        Self::new()
    }
}
