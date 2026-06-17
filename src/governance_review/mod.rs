//! Governance Review System
//!
//! Implements the maintainer governance review policy with:
//! - Graduated sanctions (private warning, public warning, removal)
//! - Time limits (180 days for cases, 90 days for appeals)
//! - Protections (whistleblower, false reports, retaliation)
//! - Conflict resolution/mediation
//! - On-platform only (off-platform activity disregarded)

pub mod appeals;
pub mod case;
pub mod deadline_notifications;
pub mod env;
pub mod github_integration;
pub mod mediation;
pub mod models;
pub mod protections;
pub mod removal;
pub mod response;
pub mod sanctions;
pub mod time_limits;

pub use case::GovernanceReviewCaseManager;
