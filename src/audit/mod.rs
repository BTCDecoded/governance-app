//! Audit Log System
//!
//! Provides tamper-evident logging for all governance operations
//! with cryptographic hash chains and Merkle tree anchoring.

pub mod entry;
pub mod logger;
pub mod verify;
pub mod merkle;

pub use entry::AuditLogEntry;
pub use logger::AuditLogger;
pub use verify::{verify_audit_log, verify_audit_log_file, load_audit_log_from_file};
pub use merkle::{build_merkle_tree, verify_merkle_root};
