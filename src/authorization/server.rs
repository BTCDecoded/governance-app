//! Authorized Server Management
//!
//! Defines structures and operations for managing authorized governance servers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authorized server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedServer {
    pub server_id: String,
    pub operator: OperatorInfo,
    pub keys: ServerKeys,
    pub infrastructure: InfrastructureInfo,
    pub status: ServerStatus,
    pub added_at: DateTime<Utc>,
    pub last_verified: Option<DateTime<Utc>>,
}

/// Operator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub jurisdiction: String,
    pub contact: Option<String>,
}

/// Server cryptographic keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeys {
    pub nostr_npub: String,
    pub ssh_fingerprint: String,
}

/// Infrastructure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureInfo {
    pub vpn_ip: Option<String>,
    pub github_runner: bool,
    pub ots_enabled: bool,
}

/// Server status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerStatus {
    Active,
    Retiring,
    Inactive,
    Compromised,
}

impl ServerStatus {
    /// Check if server is operational
    pub fn is_operational(&self) -> bool {
        matches!(self, ServerStatus::Active)
    }

    /// Check if server is compromised
    pub fn is_compromised(&self) -> bool {
        matches!(self, ServerStatus::Compromised)
    }

    /// Get status as string
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerStatus::Active => "active",
            ServerStatus::Retiring => "retiring",
            ServerStatus::Inactive => "inactive",
            ServerStatus::Compromised => "compromised",
        }
    }
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ServerStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(ServerStatus::Active),
            "retiring" => Ok(ServerStatus::Retiring),
            "inactive" => Ok(ServerStatus::Inactive),
            "compromised" => Ok(ServerStatus::Compromised),
            _ => Err(format!("Invalid server status: {}", s)),
        }
    }
}

impl AuthorizedServer {
    /// Create new authorized server
    pub fn new(
        server_id: String,
        operator: OperatorInfo,
        keys: ServerKeys,
        infrastructure: InfrastructureInfo,
    ) -> Self {
        Self {
            server_id,
            operator,
            keys,
            infrastructure,
            status: ServerStatus::Active,
            added_at: Utc::now(),
            last_verified: None,
        }
    }

    /// Check if server is authorized and operational
    pub fn is_authorized(&self) -> bool {
        self.status.is_operational()
    }

    /// Check if server is compromised
    pub fn is_compromised(&self) -> bool {
        self.status.is_compromised()
    }

    /// Get server summary
    pub fn summary(&self) -> String {
        format!(
            "{} ({}) - {} - {}",
            self.server_id,
            self.operator.name,
            self.operator.jurisdiction,
            self.status
        )
    }

    /// Get verification info
    pub fn verification_info(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();
        info.insert("server_id".to_string(), self.server_id.clone());
        info.insert("nostr_npub".to_string(), self.keys.nostr_npub.clone());
        info.insert("ssh_fingerprint".to_string(), self.keys.ssh_fingerprint.clone());
        info.insert("status".to_string(), self.status.as_str().to_string());
        info.insert("added_at".to_string(), self.added_at.to_rfc3339());
        
        if let Some(verified) = self.last_verified {
            info.insert("last_verified".to_string(), verified.to_rfc3339());
        }
        
        info
    }
}

/// Server approval action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerAction {
    Add,
    Remove,
    Compromise,
}

impl ServerAction {
    /// Get action as string
    pub fn as_str(&self) -> &'static str {
        match self {
            ServerAction::Add => "add",
            ServerAction::Remove => "remove",
            ServerAction::Compromise => "compromise",
        }
    }
}

impl std::str::FromStr for ServerAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(ServerAction::Add),
            "remove" => Ok(ServerAction::Remove),
            "compromise" => Ok(ServerAction::Compromise),
            _ => Err(format!("Invalid server action: {}", s)),
        }
    }
}

/// Server approval record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerApproval {
    pub server_id: String,
    pub maintainer_id: i32,
    pub action: ServerAction,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

impl ServerApproval {
    /// Create new server approval
    pub fn new(
        server_id: String,
        maintainer_id: i32,
        action: ServerAction,
        signature: String,
    ) -> Self {
        Self {
            server_id,
            maintainer_id,
            action,
            signature,
            timestamp: Utc::now(),
        }
    }

    /// Get approval summary
    pub fn summary(&self) -> String {
        format!(
            "{} {} by maintainer {} at {}",
            self.action.as_str(),
            self.server_id,
            self.maintainer_id,
            self.timestamp.format("%Y-%m-%d %H:%M:%S")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authorized_server_creation() {
        let operator = OperatorInfo {
            name: "Alice".to_string(),
            jurisdiction: "United States".to_string(),
            contact: Some("alice@example.com".to_string()),
        };

        let keys = ServerKeys {
            nostr_npub: "npub1abc123".to_string(),
            ssh_fingerprint: "SHA256:xyz789".to_string(),
        };

        let infrastructure = InfrastructureInfo {
            vpn_ip: Some("10.0.0.2".to_string()),
            github_runner: true,
            ots_enabled: true,
        };

        let server = AuthorizedServer::new(
            "governance-01".to_string(),
            operator,
            keys,
            infrastructure,
        );

        assert_eq!(server.server_id, "governance-01");
        assert!(server.is_authorized());
        assert!(!server.is_compromised());
    }

    #[test]
    fn test_server_status() {
        assert!(ServerStatus::Active.is_operational());
        assert!(!ServerStatus::Active.is_compromised());
        
        assert!(!ServerStatus::Inactive.is_operational());
        assert!(!ServerStatus::Inactive.is_compromised());
        
        assert!(!ServerStatus::Compromised.is_operational());
        assert!(ServerStatus::Compromised.is_compromised());
    }

    #[test]
    fn test_server_status_parsing() {
        assert_eq!("active".parse::<ServerStatus>().unwrap(), ServerStatus::Active);
        assert_eq!("compromised".parse::<ServerStatus>().unwrap(), ServerStatus::Compromised);
        assert!("invalid".parse::<ServerStatus>().is_err());
    }

    #[test]
    fn test_server_approval() {
        let approval = ServerApproval::new(
            "governance-01".to_string(),
            1,
            ServerAction::Add,
            "sig123".to_string(),
        );

        assert_eq!(approval.server_id, "governance-01");
        assert_eq!(approval.maintainer_id, 1);
        assert_eq!(approval.action.as_str(), "add");
    }
}

// Conversion from ots::anchor::AuthorizedServer to authorization::server::AuthorizedServer
impl From<crate::ots::anchor::AuthorizedServer> for AuthorizedServer {
    fn from(ots_server: crate::ots::anchor::AuthorizedServer) -> Self {
        use std::str::FromStr;
        
        Self {
            server_id: ots_server.server_id,
            operator: OperatorInfo {
                name: ots_server.operator.name,
                jurisdiction: ots_server.operator.jurisdiction,
                contact: ots_server.operator.contact,
            },
            keys: ServerKeys {
                nostr_npub: ots_server.keys.nostr_npub,
                ssh_fingerprint: ots_server.keys.ssh_fingerprint,
            },
            infrastructure: InfrastructureInfo {
                vpn_ip: ots_server.infrastructure.vpn_ip,
                github_runner: ots_server.infrastructure.github_runner,
                ots_enabled: ots_server.infrastructure.ots_enabled,
            },
            status: ServerStatus::from_str(&ots_server.status).unwrap_or(ServerStatus::Inactive),
            added_at: ots_server.added_at,
            last_verified: None, // Not available in ots version
        }
    }
}
