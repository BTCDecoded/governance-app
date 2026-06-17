//! Audit Log System
//!
//! Provides tamper-evident logging for all governance operations
//! with cryptographic hash chains and Merkle tree anchoring.

pub mod entry;
pub mod logger;
pub mod merkle;
pub mod verify;

pub use logger::AuditLogger;
