//! Server Authorization Module
//!
//! Manages authorized governance servers and their verification
//! to prevent unauthorized servers from masquerading as official infrastructure.

pub mod server;
pub mod verification;

pub use server::{AuthorizedServer, ServerStatus, OperatorInfo, ServerKeys, InfrastructureInfo};
pub use verification::verify_server_authorization;
