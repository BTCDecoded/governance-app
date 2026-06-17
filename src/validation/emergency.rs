//! Emergency tier validation and management
//!
//! Implements the three-tiered emergency response system:
//! - Tier 1 (Critical): 0 day review, 4-of-7 signatures, 7 day max duration
//! - Tier 2 (Urgent): 7 day review, 5-of-7 signatures, 30 day max duration
//! - Tier 3 (Elevated): 30 day review, 6-of-7 signatures, 90 day max duration

use crate::error::{InsufficientSignaturesArgs, MaxExtensionsReachedArgs};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::GovernanceAppError;

/// Emergency tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyTier {
    /// Critical: Network-threatening (inflation bugs, consensus forks, P2P DoS)
    Critical = 1,
    /// Urgent: Serious security issues (memory corruption, privacy leaks, crashes)
    Urgent = 2,
    /// Elevated: Important but not critical (bug fixes, competitive response)
    Elevated = 3,
}

impl EmergencyTier {
    /// Parse tier from integer
    pub fn from_i32(tier: i32) -> Result<Self, GovernanceAppError> {
        match tier {
            1 => Ok(EmergencyTier::Critical),
            2 => Ok(EmergencyTier::Urgent),
            3 => Ok(EmergencyTier::Elevated),
            _ => Err(GovernanceAppError::InvalidEmergencyTier(tier)),
        }
    }

    /// Convert tier to integer
    pub fn to_i32(&self) -> i32 {
        *self as i32
    }

    /// Get review period in days for this tier
    pub fn review_period_days(&self) -> u32 {
        match self {
            EmergencyTier::Critical => 0,
            EmergencyTier::Urgent => 7,
            EmergencyTier::Elevated => 30,
        }
    }

    /// Get signature threshold (N-of-M) for this tier
    pub fn signature_threshold(&self) -> (u32, u32) {
        match self {
            EmergencyTier::Critical => (4, 7),
            EmergencyTier::Urgent => (5, 7),
            EmergencyTier::Elevated => (6, 7),
        }
    }

    /// Get activation threshold (keyholders required to activate)
    pub fn activation_threshold(&self) -> (u32, u32) {
        // All tiers require 5-of-7 emergency keyholders to activate
        (5, 7)
    }

    /// Get maximum duration in days
    pub fn max_duration_days(&self) -> u32 {
        match self {
            EmergencyTier::Critical => 7,
            EmergencyTier::Urgent => 30,
            EmergencyTier::Elevated => 90,
        }
    }

    /// Get whether extensions are allowed
    pub fn allows_extensions(&self) -> bool {
        match self {
            EmergencyTier::Critical => false,
            EmergencyTier::Urgent => true,
            EmergencyTier::Elevated => true,
        }
    }

    /// Get maximum number of extensions allowed
    pub fn max_extensions(&self) -> u32 {
        match self {
            EmergencyTier::Critical => 0,
            EmergencyTier::Urgent => 1,
            EmergencyTier::Elevated => 2,
        }
    }

    /// Get extension duration in days
    pub fn extension_duration_days(&self) -> u32 {
        match self {
            EmergencyTier::Critical => 0,
            EmergencyTier::Urgent => 30,
            EmergencyTier::Elevated => 30,
        }
    }

    /// Get extension threshold (N-of-M)
    pub fn extension_threshold(&self) -> (u32, u32) {
        match self {
            EmergencyTier::Critical => (0, 0), // Not applicable
            EmergencyTier::Urgent => (6, 7),
            EmergencyTier::Elevated => (6, 7),
        }
    }

    /// Get post-mortem deadline in days
    pub fn post_mortem_deadline_days(&self) -> u32 {
        match self {
            EmergencyTier::Critical => 30,
            EmergencyTier::Urgent => 60,
            EmergencyTier::Elevated => 90,
        }
    }

    /// Get whether security audit is required
    pub fn requires_security_audit(&self) -> bool {
        match self {
            EmergencyTier::Critical => true,
            EmergencyTier::Urgent => false,
            EmergencyTier::Elevated => false,
        }
    }

    /// Get security audit deadline in days (if required)
    pub fn security_audit_deadline_days(&self) -> Option<u32> {
        match self {
            EmergencyTier::Critical => Some(60),
            EmergencyTier::Urgent => None,
            EmergencyTier::Elevated => None,
        }
    }

