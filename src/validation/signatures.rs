use secp256k1::{PublicKey, Secp256k1, ecdsa::Signature};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use crate::error::GovernanceError;

pub struct SignatureValidator {
    secp: Secp256k1<secp256k1::All>,
}

impl SignatureValidator {
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
        }
    }

    pub fn verify_signature(
        &self,
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> Result<bool, GovernanceError> {
        // Parse public key
        let pub_key = PublicKey::from_str(public_key)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid public key: {}", e)))?;

        // Parse signature
        let sig = Signature::from_str(signature)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid signature: {}", e)))?;

        // Hash message
        let message_hash = Sha256::digest(message.as_bytes());
        let message_hash = secp256k1::Message::from_digest_slice(&message_hash)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {}", e)))?;

        // Verify signature
        match self.secp.verify_ecdsa(&message_hash, &sig, &pub_key) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn verify_multisig_threshold(
        &self,
        signatures: &[(String, String)], // (signer, signature)
        required_threshold: (usize, usize), // (required, total)
        maintainer_keys: &std::collections::HashMap<String, String>, // username -> public_key
    ) -> Result<bool, GovernanceError> {
        let (required, _total) = required_threshold;
        let mut valid_signatures = 0;

        for (signer, signature) in signatures {
            if let Some(public_key) = maintainer_keys.get(signer) {
                // Create message for signature verification
                let message = format!("governance-signature:{}", signer);
                
                if self.verify_signature(&message, signature, public_key)? {
                    valid_signatures += 1;
                }
            }
        }

        Ok(valid_signatures >= required)
    }
}

impl Default for SignatureValidator {
    fn default() -> Self {
        Self::new()
    }
}




