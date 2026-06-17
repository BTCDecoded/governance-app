//! Governance Module
//!
//! Handles governance contribution tracking, weight calculation, and voting.

pub mod aggregator;
pub mod challenge;
pub mod contributions;
pub mod phase_calculator;
pub mod time_lock;
pub mod vote_aggregator;
pub mod weight_calculator;

pub use aggregator::ContributionAggregator;
pub use contributions::ContributionTracker;
pub use weight_calculator::WeightCalculator;
