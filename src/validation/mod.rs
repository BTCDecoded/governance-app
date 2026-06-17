pub mod content_hash;
pub mod cross_layer;
pub mod diff_parser;
pub mod emergency;
pub mod equivalence_proof;
pub mod nested_multisig;
pub mod review_period;
pub mod security_controls;
pub mod signatures;
pub mod teams_loader;
pub mod threshold;
pub mod tier_classification;
pub mod verification_check;
pub mod version_pinning;

use serde::{Deserialize, Serialize};

/// Result of validation checks
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)] // Used across multiple modules
pub enum ValidationResult {
    /// Validation passed
    Valid { message: String },
    /// Validation failed
    Invalid { message: String, blocking: bool },
    /// Validation is still pending
    Pending { message: String },
    /// Validation not applicable to this case
    NotApplicable,
}
