//! Server Authorization Verification
//!
//! Provides utilities for verifying server authorization and managing
//! the authorized server registry.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::authorization::server::{AuthorizedServer, ServerStatus};
use crate::ots::anchor::GovernanceRegistry;

/// Verify if a server is authorized
pub fn verify_server_authorization(
    server_id: &str,
    nostr_npub: &str,
    registry: &GovernanceRegistry,
) -> Result<bool> {
    debug!("Verifying server authorization: {} with npub {}", server_id, nostr_npub);

    // Check if server is in authorized list
    let server = registry.authorized_servers
        .iter()
        .find(|s| s.server_id == server_id && s.keys.nostr_npub == nostr_npub);

    match server {
        Some(s) if s.status == "active" => {
            info!("Server {} is authorized and active", server_id);
            Ok(true)
        }
        Some(s) if s.status == "compromised" => {
            warn!("Server {} is marked as compromised", server_id);
            Err(anyhow!("Server {} is marked as compromised", server_id))
        }
        Some(s) => {
            warn!("Server {} is not active (status: {})", server_id, s.status);
            Ok(false)
        }
        None => {
            warn!("Server {} not found in authorized list", server_id);
            Ok(false)
        }
    }
}

/// Verify server authorization with detailed result
pub fn verify_server_authorization_detailed(
    server_id: &str,
    nostr_npub: &str,
    registry: &GovernanceRegistry,
) -> ServerVerificationResult {
    debug!("Detailed verification for server: {} with npub {}", server_id, nostr_npub);

    // Check if server exists
    let server = registry.authorized_servers
        .iter()
        .find(|s| s.server_id == server_id);

    match server {
        Some(s) => {
            // Check NPUB match
            if s.keys.nostr_npub != nostr_npub {
                return ServerVerificationResult {
                    is_authorized: false,
                    is_active: false,
                    is_compromised: false,
                    error_message: Some("NPUB does not match".to_string()),
                    server_info: Some(s.clone().into()),
                };
            }

            // Check status
            let is_active = s.status == "active";
            let is_compromised = s.status == "compromised";
            let is_authorized = is_active && !is_compromised;

            let error_message = if is_compromised {
                Some("Server is marked as compromised".to_string())
            } else if !is_active {
                Some(format!("Server is not active (status: {})", s.status))
            } else {
                None
            };

            ServerVerificationResult {
                is_authorized,
                is_active,
                is_compromised,
                error_message,
                server_info: Some(s.clone().into()),
            }
        }
        None => {
            ServerVerificationResult {
                is_authorized: false,
                is_active: false,
                is_compromised: false,
                error_message: Some("Server not found in authorized list".to_string()),
                server_info: None,
            }
        }
    }
}

/// Server verification result
#[derive(Debug, Clone)]
pub struct ServerVerificationResult {
    pub is_authorized: bool,
    pub is_active: bool,
    pub is_compromised: bool,
    pub error_message: Option<String>,
    pub server_info: Option<AuthorizedServer>,
}

impl ServerVerificationResult {
    /// Get human-readable summary
    pub fn summary(&self) -> String {
        if self.is_authorized {
            "✅ Server is authorized and active".to_string()
        } else if self.is_compromised {
            "❌ Server is compromised".to_string()
        } else if !self.is_active {
            "⚠️ Server is not active".to_string()
        } else {
            "❌ Server is not authorized".to_string()
        }
    }

    /// Get detailed status
    pub fn detailed_status(&self) -> String {
        let mut status = vec![
            format!("Authorized: {}", if self.is_authorized { "✅" } else { "❌" }),
            format!("Active: {}", if self.is_active { "✅" } else { "❌" }),
            format!("Compromised: {}", if self.is_compromised { "⚠️" } else { "✅" }),
        ];

        if let Some(error) = &self.error_message {
            status.push(format!("Error: {}", error));
        }

        if let Some(server) = &self.server_info {
            status.push(format!("Server: {}", server.summary()));
        }

        status.join("\n")
    }
}

