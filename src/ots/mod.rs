//! OpenTimestamps Integration Module
//!
//! This module provides historical proof of governance operations
//! by anchoring monthly registries to the Bitcoin blockchain.

pub mod client;
pub mod anchor;
pub mod verify;

pub use client::OtsClient;
pub use anchor::RegistryAnchorer;
pub use verify::verify_registry;
