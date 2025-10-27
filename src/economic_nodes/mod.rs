//! Economic Node Registry and Veto System
//!
//! Handles registration, qualification verification, and veto signal collection
//! for economic nodes (mining pools, exchanges, custodians, etc.)

pub mod registry;
pub mod types;
pub mod veto;

pub use registry::EconomicNodeRegistry;
pub use types::*;
pub use veto::VetoManager;