    /// Get tier name
    pub fn name(&self) -> &'static str {
        match self {
            EmergencyTier::Critical => "Critical Emergency",
            EmergencyTier::Urgent => "Urgent Security Issue",
            EmergencyTier::Elevated => "Elevated Priority",
        }
    }

    /// Get tier emoji for display
    pub fn emoji(&self) -> &'static str {
        match self {
            EmergencyTier::Critical => "🚨",
            EmergencyTier::Urgent => "⚠️",
            EmergencyTier::Elevated => "📢",
        }
    }

    /// Get tier description
    pub fn description(&self) -> &'static str {
        match self {
            EmergencyTier::Critical => {
                "Network-threatening vulnerability requiring immediate action"
            }
            EmergencyTier::Urgent => "Serious security issue requiring urgent response",
            EmergencyTier::Elevated => "Important priority requiring accelerated review",
        }
    }
}

/// Emergency tier activation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyActivation {
    pub tier: EmergencyTier,
    pub activated_by: String,
    pub reason: String,
    pub evidence: String,
    pub signatures: Vec<KeyholderSignature>,
}

/// Keyholder signature for emergency activation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyholderSignature {
    pub keyholder: String,
    pub public_key: String,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

/// Active emergency tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveEmergency {
    pub id: i32,
    pub tier: EmergencyTier,
    pub activated_by: String,
    pub reason: String,
    pub activated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub extended: bool,
    pub extension_count: u32,
}

impl ActiveEmergency {
    /// Check if emergency has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get remaining duration
    pub fn remaining_duration(&self) -> Duration {
        self.expires_at - Utc::now()
    }

    /// Check if extension is allowed
    pub fn can_extend(&self) -> bool {
        self.tier.allows_extensions()
            && self.extension_count < self.tier.max_extensions()
            && !self.is_expired()
    }

    /// Calculate new expiration if extended
    pub fn calculate_extension_expiration(&self) -> Option<DateTime<Utc>> {
        if !self.can_extend() {
            return None;
        }

        let extension_days = self.tier.extension_duration_days() as i64;
        Some(self.expires_at + Duration::try_days(extension_days).unwrap_or_default())
    }
}

/// Emergency tier validator
pub struct EmergencyValidator;

impl EmergencyValidator {
    /// Validate emergency activation request
    pub fn validate_activation(activation: &EmergencyActivation) -> Result<(), GovernanceAppError> {
        // Check minimum evidence length
        if activation.evidence.len() < 100 {
            return Err(GovernanceAppError::InsufficientEvidence(
                activation.evidence.len(),
            ));
        }

        // Check signature count meets activation threshold
        let (required, total) = activation.tier.activation_threshold();
        if activation.signatures.len() < required as usize {
            return Err(GovernanceAppError::InsufficientSignatures(
                InsufficientSignaturesArgs {
                    required: required as usize,
                    found: activation.signatures.len(),
                    threshold: format!("{required}-of-{total}"),
                },
            ));
        }

        // Validate individual signatures
        for sig in &activation.signatures {
            Self::validate_keyholder_signature(sig, activation)?;
        }

        Ok(())
    }

    /// Validate individual keyholder signature
    fn validate_keyholder_signature(
        sig: &KeyholderSignature,
        activation: &EmergencyActivation,
    ) -> Result<(), GovernanceAppError> {
        // Basic validation
        if sig.keyholder.is_empty() {
            return Err(GovernanceAppError::InvalidSignature(
                "Empty keyholder name".to_string(),
            ));
        }

        if sig.public_key.is_empty() {
            return Err(GovernanceAppError::InvalidSignature(
                "Empty public key".to_string(),
            ));
        }

        if sig.signature.is_empty() {
            return Err(GovernanceAppError::InvalidSignature(
                "Empty signature".to_string(),
            ));
        }

        // Parse public key from hex string
        let public_key_bytes =
            hex::decode(sig.public_key.trim_start_matches("0x")).map_err(|e| {
                GovernanceAppError::InvalidSignature(format!("Invalid public key hex: {e}"))
            })?;

        let public_key =
            blvm_sdk::governance::PublicKey::from_bytes(&public_key_bytes).map_err(|e| {
                GovernanceAppError::InvalidSignature(format!("Invalid public key format: {e}"))
            })?;

        // Parse signature from hex string
        let signature_bytes = hex::decode(sig.signature.trim_start_matches("0x")).map_err(|e| {
            GovernanceAppError::InvalidSignature(format!("Invalid signature hex: {e}"))
        })?;

        let signature =
            blvm_sdk::governance::Signature::from_bytes(&signature_bytes).map_err(|e| {
                GovernanceAppError::InvalidSignature(format!("Invalid signature format: {e}"))
            })?;

        // Create message to verify: serialize activation data
        let message = serde_json::to_vec(&serde_json::json!({
            "tier": activation.tier.to_i32(),
            "activated_by": activation.activated_by,
            "reason": activation.reason,
            "evidence": activation.evidence,
            "keyholder": sig.keyholder,
            "timestamp": sig.timestamp.to_rfc3339(),
        }))
        .map_err(|e| {
            GovernanceAppError::InvalidSignature(format!(
                "Failed to serialize activation message: {e}"
            ))
        })?;

        // Verify signature using blvm-sdk
        let verified = blvm_sdk::governance::verify_signature(&signature, &message, &public_key)
            .map_err(|e| {
                GovernanceAppError::InvalidSignature(format!("Signature verification error: {e}"))
            })?;

        if !verified {
            return Err(GovernanceAppError::InvalidSignature(format!(
                "Signature verification failed for keyholder: {}",
                sig.keyholder
            )));
        }

        Ok(())
    }

