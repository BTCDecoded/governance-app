//! Governance Fork Capability
//!
//! Handles governance ruleset export, versioning, adoption tracking, and fork support

pub mod adoption;
pub mod dashboard;
pub mod detection;
pub mod executor;
pub mod export;
pub mod types;
pub mod versioning;

pub use adoption::AdoptionTracker;
pub use dashboard::AdoptionDashboard;
pub use detection::{ForkDetector, ForkDetectionEvent, ForkTriggerType, ForkAction};
pub use executor::{ForkExecutor, ForkStatus};
pub use export::GovernanceExporter;
pub use types::*;
pub use versioning::RulesetVersioning;




