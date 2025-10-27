//! Nostr Integration Module
//!
//! This module provides real-time transparency for governance operations
//! by publishing status updates to the Nostr protocol.

pub mod client;
pub mod publisher;
pub mod events;

pub use client::NostrClient;
pub use publisher::StatusPublisher;
pub use events::{GovernanceStatus, ServerHealth, Hashes};
