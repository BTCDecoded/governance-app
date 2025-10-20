//! Emergency tier validation and management
//!
//! Implements the three-tiered emergency response system:
//! - Tier 1 (Critical): 0 day review, 4-of-7 signatures, 7 day max duration
//! - Tier 2 (Urgent): 7 day review, 5-of-7 signatures, 30 day max duration
//! - Tier 3 (Elevated): 30 day review, 6-of-7 signatures, 90 day max duration

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use crate::error::{InsufficientSignaturesArgs, MaxExtensionsReachedArgs};

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
            EmergencyTier::Critical => "Network-threatening vulnerability requiring immediate action",
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
        Some(self.expires_at + Duration::days(extension_days))
    }
}

/// Emergency tier validator
pub struct EmergencyValidator;

impl EmergencyValidator {
    /// Validate emergency activation request
    pub fn validate_activation(
        activation: &EmergencyActivation,
    ) -> Result<(), GovernanceAppError> {
        // Check minimum evidence length
        if activation.evidence.len() < 100 {
            return Err(GovernanceAppError::InsufficientEvidence(
                activation.evidence.len(),
            ));
        }

        // Check signature count meets activation threshold
        let (required, total) = activation.tier.activation_threshold();
        if activation.signatures.len() < required as usize {
            return Err(GovernanceAppError::InsufficientSignatures(InsufficientSignaturesArgs {
                required: required as usize,
                found: activation.signatures.len(),
                threshold: format!("{}-of-{}", required, total),
            }));
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
        _activation: &EmergencyActivation,
    ) -> Result<(), GovernanceAppError> {
        // TODO: Implement actual cryptographic verification using developer-sdk
        // For now, just basic validation
        
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
            return Err(GovernanceAppError::MaxExtensionsReached(MaxExtensionsReachedArgs {
                current: emergency.extension_count,
                max: emergency.tier.max_extensions(),
            }));
        }

        // Check if already expired
        if emergency.is_expired() {
            return Err(GovernanceAppError::EmergencyExpired(emergency.id));
        }

        // Check signature count meets extension threshold
        let (required, total) = emergency.tier.extension_threshold();
        if signatures.len() < required as usize {
            return Err(GovernanceAppError::InsufficientSignatures(InsufficientSignaturesArgs {
                required: required as usize,
                found: signatures.len(),
                threshold: format!("{}-of-{}", required, total),
            }));
        }

        Ok(())
    }

    /// Check for expired emergencies
    pub fn check_expiration(
        active_emergencies: &[ActiveEmergency],
    ) -> Vec<i32> {
        active_emergencies
            .iter()
            .filter(|e| e.is_expired())
            .map(|e| e.id)
            .collect()
    }

    /// Calculate expiration timestamp for new emergency
    pub fn calculate_expiration(tier: EmergencyTier) -> DateTime<Utc> {
        let duration_days = tier.max_duration_days() as i64;
        Utc::now() + Duration::days(duration_days)
    }

    /// Calculate post-mortem deadline
    pub fn calculate_post_mortem_deadline(
        tier: EmergencyTier,
        activated_at: DateTime<Utc>,
    ) -> DateTime<Utc> {
        let deadline_days = tier.post_mortem_deadline_days() as i64;
        activated_at + Duration::days(deadline_days)
    }

    /// Calculate security audit deadline (if required)
    pub fn calculate_security_audit_deadline(
        tier: EmergencyTier,
        activated_at: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        tier.security_audit_deadline_days()
            .map(|days| activated_at + Duration::days(days as i64))
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
            activated_at: Utc::now() - Duration::days(10),
            expires_at: Utc::now() - Duration::days(1),
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
            activated_at: Utc::now() - Duration::days(20),
            expires_at: Utc::now() + Duration::days(10),
            extended: false,
            extension_count: 0,
        };

        assert!(!emergency.is_expired());
        assert!(emergency.can_extend()); // Urgent allows 1 extension
        assert!(emergency.calculate_extension_expiration().is_some());
    }
}


