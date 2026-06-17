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
                            "Invalid public key for {signer}: {e}"
                        ))
                    })?;

                // Parse signature
                let sig = signature
                    .parse::<secp256k1::ecdsa::Signature>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid signature from {signer}: {e}"
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
                            "Invalid public key for {signer}: {e}"
                        ))
                    })?;

                let sig = signature
                    .parse::<secp256k1::ecdsa::Signature>()
                    .map_err(|e| {
                        GovernanceError::CryptoError(format!(
                            "Invalid signature from {signer}: {e}"
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

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::rand::rngs::OsRng;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    fn create_test_keypair() -> (String, SecretKey, PublicKey) {
        let secp = Secp256k1::new();
        let mut rng = OsRng;
        let secret_key = SecretKey::new(&mut rng);
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_str = public_key.to_string();
        (public_key_str, secret_key, public_key)
    }

    #[test]
    fn test_multisig_threshold_met() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();
        let (pub3, sec3, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);
        public_keys.insert("charlie".to_string(), pub3);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();
        let sig2 = sig_manager.create_signature(message, &sec2).unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), sig2.to_string()),
        ];

        // 2-of-3 threshold
        let result = manager.verify_multisig(message, &signatures, &public_keys, (2, 3));
        assert!(
            result.is_ok() && result.unwrap(),
            "Should meet 2-of-3 threshold"
        );
    }

    #[test]
    fn test_multisig_threshold_not_met() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();
        let (pub3, sec3, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);
        public_keys.insert("charlie".to_string(), pub3);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();

        let signatures = vec![("alice".to_string(), sig1.to_string())];

        // 2-of-3 threshold, only 1 signature
        let result = manager.verify_multisig(message, &signatures, &public_keys, (2, 3));
        assert!(
            result.is_err(),
            "Should fail 2-of-3 threshold with only 1 signature"
        );
    }

    #[test]
    fn test_multisig_invalid_signature() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();
        let wrong_sig = sig_manager
            .create_signature("wrong message", &sec2)
            .unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), wrong_sig.to_string()), // Wrong signature
        ];

        // 2-of-2 threshold, but one signature is invalid
        let result = manager.verify_multisig(message, &signatures, &public_keys, (2, 2));
        assert!(result.is_err(), "Should fail with invalid signature");
    }

    #[test]
    fn test_multisig_missing_public_key() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        // bob's key is missing

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), "invalid".to_string()),
        ];

        // bob's signature will be ignored (no public key)
        let result = manager.verify_multisig(message, &signatures, &public_keys, (1, 2));
        assert!(
            result.is_ok() && result.unwrap(),
            "Should pass with alice's signature"
        );
    }

    #[test]
    fn test_get_verified_signers() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();
        let (pub3, sec3, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);
        public_keys.insert("charlie".to_string(), pub3);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();
        let sig2 = sig_manager.create_signature(message, &sec2).unwrap();
        let wrong_sig = sig_manager.create_signature("wrong", &sec3).unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), sig2.to_string()),
            ("charlie".to_string(), wrong_sig.to_string()),
        ];

        let verified = manager
            .get_verified_signers(message, &signatures, &public_keys)
            .unwrap();
        assert_eq!(verified.len(), 2, "Should have 2 verified signers");
        assert!(verified.contains(&"alice".to_string()));
        assert!(verified.contains(&"bob".to_string()));
        assert!(!verified.contains(&"charlie".to_string()));
    }

    #[test]
    fn test_multisig_exact_threshold() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();
        let sig2 = sig_manager.create_signature(message, &sec2).unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), sig2.to_string()),
        ];

        // Exactly 2-of-2
        let result = manager.verify_multisig(message, &signatures, &public_keys, (2, 2));
        assert!(
            result.is_ok() && result.unwrap(),
            "Should meet exact 2-of-2 threshold"
        );
    }

    #[test]
    fn test_multisig_more_than_threshold() {
        let manager = MultisigManager::new();
        let sig_manager = SignatureManager::new();

        let (pub1, sec1, _) = create_test_keypair();
        let (pub2, sec2, _) = create_test_keypair();
        let (pub3, sec3, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);
        public_keys.insert("bob".to_string(), pub2);
        public_keys.insert("charlie".to_string(), pub3);

        let message = "test message";
        let sig1 = sig_manager.create_signature(message, &sec1).unwrap();
        let sig2 = sig_manager.create_signature(message, &sec2).unwrap();
        let sig3 = sig_manager.create_signature(message, &sec3).unwrap();

        let signatures = vec![
            ("alice".to_string(), sig1.to_string()),
            ("bob".to_string(), sig2.to_string()),
            ("charlie".to_string(), sig3.to_string()),
        ];

        // 2-of-3 threshold, but have 3 signatures
        let result = manager.verify_multisig(message, &signatures, &public_keys, (2, 3));
        assert!(
            result.is_ok() && result.unwrap(),
            "Should pass with more than threshold"
        );
    }

    #[test]
    fn test_multisig_invalid_public_key_format() {
        let manager = MultisigManager::new();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), "invalid_key".to_string());

        let signatures = vec![("alice".to_string(), "invalid_sig".to_string())];

        let result = manager.verify_multisig("message", &signatures, &public_keys, (1, 1));
        assert!(
            result.is_err(),
            "Should fail with invalid public key format"
        );
    }

    #[test]
    fn test_multisig_invalid_signature_format() {
        let manager = MultisigManager::new();
        let (pub1, _, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);

        let signatures = vec![("alice".to_string(), "invalid_signature".to_string())];

        let result = manager.verify_multisig("message", &signatures, &public_keys, (1, 1));
        assert!(result.is_err(), "Should fail with invalid signature format");
    }

    #[test]
    fn test_multisig_empty_signatures() {
        let manager = MultisigManager::new();
        let (pub1, _, _) = create_test_keypair();

        let mut public_keys = HashMap::new();
        public_keys.insert("alice".to_string(), pub1);

        let signatures = vec![];

        let result = manager.verify_multisig("message", &signatures, &public_keys, (1, 1));
        assert!(result.is_err(), "Should fail with no signatures");
    }
}
