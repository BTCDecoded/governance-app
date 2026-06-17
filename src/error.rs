use thiserror::Error;

impl From<serde_json::Error> for GovernanceError {
    fn from(err: serde_json::Error) -> Self {
        Self::CryptoError(format!("JSON serialization error: {err}"))
    }
}

impl From<sqlx::Error> for GovernanceError {
    fn from(err: sqlx::Error) -> Self {
        Self::DatabaseError(format!("Database error: {err}"))
    }
}

impl From<octocrab::Error> for GovernanceError {
    fn from(err: octocrab::Error) -> Self {
        Self::GitHubError(format!("GitHub API error: {err}"))
    }
}

impl From<reqwest::Error> for GovernanceError {
    fn from(err: reqwest::Error) -> Self {
        Self::GitHubError(format!("HTTP error: {err}"))
    }
}

impl From<std::io::Error> for GovernanceError {
    fn from(err: std::io::Error) -> Self {
        Self::ConfigError(format!("IO error: {err}"))
    }
}

impl From<anyhow::Error> for GovernanceError {
    fn from(err: anyhow::Error) -> Self {
        Self::ConfigError(format!("Error: {err}"))
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

    #[error("Build orchestration error: {0}")]
    BuildError(String),
}

// Type alias for compatibility with emergency module
pub type GovernanceAppError = GovernanceError;

// Additional error variants for emergency tier system
impl GovernanceError {
    pub fn invalid_emergency_tier(tier: i32) -> Self {
        Self::ValidationError(format!(
            "Invalid emergency tier: {tier}. Must be 1, 2, or 3"
        ))
    }

    pub fn insufficient_evidence(length: usize) -> Self {
        Self::ValidationError(format!(
            "Insufficient evidence: {length} characters (minimum 100 required)"
        ))
    }

    pub fn insufficient_signatures(required: usize, found: usize, threshold: String) -> Self {
        Self::ValidationError(format!(
            "Insufficient signatures: found {found}, required {required} (threshold: {threshold})"
        ))
    }

    pub fn invalid_signature(msg: String) -> Self {
        Self::SignatureError(msg)
    }

    pub fn extension_not_allowed(tier: String) -> Self {
        Self::ValidationError(format!("Extensions not allowed for tier: {tier}"))
    }

    pub fn max_extensions_reached(current: u32, max: u32) -> Self {
        Self::ValidationError(format!(
            "Maximum extensions reached: {current} of {max} used"
        ))
    }

    pub fn emergency_expired(id: i32) -> Self {
        Self::ValidationError(format!("Emergency tier {id} has expired"))
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

/// Type alias for Result with GovernanceError
pub type Result<T> = std::result::Result<T, GovernanceError>;
