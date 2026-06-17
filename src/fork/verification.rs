//! Fork Decision Verification
//!
//! Utilities for verifying fork decision signatures.

use super::types::ForkDecision;
use crate::error::GovernanceError;
use blvm_sdk::governance::{PublicKey, Signature, verify_signature};
use hex;
use serde_json;

/// Serialize fork decision for signing (excludes signature field)
fn serialize_decision_for_signing(decision: &ForkDecision) -> Vec<u8> {
    // Serialize all fields except signature
    let data = serde_json::json!({
        "node_id": decision.node_id,
        "node_type": decision.node_type,
        "chosen_ruleset": decision.chosen_ruleset,
        "decision_reason": decision.decision_reason,
        "weight": decision.weight,
        "timestamp": decision.timestamp.to_rfc3339(),
    });
    serde_json::to_vec(&data).unwrap_or_default()
}

/// Verify a fork decision signature
pub fn verify_fork_decision_signature(
    decision: &ForkDecision,
    public_key: &PublicKey,
) -> Result<bool, GovernanceError> {
    // Serialize decision (without signature)
    let message = serialize_decision_for_signing(decision);

    // Parse signature
    let signature_bytes = hex::decode(&decision.signature)
        .map_err(|_| GovernanceError::InvalidSignature("Invalid hex format".to_string()))?;

    let signature = Signature::from_bytes(&signature_bytes)
        .map_err(|e| GovernanceError::InvalidSignature(format!("Invalid signature format: {e}")))?;

    // Verify
    verify_signature(&signature, &message, public_key)
        .map_err(|e| GovernanceError::CryptoError(format!("Verification error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blvm_sdk::governance::{GovernanceKeypair, signatures::sign_message};
    use chrono::Utc;

    #[test]
    fn test_fork_decision_signature_verification() {
        let keypair = GovernanceKeypair::generate().unwrap();
        let public_key = keypair.public_key();

        // Create decision
        let mut decision = ForkDecision {
            node_id: "test-node".to_string(),
            node_type: "test".to_string(),
            chosen_ruleset: "ruleset-1".to_string(),
            decision_reason: "Test fork".to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            signature: String::new(),
        };

        // Sign decision
        let message = serialize_decision_for_signing(&decision);
        let signature = sign_message(&keypair.secret_key, &message).unwrap();
        decision.signature = hex::encode(signature.to_bytes());

        // Verify
        let verified = verify_fork_decision_signature(&decision, &public_key).unwrap();
        assert!(verified, "Signature should verify");
    }

    #[test]
    fn test_fork_decision_signature_rejects_tampered() {
        let keypair = GovernanceKeypair::generate().unwrap();
        let public_key = keypair.public_key();

        // Create and sign decision
        let mut decision = ForkDecision {
            node_id: "test-node".to_string(),
            node_type: "test".to_string(),
            chosen_ruleset: "ruleset-1".to_string(),
            decision_reason: "Test fork".to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            signature: String::new(),
        };

        let message = serialize_decision_for_signing(&decision);
        let signature = sign_message(&keypair.secret_key, &message).unwrap();
        decision.signature = hex::encode(signature.to_bytes());

        // Tamper with decision
        decision.chosen_ruleset = "ruleset-2".to_string();

        // Verification should fail
        let verified = verify_fork_decision_signature(&decision, &public_key).unwrap();
        assert!(!verified, "Tampered decision should fail verification");
    }
}