    /// Validate extension request
    pub fn validate_extension(
        emergency: &ActiveEmergency,
        signatures: &[KeyholderSignature],
    ) -> Result<(), GovernanceAppError> {
        // Check if tier allows extensions
        if !emergency.tier.allows_extensions() {
            return Err(GovernanceAppError::ExtensionNotAllowed(
                emergency.tier.name().to_string(),
            ));
        }

        // Check extension count
        if emergency.extension_count >= emergency.tier.max_extensions() {
            return Err(GovernanceAppError::MaxExtensionsReached(
                MaxExtensionsReachedArgs {
                    current: emergency.extension_count,
                    max: emergency.tier.max_extensions(),
                },
            ));
        }

        // Check if already expired
        if emergency.is_expired() {
            return Err(GovernanceAppError::EmergencyExpired(emergency.id));
        }

        // Check signature count meets extension threshold
        let (required, total) = emergency.tier.extension_threshold();
        if signatures.len() < required as usize {
            return Err(GovernanceAppError::InsufficientSignatures(
                InsufficientSignaturesArgs {
                    required: required as usize,
                    found: signatures.len(),
                    threshold: format!("{required}-of-{total}"),
                },
            ));
        }

        Ok(())
    }

    /// Check for expired emergencies
    pub fn check_expiration(active_emergencies: &[ActiveEmergency]) -> Vec<i32> {
        active_emergencies
            .iter()
            .filter(|e| e.is_expired())
            .map(|e| e.id)
            .collect()
    }

    /// Calculate expiration timestamp for new emergency
    pub fn calculate_expiration(tier: EmergencyTier) -> DateTime<Utc> {
        let duration_days = tier.max_duration_days() as i64;
        Utc::now() + Duration::try_days(duration_days).unwrap_or_default()
    }

    /// Calculate post-mortem deadline
    pub fn calculate_post_mortem_deadline(
        tier: EmergencyTier,
        activated_at: DateTime<Utc>,
    ) -> DateTime<Utc> {
        let deadline_days = tier.post_mortem_deadline_days() as i64;
        activated_at + Duration::try_days(deadline_days).unwrap_or_default()
    }

