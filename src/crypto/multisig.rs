use crate::crypto::signatures::SignatureManager;
use crate::error::GovernanceError;
use std::collections::HashMap;

pub struct MultisigManager {
    signature_manager: SignatureManager,
}

impl MultisigManager {
    pub fn new() -> Self {
        Self {
            signature_manager: SignatureManager::new(),
        }
    }

    pub fn verify_multisig(
        &self,
        message: &str,
        signatures: &[(String, String)],       // (signer, signature)
        public_keys: &HashMap<String, String>, // username -> public_key
        required_threshold: (usize, usize),    // (required, total)
    ) -> Result<bool, GovernanceError> {
        let (required, total) = required_threshold;
        let mut valid_signatures = 0;
        let mut verified_signers = Vec::new();

        for (signer, signature) in signatures {
            if let Some(public_key_str) = public_keys.get(signer) {
                // Parse public key
                let public_key = public_key_str
                    .parse::<secp256k1::PublicKey>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid public key for {}: {}",
                            signer, e
                        ))
                    })?;

                // Parse signature
                let sig = signature
                    .parse::<secp256k1::ecdsa::Signature>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid signature from {}: {}",
                            signer, e
                        ))
                    })?;

                // Verify signature
                if self
                    .signature_manager
                    .verify_signature(message, &sig, &public_key)?
                {
                    valid_signatures += 1;
                    verified_signers.push(signer.clone());
                }
            }
        }

        if valid_signatures >= required {
            Ok(true)
        } else {
            Err(GovernanceError::ThresholdError(format!(
                "Multisig threshold not met. Required: {}/{} signatures, Valid: {}/{}",
                required,
                total,
                valid_signatures,
                signatures.len()
            )))
        }
    }

    pub fn get_verified_signers(
        &self,
        message: &str,
        signatures: &[(String, String)],
        public_keys: &HashMap<String, String>,
    ) -> Result<Vec<String>, GovernanceError> {
        let mut verified_signers = Vec::new();

        for (signer, signature) in signatures {
            if let Some(public_key_str) = public_keys.get(signer) {
                let public_key = public_key_str
                    .parse::<secp256k1::PublicKey>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid public key for {}: {}",
                            signer, e
                        ))
                    })?;

                let sig = signature
                    .parse::<secp256k1::ecdsa::Signature>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid signature from {}: {}",
                            signer, e
                        ))
                    })?;

                if self
                    .signature_manager
                    .verify_signature(message, &sig, &public_key)?
                {
                    verified_signers.push(signer.clone());
                }
            }
        }

        Ok(verified_signers)
    }
}

impl Default for MultisigManager {
    fn default() -> Self {
        Self::new()
    }
}
