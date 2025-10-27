use thiserror::Error;

impl From<serde_json::Error> for GovernanceError {
    fn from(err: serde_json::Error) -> Self {
        Self::CryptoError(format!("JSON serialization error: {}", err))
    }
}

impl From<sqlx::Error> for GovernanceError {
    fn from(err: sqlx::Error) -> Self {
        Self::DatabaseError(format!("Database error: {}", err))
    }
}

#[derive(Error, Debug)]
pub enum GovernanceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("GitHub API error: {0}")]
    GitHubError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Cryptographic error: {0}")]
    CryptoError(String),

    #[error("Webhook processing error: {0}")]
    WebhookError(String),

    #[error("Signature verification failed: {0}")]
    SignatureError(String),

    #[error("Review period not met: {0}")]
    ReviewPeriodError(String),

    #[error("Threshold not satisfied: {0}")]
    ThresholdError(String),
}

// Type alias for compatibility with emergency module
pub type GovernanceAppError = GovernanceError;

// Additional error variants for emergency tier system
impl GovernanceError {
    pub fn invalid_emergency_tier(tier: i32) -> Self {
        Self::ValidationError(format!(
            "Invalid emergency tier: {}. Must be 1, 2, or 3",
            tier
        ))
    }

    pub fn insufficient_evidence(length: usize) -> Self {
        Self::ValidationError(format!(
            "Insufficient evidence: {} characters (minimum 100 required)",
            length
        ))
    }

    pub fn insufficient_signatures(required: usize, found: usize, threshold: String) -> Self {
        Self::ValidationError(format!(
            "Insufficient signatures: found {}, required {} (threshold: {})",
            found, required, threshold
        ))
    }

    pub fn invalid_signature(msg: String) -> Self {
        Self::SignatureError(msg)
    }

    pub fn extension_not_allowed(tier: String) -> Self {
        Self::ValidationError(format!("Extensions not allowed for tier: {}", tier))
    }

    pub fn max_extensions_reached(current: u32, max: u32) -> Self {
        Self::ValidationError(format!(
            "Maximum extensions reached: {} of {} used",
            current, max
        ))
    }

    pub fn emergency_expired(id: i32) -> Self {
        Self::ValidationError(format!("Emergency tier {} has expired", id))
    }
}

// Helper functions that match emergency.rs error constructors
impl GovernanceError {
    pub fn InvalidEmergencyTier(tier: i32) -> Self {
        Self::invalid_emergency_tier(tier)
    }

    pub fn InsufficientEvidence(length: usize) -> Self {
        Self::insufficient_evidence(length)
    }

    pub fn InsufficientSignatures(args: InsufficientSignaturesArgs) -> Self {
        Self::insufficient_signatures(args.required, args.found, args.threshold)
    }

    pub fn InvalidSignature(msg: String) -> Self {
        Self::invalid_signature(msg)
    }

    pub fn ExtensionNotAllowed(tier: String) -> Self {
        Self::extension_not_allowed(tier)
    }

    pub fn MaxExtensionsReached(args: MaxExtensionsReachedArgs) -> Self {
        Self::max_extensions_reached(args.current, args.max)
    }

    pub fn EmergencyExpired(id: i32) -> Self {
        Self::emergency_expired(id)
    }
}

pub struct InsufficientSignaturesArgs {
    pub required: usize,
    pub found: usize,
    pub threshold: String,
}

pub struct MaxExtensionsReachedArgs {
    pub current: u32,
    pub max: u32,
}
