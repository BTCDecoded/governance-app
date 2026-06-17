//! Build orchestration module
//!
//! Handles cross-repository build coordination, dependency management,
//! build monitoring, and artifact collection for releases.

pub mod artifacts;
pub mod dependency;
pub mod monitor;
pub mod orchestrator;

#[cfg(test)]
mod tests;