    /// Calculate security audit deadline (if required)
    pub fn calculate_security_audit_deadline(
        tier: EmergencyTier,
        activated_at: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        tier.security_audit_deadline_days()
            .map(|days| activated_at + Duration::try_days(days as i64).unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_properties() {
        // Tier 1 (Critical)
        assert_eq!(EmergencyTier::Critical.review_period_days(), 0);
        assert_eq!(EmergencyTier::Critical.signature_threshold(), (4, 7));
        assert_eq!(EmergencyTier::Critical.max_duration_days(), 7);
        assert!(!EmergencyTier::Critical.allows_extensions());
        assert!(EmergencyTier::Critical.requires_security_audit());

        // Tier 2 (Urgent)
        assert_eq!(EmergencyTier::Urgent.review_period_days(), 7);
        assert_eq!(EmergencyTier::Urgent.signature_threshold(), (5, 7));
        assert_eq!(EmergencyTier::Urgent.max_duration_days(), 30);
        assert!(EmergencyTier::Urgent.allows_extensions());
        assert!(!EmergencyTier::Urgent.requires_security_audit());

        // Tier 3 (Elevated)
        assert_eq!(EmergencyTier::Elevated.review_period_days(), 30);
        assert_eq!(EmergencyTier::Elevated.signature_threshold(), (6, 7));
        assert_eq!(EmergencyTier::Elevated.max_duration_days(), 90);
        assert!(EmergencyTier::Elevated.allows_extensions());
        assert_eq!(EmergencyTier::Elevated.max_extensions(), 2);
    }

    #[test]
    fn test_tier_parsing() {
        assert_eq!(EmergencyTier::from_i32(1).unwrap(), EmergencyTier::Critical);
        assert_eq!(EmergencyTier::from_i32(2).unwrap(), EmergencyTier::Urgent);
        assert_eq!(EmergencyTier::from_i32(3).unwrap(), EmergencyTier::Elevated);
        assert!(EmergencyTier::from_i32(4).is_err());
    }

    #[test]
    fn test_active_emergency_expiration() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Critical,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now() - Duration::try_days(10).unwrap_or_default(),
            expires_at: Utc::now() - Duration::try_days(1).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        assert!(emergency.is_expired());
        assert!(!emergency.can_extend()); // Critical doesn't allow extensions
    }

    #[test]
    fn test_active_emergency_extension() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now() - Duration::try_days(20).unwrap_or_default(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        assert!(!emergency.is_expired());
        assert!(emergency.can_extend()); // Urgent allows 1 extension
        assert!(emergency.calculate_extension_expiration().is_some());
    }

    #[test]
    fn test_emergency_tier_to_i32() {
        assert_eq!(EmergencyTier::Critical.to_i32(), 1);
        assert_eq!(EmergencyTier::Urgent.to_i32(), 2);
        assert_eq!(EmergencyTier::Elevated.to_i32(), 3);
    }

    #[test]
    fn test_emergency_tier_activation_threshold() {
        // All tiers require 5-of-7 emergency keyholders to activate
        assert_eq!(EmergencyTier::Critical.activation_threshold(), (5, 7));
        assert_eq!(EmergencyTier::Urgent.activation_threshold(), (5, 7));
        assert_eq!(EmergencyTier::Elevated.activation_threshold(), (5, 7));
    }

    #[test]
    fn test_emergency_tier_max_extensions() {
        assert_eq!(EmergencyTier::Critical.max_extensions(), 0);
        assert_eq!(EmergencyTier::Urgent.max_extensions(), 1);
        assert_eq!(EmergencyTier::Elevated.max_extensions(), 2);
    }

    #[test]
    fn test_emergency_tier_extension_duration_days() {
        assert_eq!(EmergencyTier::Critical.extension_duration_days(), 0);
        assert_eq!(EmergencyTier::Urgent.extension_duration_days(), 30);
        assert_eq!(EmergencyTier::Elevated.extension_duration_days(), 30);
    }

    #[test]
    fn test_emergency_tier_extension_threshold() {
        assert_eq!(EmergencyTier::Critical.extension_threshold(), (0, 0));
        assert_eq!(EmergencyTier::Urgent.extension_threshold(), (6, 7));
        assert_eq!(EmergencyTier::Elevated.extension_threshold(), (6, 7));
    }

    #[test]
    fn test_emergency_tier_post_mortem_deadline_days() {
        assert_eq!(EmergencyTier::Critical.post_mortem_deadline_days(), 30);
        assert_eq!(EmergencyTier::Urgent.post_mortem_deadline_days(), 60);
        assert_eq!(EmergencyTier::Elevated.post_mortem_deadline_days(), 90);
    }

    #[test]
    fn test_emergency_tier_security_audit_deadline_days() {
        assert_eq!(
            EmergencyTier::Critical.security_audit_deadline_days(),
            Some(60)
        );
        assert_eq!(EmergencyTier::Urgent.security_audit_deadline_days(), None);
        assert_eq!(EmergencyTier::Elevated.security_audit_deadline_days(), None);
    }

    #[test]
    fn test_emergency_tier_name() {
        assert_eq!(EmergencyTier::Critical.name(), "Critical Emergency");
        assert_eq!(EmergencyTier::Urgent.name(), "Urgent Security Issue");
        assert_eq!(EmergencyTier::Elevated.name(), "Elevated Priority");
    }

    #[test]
    fn test_emergency_tier_emoji() {
        assert_eq!(EmergencyTier::Critical.emoji(), "🚨");
        assert_eq!(EmergencyTier::Urgent.emoji(), "⚠️");
        assert_eq!(EmergencyTier::Elevated.emoji(), "📢");
    }

    #[test]
    fn test_emergency_tier_description() {
        assert!(
            EmergencyTier::Critical
                .description()
                .contains("Network-threatening")
        );
        assert!(
            EmergencyTier::Urgent
                .description()
                .contains("Serious security")
        );
        assert!(
            EmergencyTier::Elevated
                .description()
                .contains("Important priority")
        );
    }

    #[test]
    fn test_active_emergency_remaining_duration() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Critical,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(7).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        let remaining = emergency.remaining_duration();
        assert!(remaining.num_days() >= 6);
        assert!(remaining.num_days() <= 7);
    }

