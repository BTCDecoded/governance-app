use crate::error::GovernanceError;
use crate::validation::nested_multisig::{NestedMultisigVerifier, Team};
use secp256k1::{PublicKey, Secp256k1, ecdsa::Signature};
use sha2::{Digest, Sha256};
use std::str::FromStr;

pub struct SignatureValidator {
    secp: Secp256k1<secp256k1::All>,
    nested_multisig_verifier: Option<NestedMultisigVerifier>,
}

impl SignatureValidator {
    pub fn new() -> Self {
        Self {
            secp: Secp256k1::new(),
            nested_multisig_verifier: None,
        }
    }

    /// Create with nested multisig support (for Tier 3+)
    pub fn with_nested_multisig(teams: Vec<Team>) -> Self {
        Self {
            secp: Secp256k1::new(),
            nested_multisig_verifier: Some(NestedMultisigVerifier::new(teams)),
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
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid public key: {e}")))?;

        // Parse signature
        let sig = Signature::from_str(signature)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid signature: {e}")))?;

        // Hash message
        let message_hash = Sha256::digest(message.as_bytes());
        let message_hash = secp256k1::Message::from_digest_slice(&message_hash)
            .map_err(|e| GovernanceError::CryptoError(format!("Invalid message hash: {e}")))?;

        // Verify signature
        match self.secp.verify_ecdsa(&message_hash, &sig, &pub_key) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub fn verify_multisig_threshold(
        &self,
        signatures: &[(String, String)],    // (signer, signature)
        required_threshold: (usize, usize), // (required, total)
        maintainer_keys: &std::collections::HashMap<String, String>, // username -> public_key
        tier: Option<u32>,                  // Optional tier for nested multisig
    ) -> Result<bool, GovernanceError> {
        // For Tier 3+, use nested multisig if available
        if let Some(tier_val) = tier {
            if tier_val >= 3 {
                if let Some(ref nested_verifier) = self.nested_multisig_verifier {
                    // Verify signatures first, then use nested multisig
                    let mut verified_sigs = Vec::new();
                    for (signer, signature) in signatures {
                        if let Some(public_key) = maintainer_keys.get(signer) {
                            let message = format!("governance-signature:{signer}");
                            if self.verify_signature(&message, signature, public_key)? {
                                verified_sigs.push((signer.clone(), signature.clone()));
                            }
                        }
                    }

                    // Use nested multisig verification
                    let result =
                        nested_verifier.verify_nested_multisig(&verified_sigs, tier_val)?;
                    return Ok(result.inter_team_approved);
                }
            }
        }

        // Fall back to simple multisig for Tier 1-2 or if nested multisig not available
        let (required, _total) = required_threshold;
        let mut valid_signatures = 0;

        for (signer, signature) in signatures {
            if let Some(public_key) = maintainer_keys.get(signer) {
                // Create message for signature verification
                let message = format!("governance-signature:{signer}");

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
