// Database schema definitions and migrations
// This module contains the SQL schema for the governance app database

pub const INITIAL_SCHEMA: &str = include_str!("../../migrations/001_initial_schema.sql");
pub const EMERGENCY_MODE_SCHEMA: &str = include_str!("../../migrations/002_emergency_mode.sql");
pub const AUDIT_LOG_SCHEMA: &str = include_str!("../../migrations/003_audit_log.sql");