    #[test]
    fn test_active_emergency_can_extend_critical() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Critical,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(5).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        assert!(!emergency.can_extend()); // Critical doesn't allow extensions
        assert!(emergency.calculate_extension_expiration().is_none());
    }

    #[test]
    fn test_active_emergency_can_extend_urgent() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        assert!(emergency.can_extend()); // Urgent allows 1 extension
        assert!(emergency.calculate_extension_expiration().is_some());
    }

    #[test]
    fn test_active_emergency_cannot_extend_when_max_reached() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: true,
            extension_count: 1, // Already at max
        };

        assert!(!emergency.can_extend()); // Max extensions reached
        assert!(emergency.calculate_extension_expiration().is_none());
    }

    #[test]
    fn test_active_emergency_cannot_extend_when_expired() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now() - Duration::try_days(40).unwrap_or_default(),
            expires_at: Utc::now() - Duration::try_days(1).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        assert!(emergency.is_expired());
        assert!(!emergency.can_extend()); // Can't extend expired emergency
        assert!(emergency.calculate_extension_expiration().is_none());
    }

    #[test]
    fn test_validate_activation_insufficient_evidence() {
        let activation = EmergencyActivation {
            tier: EmergencyTier::Critical,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            evidence: "short".to_string(), // Less than 100 chars
            signatures: vec![],
        };

        let result = EmergencyValidator::validate_activation(&activation);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("Insufficient evidence"));
            }
            _ => panic!("Expected InsufficientEvidence error"),
        }
    }

    #[test]
    fn test_validate_activation_insufficient_signatures() {
        let activation = EmergencyActivation {
            tier: EmergencyTier::Critical,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            evidence: "x".repeat(100), // Sufficient evidence
            signatures: vec![],        // Need 5-of-7, have 0
        };

        let result = EmergencyValidator::validate_activation(&activation);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("Insufficient signatures"));
            }
            _ => panic!("Expected InsufficientSignatures error"),
        }
    }

    #[test]
    fn test_validate_extension_not_allowed() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Critical, // Doesn't allow extensions
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(5).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        let result = EmergencyValidator::validate_extension(&emergency, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("Extensions not allowed"));
            }
            _ => panic!("Expected ExtensionNotAllowed error"),
        }
    }

    #[test]
    fn test_validate_extension_max_reached() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: true,
            extension_count: 1, // Already at max
        };

        let result = EmergencyValidator::validate_extension(&emergency, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("Maximum extensions reached"));
            }
            _ => panic!("Expected MaxExtensionsReached error"),
        }
    }

    #[test]
    fn test_validate_extension_expired() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now() - Duration::try_days(40).unwrap_or_default(),
            expires_at: Utc::now() - Duration::try_days(1).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        let result = EmergencyValidator::validate_extension(&emergency, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("expired"));
            }
            _ => panic!("Expected EmergencyExpired error"),
        }
    }

    #[test]
    fn test_validate_extension_insufficient_signatures() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        // Need 6-of-7 signatures for extension, have 0
        let result = EmergencyValidator::validate_extension(&emergency, &[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceAppError::ValidationError(msg) => {
                assert!(msg.contains("Insufficient signatures"));
            }
            _ => panic!("Expected InsufficientSignatures error"),
        }
    }

    #[test]
    fn test_check_expiration() {
        let active_emergencies = vec![
            ActiveEmergency {
                id: 1,
                tier: EmergencyTier::Critical,
                activated_by: "alice".to_string(),
                reason: "Test 1".to_string(),
                activated_at: Utc::now() - Duration::try_days(10).unwrap_or_default(),
                expires_at: Utc::now() - Duration::try_days(1).unwrap_or_default(), // Expired
                extended: false,
                extension_count: 0,
            },
            ActiveEmergency {
                id: 2,
                tier: EmergencyTier::Urgent,
                activated_by: "bob".to_string(),
                reason: "Test 2".to_string(),
                activated_at: Utc::now(),
                expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(), // Not expired
                extended: false,
                extension_count: 0,
            },
            ActiveEmergency {
                id: 3,
                tier: EmergencyTier::Elevated,
                activated_by: "charlie".to_string(),
                reason: "Test 3".to_string(),
                activated_at: Utc::now() - Duration::try_days(100).unwrap_or_default(),
                expires_at: Utc::now() - Duration::try_days(1).unwrap_or_default(), // Expired
                extended: false,
                extension_count: 0,
            },
        ];

        let expired = EmergencyValidator::check_expiration(&active_emergencies);
        assert_eq!(expired.len(), 2);
        assert!(expired.contains(&1));
        assert!(expired.contains(&3));
        assert!(!expired.contains(&2));
    }

    #[test]
    fn test_check_expiration_empty() {
        let expired = EmergencyValidator::check_expiration(&[]);
        assert_eq!(expired.len(), 0);
    }

    #[test]
    fn test_calculate_expiration() {
        let expiration = EmergencyValidator::calculate_expiration(EmergencyTier::Critical);
        let expected = Utc::now() + Duration::try_days(7).unwrap_or_default();

        // Allow small time difference
        let diff = (expiration - expected).num_seconds().abs();
        assert!(diff < 5);
    }

    #[test]
    fn test_calculate_post_mortem_deadline() {
        let activated_at = Utc::now();
        let deadline = EmergencyValidator::calculate_post_mortem_deadline(
            EmergencyTier::Critical,
            activated_at,
        );

        let expected = activated_at + Duration::try_days(30).unwrap_or_default();
        let diff = (deadline - expected).num_seconds().abs();
        assert!(diff < 5);
    }

    #[test]
    fn test_calculate_security_audit_deadline_critical() {
        let activated_at = Utc::now();
        let deadline = EmergencyValidator::calculate_security_audit_deadline(
            EmergencyTier::Critical,
            activated_at,
        );

        assert!(deadline.is_some());
        let expected = activated_at + Duration::try_days(60).unwrap_or_default();
        let diff = (deadline.unwrap() - expected).num_seconds().abs();
        assert!(diff < 5);
    }

    #[test]
    fn test_calculate_security_audit_deadline_urgent() {
        let activated_at = Utc::now();
        let deadline = EmergencyValidator::calculate_security_audit_deadline(
            EmergencyTier::Urgent,
            activated_at,
        );

        assert!(deadline.is_none()); // Urgent doesn't require audit
    }

    #[test]
    fn test_calculate_security_audit_deadline_elevated() {
        let activated_at = Utc::now();
        let deadline = EmergencyValidator::calculate_security_audit_deadline(
            EmergencyTier::Elevated,
            activated_at,
        );

        assert!(deadline.is_none()); // Elevated doesn't require audit
    }

    #[test]
    fn test_active_emergency_extension_expiration_calculation() {
        let emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Urgent,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(10).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        let extension_expiration = emergency.calculate_extension_expiration();
        assert!(extension_expiration.is_some());

        let expected = emergency.expires_at + Duration::try_days(30).unwrap_or_default();
        let diff = (extension_expiration.unwrap() - expected)
            .num_seconds()
            .abs();
        assert!(diff < 5);
    }

    #[test]
    fn test_elevated_tier_multiple_extensions() {
        let mut emergency = ActiveEmergency {
            id: 1,
            tier: EmergencyTier::Elevated,
            activated_by: "alice".to_string(),
            reason: "Test".to_string(),
            activated_at: Utc::now(),
            expires_at: Utc::now() + Duration::try_days(50).unwrap_or_default(),
            extended: false,
            extension_count: 0,
        };

        // First extension allowed
        assert!(emergency.can_extend());

        emergency.extension_count = 1;
        // Second extension allowed
        assert!(emergency.can_extend());

        emergency.extension_count = 2;
        // Max extensions reached
        assert!(!emergency.can_extend());
    }
}
