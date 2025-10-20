use secp256k1::{PublicKey, Secp256k1, SecretKey, ecdsa::Signature};
use sha2::{Digest, Sha256};
use crate::error::GovernanceError;

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

    pub fn public_key_from_secret(&self, secret_key: &SecretKey) -> PublicKey {
        PublicKey::from_secret_key(&self.secp, secret_key)
    }
}

impl Default for SignatureManager {
    fn default() -> Self {
        Self::new()
    }
}




