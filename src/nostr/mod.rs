//! Nostr Integration Module
//!
//! This module provides real-time transparency for governance operations
//! by publishing status updates to the Nostr protocol.

pub mod bot_manager;
pub mod client;
pub mod events;
pub mod governance_publisher;
pub mod helpers;
pub mod publisher;
pub mod zap_tracker;
pub mod zap_voting;

pub use client::{NostrClient, ZapEvent};
pub use events::{
    CombinedRequirement, KeyholderAnnouncement, KeyholderSignature, LayerRequirement,
    TierRequirement,
};
pub use governance_publisher::GovernanceActionPublisher;
pub use helpers::publish_merge_action;
pub use publisher::StatusPublisher;
pub use zap_tracker::ZapTracker;