/// Get all authorized servers from registry
pub fn get_authorized_servers(registry: &GovernanceRegistry) -> Vec<crate::authorization::server::AuthorizedServer> {
    registry.authorized_servers
        .iter()
        .filter(|s| s.status == "active")
        .map(|s| s.clone().into())
        .collect()
}

/// Get all servers by status
pub fn get_servers_by_status(
    registry: &GovernanceRegistry,
    status: ServerStatus,
) -> Vec<crate::authorization::server::AuthorizedServer> {
    registry.authorized_servers
        .iter()
        .filter(|s| s.status == status.to_string())
        .map(|s| s.clone().into())
        .collect()
}

/// Get server by ID
pub fn get_server_by_id<'a>(
    registry: &'a GovernanceRegistry,
    server_id: &'a str,
) -> Option<crate::authorization::server::AuthorizedServer> {
    registry.authorized_servers
        .iter()
        .find(|s| s.server_id == server_id)
        .map(|s| s.clone().into())
}

/// Get server by NPUB
pub fn get_server_by_npub<'a>(
    registry: &'a GovernanceRegistry,
    nostr_npub: &'a str,
) -> Option<crate::authorization::server::AuthorizedServer> {
    registry.authorized_servers
        .iter()
        .find(|s| s.keys.nostr_npub == nostr_npub)
        .map(|s| s.clone().into())
}

/// Check if server exists in registry
pub fn server_exists(registry: &GovernanceRegistry, server_id: &str) -> bool {
    registry.authorized_servers
        .iter()
        .any(|s| s.server_id == server_id)
}

