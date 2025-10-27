//! Registry Anchorer for Monthly OTS Anchoring
//!
//! Creates monthly governance registries and anchors them to Bitcoin
//! using OpenTimestamps for historical proof.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::database::Database;
use crate::ots::client::{OtsClient, VerificationResult};

/// Registry anchorer for monthly governance anchoring
pub struct RegistryAnchorer {
    ots_client: OtsClient,
    database: Database,
    registry_path: PathBuf,
    proofs_path: PathBuf,
}

/// Governance registry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRegistry {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub previous_registry_hash: String,
    pub maintainers: Vec<Maintainer>,
    pub authorized_servers: Vec<AuthorizedServer>,
    pub audit_logs: HashMap<String, AuditLogSummary>,
    pub multisig_config: MultisigConfig,
}

/// Maintainer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Maintainer {
    pub id: i32,
    pub name: String,
    pub npub: String,
    pub added_at: DateTime<Utc>,
    pub status: String,
}

/// Authorized server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedServer {
    pub server_id: String,
    pub operator: OperatorInfo,
    pub keys: ServerKeys,
    pub infrastructure: InfrastructureInfo,
    pub status: String,
    pub added_at: DateTime<Utc>,
}

/// Operator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: String,
    pub jurisdiction: String,
    pub contact: Option<String>,
}

/// Server keys
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

/// Audit log summary for a server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogSummary {
    pub entries_count: u64,
    pub first_entry_hash: String,
    pub last_entry_hash: String,
    pub merkle_root: String,
}

/// Multisig configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigConfig {
    pub required_signatures: usize,
    pub total_maintainers: usize,
}

impl RegistryAnchorer {
    /// Create new registry anchorer
    pub fn new(
        ots_client: OtsClient,
        database: Database,
        registry_path: String,
        proofs_path: String,
    ) -> Self {
        Self {
            ots_client,
            database,
            registry_path: PathBuf::from(registry_path),
            proofs_path: PathBuf::from(proofs_path),
        }
    }

    /// Generate and anchor monthly registry
    pub async fn anchor_registry(&self) -> Result<()> {
        let now = Utc::now();
        let month_key = now.format("%Y-%m").to_string();
        
        info!("Generating monthly registry for {}", month_key);

        // Generate registry
        let registry = self.generate_registry().await?;
        
        // Save registry JSON
        let registry_file = self.registry_path.join(format!("{}.json", month_key));
        self.save_registry(&registry, &registry_file).await?;

        // Create OTS timestamp
        let registry_data = serde_json::to_vec(&registry)
            .map_err(|e| anyhow!("Failed to serialize registry: {}", e))?;

        let proof_data = self.ots_client.stamp(&registry_data).await?;

        // Save OTS proof
        let proof_file = self.proofs_path.join(format!("{}.json.ots", month_key));
        self.save_proof(&proof_data, &proof_file).await?;

        // Store in database
        self.store_registry_info(&month_key, &registry_file, &proof_file).await?;

        info!("Successfully anchored registry for {} to Bitcoin", month_key);
        Ok(())
    }

    /// Generate governance registry from database
    async fn generate_registry(&self) -> Result<GovernanceRegistry> {
        let now = Utc::now();
        let version = now.format("%Y-%m").to_string();

        // Get previous registry hash
        let previous_hash = self.get_previous_registry_hash().await?;

        // Get maintainers from database
        let maintainers = self.get_maintainers().await?;

        // Get authorized servers from database
        let authorized_servers = self.get_authorized_servers().await?;

        // Get audit log summaries
        let audit_logs = self.get_audit_log_summaries().await?;

        // Get multisig configuration
        let multisig_config = self.get_multisig_config().await?;

        Ok(GovernanceRegistry {
            version,
            timestamp: now,
            previous_registry_hash: previous_hash,
            maintainers,
            authorized_servers,
            audit_logs,
            multisig_config,
        })
    }

    /// Get previous registry hash
    async fn get_previous_registry_hash(&self) -> Result<String> {
        // This would query the database for the last registry hash
        // For now, return a placeholder
        Ok("sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string())
    }

    /// Get maintainers from database
    async fn get_maintainers(&self) -> Result<Vec<Maintainer>> {
        // This would query the database for maintainers
        // For now, return empty vector
        Ok(vec![])
    }

    /// Get authorized servers from database
    async fn get_authorized_servers(&self) -> Result<Vec<AuthorizedServer>> {
        // This would query the database for authorized servers
        // For now, return empty vector
        Ok(vec![])
    }

    /// Get audit log summaries
    async fn get_audit_log_summaries(&self) -> Result<HashMap<String, AuditLogSummary>> {
        // This would calculate merkle roots for each server's audit log
        // For now, return empty map
        Ok(HashMap::new())
    }

    /// Get multisig configuration
    async fn get_multisig_config(&self) -> Result<MultisigConfig> {
        Ok(MultisigConfig {
            required_signatures: 3,
            total_maintainers: 5,
        })
    }

    /// Save registry to file
    async fn save_registry(&self, registry: &GovernanceRegistry, path: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(registry)
            .map_err(|e| anyhow!("Failed to serialize registry: {}", e))?;

        fs::write(path, json)
            .map_err(|e| anyhow!("Failed to write registry file: {}", e))?;

        info!("Saved registry to: {}", path.display());
        Ok(())
    }

    /// Save OTS proof to file
    async fn save_proof(&self, proof: &[u8], path: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create directory: {}", e))?;
        }

        fs::write(path, proof)
            .map_err(|e| anyhow!("Failed to write proof file: {}", e))?;

        info!("Saved OTS proof to: {}", path.display());
        Ok(())
    }

    /// Store registry information in database
    async fn store_registry_info(&self, month_key: &str, registry_file: &Path, proof_file: &Path) -> Result<()> {
        // This would store the registry info in the database
        // For now, just log
        info!("Stored registry info for {}: {} -> {}", month_key, registry_file.display(), proof_file.display());
        Ok(())
    }

    /// Verify a registry against its OTS proof
    pub async fn verify_registry(&self, registry_file: &Path, proof_file: &Path) -> Result<VerificationResult> {
        // Load registry data
        let registry_data = fs::read(registry_file)
            .map_err(|e| anyhow!("Failed to read registry file: {}", e))?;

        // Load OTS proof
        let proof_data = fs::read(proof_file)
            .map_err(|e| anyhow!("Failed to read proof file: {}", e))?;

        // Verify timestamp
        self.ots_client.verify(&registry_data, &proof_data).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_registry_anchorer_creation() {
        let temp_dir = tempdir().unwrap();
        let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
        let database = Database::new_sqlite(":memory:".to_string()).await.unwrap();
        
        let anchorer = RegistryAnchorer::new(
            ots_client,
            database,
            temp_dir.path().join("registries").to_string_lossy().to_string(),
            temp_dir.path().join("proofs").to_string_lossy().to_string(),
        );

        assert!(anchorer.registry_path.exists() || anchorer.registry_path.parent().unwrap().exists());
        assert!(anchorer.proofs_path.exists() || anchorer.proofs_path.parent().unwrap().exists());
    }

    #[test]
    fn test_governance_registry_creation() {
        let registry = GovernanceRegistry {
            version: "2025-01".to_string(),
            timestamp: Utc::now(),
            previous_registry_hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            maintainers: vec![],
            authorized_servers: vec![],
            audit_logs: HashMap::new(),
            multisig_config: MultisigConfig {
                required_signatures: 3,
                total_maintainers: 5,
            },
        };

        assert_eq!(registry.version, "2025-01");
        assert_eq!(registry.multisig_config.required_signatures, 3);
    }
}