/// Get server statistics
pub fn get_server_statistics(registry: &GovernanceRegistry) -> ServerStatistics {
    let total_servers = registry.authorized_servers.len();
    let active_servers = get_servers_by_status(registry, ServerStatus::Active).len();
    let compromised_servers = get_servers_by_status(registry, ServerStatus::Compromised).len();
    let inactive_servers = get_servers_by_status(registry, ServerStatus::Inactive).len();
    let retiring_servers = get_servers_by_status(registry, ServerStatus::Retiring).len();

    ServerStatistics {
        total_servers,
        active_servers,
        compromised_servers,
        inactive_servers,
        retiring_servers,
    }
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStatistics {
    pub total_servers: usize,
    pub active_servers: usize,
    pub compromised_servers: usize,
    pub inactive_servers: usize,
    pub retiring_servers: usize,
}

impl ServerStatistics {
    /// Get summary string
    pub fn summary(&self) -> String {
        format!(
            "Servers: {} total, {} active, {} compromised, {} inactive, {} retiring",
            self.total_servers,
            self.active_servers,
            self.compromised_servers,
            self.inactive_servers,
            self.retiring_servers
        )
    }

    /// Get health percentage
    pub fn health_percentage(&self) -> f64 {
        if self.total_servers == 0 {
            0.0
        } else {
            (self.active_servers as f64 / self.total_servers as f64) * 100.0
        }
    }
}

/// Validate server configuration
pub fn validate_server_config(server: &AuthorizedServer) -> Result<()> {
    // Check required fields
    if server.server_id.is_empty() {
        return Err(anyhow!("Server ID cannot be empty"));
    }

    if server.operator.name.is_empty() {
        return Err(anyhow!("Operator name cannot be empty"));
    }

    if server.operator.jurisdiction.is_empty() {
        return Err(anyhow!("Operator jurisdiction cannot be empty"));
    }

    if server.keys.nostr_npub.is_empty() {
        return Err(anyhow!("Nostr NPUB cannot be empty"));
    }

    if server.keys.ssh_fingerprint.is_empty() {
        return Err(anyhow!("SSH fingerprint cannot be empty"));
    }

    // Validate NPUB format (basic check)
    if !server.keys.nostr_npub.starts_with("npub1") {
        return Err(anyhow!("Invalid NPUB format"));
    }

    // Validate SSH fingerprint format (basic check)
    if !server.keys.ssh_fingerprint.starts_with("SHA256:") {
        return Err(anyhow!("Invalid SSH fingerprint format"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ots::anchor::{GovernanceRegistry, Maintainer, MultisigConfig};
    use std::collections::HashMap;

    fn create_test_registry() -> GovernanceRegistry {
        GovernanceRegistry {
            version: "2025-01".to_string(),
            timestamp: chrono::Utc::now(),
            previous_registry_hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            maintainers: vec![],
            authorized_servers: vec![
                AuthorizedServer {
                    server_id: "governance-01".to_string(),
                    operator: crate::authorization::server::OperatorInfo {
                        name: "Alice".to_string(),
                        jurisdiction: "United States".to_string(),
                        contact: Some("alice@example.com".to_string()),
                    },
                    keys: crate::authorization::server::ServerKeys {
                        nostr_npub: "npub1abc123".to_string(),
                        ssh_fingerprint: "SHA256:xyz789".to_string(),
                    },
                    infrastructure: crate::authorization::server::InfrastructureInfo {
                        vpn_ip: Some("10.0.0.2".to_string()),
                        github_runner: true,
                        ots_enabled: true,
                    },
                    status: ServerStatus::Active,
                    added_at: chrono::Utc::now(),
                    last_verified: None,
                },
                AuthorizedServer {
                    server_id: "governance-02".to_string(),
                    operator: crate::authorization::server::OperatorInfo {
                        name: "Bob".to_string(),
                        jurisdiction: "European Union".to_string(),
                        contact: Some("bob@example.com".to_string()),
                    },
                    keys: crate::authorization::server::ServerKeys {
                        nostr_npub: "npub1def456".to_string(),
                        ssh_fingerprint: "SHA256:uvw012".to_string(),
                    },
                    infrastructure: crate::authorization::server::InfrastructureInfo {
                        vpn_ip: Some("10.0.0.3".to_string()),
                        github_runner: true,
                        ots_enabled: true,
                    },
                    status: ServerStatus::Compromised,
                    added_at: chrono::Utc::now(),
                    last_verified: None,
                },
            ],
            audit_logs: HashMap::new(),
            multisig_config: MultisigConfig {
                required_signatures: 3,
                total_maintainers: 5,
            },
        }
    }

    #[test]
    fn test_verify_server_authorization() {
        let registry = create_test_registry();
        
        // Test authorized server
        let result = verify_server_authorization("governance-01", "npub1abc123", &registry);
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Test unauthorized server
        let result = verify_server_authorization("governance-01", "npub1wrong", &registry);
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Test compromised server
        let result = verify_server_authorization("governance-02", "npub1def456", &registry);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_server_authorization_detailed() {
        let registry = create_test_registry();
        
        let result = verify_server_authorization_detailed("governance-01", "npub1abc123", &registry);
        assert!(result.is_authorized);
        assert!(result.is_active);
        assert!(!result.is_compromised);
        assert!(result.server_info.is_some());
    }

    #[test]
    fn test_get_authorized_servers() {
        let registry = create_test_registry();
        let authorized = get_authorized_servers(&registry);
        assert_eq!(authorized.len(), 1);
        assert_eq!(authorized[0].server_id, "governance-01");
    }

    #[test]
    fn test_get_server_statistics() {
        let registry = create_test_registry();
        let stats = get_server_statistics(&registry);
        assert_eq!(stats.total_servers, 2);
        assert_eq!(stats.active_servers, 1);
        assert_eq!(stats.compromised_servers, 1);
    }

    #[test]
    fn test_validate_server_config() {
        let server = AuthorizedServer {
            server_id: "test".to_string(),
            operator: crate::authorization::server::OperatorInfo {
                name: "Test".to_string(),
                jurisdiction: "Test".to_string(),
                contact: None,
            },
            keys: crate::authorization::server::ServerKeys {
                nostr_npub: "npub1test".to_string(),
                ssh_fingerprint: "SHA256:test".to_string(),
            },
            infrastructure: crate::authorization::server::InfrastructureInfo {
                vpn_ip: None,
                github_runner: false,
                ots_enabled: false,
            },
            status: ServerStatus::Active,
            added_at: chrono::Utc::now(),
            last_verified: None,
        };

        assert!(validate_server_config(&server).is_ok());
    }
}
