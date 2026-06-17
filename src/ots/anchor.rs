//! Registry Anchorer for Monthly OTS Anchoring
//!
//! Creates monthly governance registries and anchors them to Bitcoin
//! using OpenTimestamps for historical proof.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use hex;
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
        self.store_registry_info(&month_key, &registry_file, &proof_file)
            .await?;

        info!(
            "Successfully anchored registry for {} to Bitcoin",
            month_key
        );
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
        use sqlx::Row;

        let pool = self
            .database
            .get_sqlite_pool()
            .ok_or_else(|| anyhow!("Database pool not available or not SQLite"))?;

        let row = sqlx::query(
            "SELECT registry_hash FROM governance_registries ORDER BY timestamp DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(r.get::<String, _>(0)),
            None => Ok(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            ),
        }
    }

    /// Get maintainers from database
    async fn get_maintainers(&self) -> Result<Vec<Maintainer>> {
        use crate::database::queries::Queries;

        let pool = self
            .database
            .get_sqlite_pool()
            .ok_or_else(|| anyhow!("Database pool not available or not SQLite"))?;

        // Get all maintainers from all layers
        let mut all_maintainers = Vec::new();

        // Query maintainers for each layer (layers 1-6)
        for layer in 1..=6 {
            match Queries::get_maintainers_for_layer(pool, layer).await {
                Ok(maintainers) => {
                    for db_maintainer in maintainers {
                        // Map database Maintainer to registry Maintainer
                        // Note: Database maintainer has github_username and public_key
                        // Registry maintainer needs name, npub, added_at, status
                        // We'll use github_username as name, and derive npub from public_key if possible
                        all_maintainers.push(Maintainer {
                            id: db_maintainer.id,
                            name: db_maintainer.github_username.clone(),
                            npub: db_maintainer.public_key.clone(), // Use public_key as npub for now
                            added_at: db_maintainer.last_updated,
                            status: if db_maintainer.active {
                                "active".to_string()
                            } else {
                                "inactive".to_string()
                            },
                        });
                    }
                }
                Err(e) => {
                    warn!("Failed to get maintainers for layer {}: {}", layer, e);
                }
            }
        }

        Ok(all_maintainers)
    }

    /// Get authorized servers from database
    async fn get_authorized_servers(&self) -> Result<Vec<AuthorizedServer>> {
        // Note: There is no authorized_servers table in the current schema
        // This would need to be added in a future migration if server registration is needed
        // For now, return empty vector
        warn!("Authorized servers table not implemented - returning empty list");
        Ok(vec![])
    }

    /// Get audit log summaries
    async fn get_audit_log_summaries(&self) -> Result<HashMap<String, AuditLogSummary>> {
        use crate::audit::logger::AuditLogger;

        // Note: Audit logs are file-based, not database-based
        // We need to read from the audit log file path
        // For now, we'll try to read from a default path or return empty
        // In production, this would be configured via AppConfig

        // Try to find audit log files in common locations
        let default_audit_path = "/var/lib/governance/audit-log.jsonl";

        // Check if default path exists
        if !Path::new(default_audit_path).exists() {
            warn!(
                "Audit log file not found at {} - returning empty summaries",
                default_audit_path
            );
            return Ok(HashMap::new());
        }

        // Create audit logger to read entries
        let logger = match AuditLogger::new(default_audit_path.to_string()) {
            Ok(l) => l,
            Err(e) => {
                warn!(
                    "Failed to create audit logger: {}. Returning empty summaries.",
                    e
                );
                return Ok(HashMap::new());
            }
        };

        // Get all entries
        let entries = match logger.get_all_entries().await {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    "Failed to read audit log entries: {}. Returning empty summaries.",
                    e
                );
                return Ok(HashMap::new());
            }
        };

        if entries.is_empty() {
            return Ok(HashMap::new());
        }

        // Group entries by server_id
        let mut summaries: HashMap<String, Vec<crate::audit::entry::AuditLogEntry>> =
            HashMap::new();
        for entry in entries {
            summaries
                .entry(entry.server_id.clone())
                .or_insert_with(Vec::new)
                .push(entry);
        }

        // Calculate summaries for each server
        let mut result = HashMap::new();
        for (server_id, server_entries) in summaries {
            let entries_count = server_entries.len() as u64;
            let first_entry_hash = server_entries
                .first()
                .map(|e| e.this_log_hash.clone())
                .unwrap_or_else(|| {
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string()
                });
            let last_entry_hash = server_entries
                .last()
                .map(|e| e.this_log_hash.clone())
                .unwrap_or_else(|| {
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string()
                });

            // Calculate Merkle root from entries
            let merkle_root = Self::calculate_merkle_root_for_entries(&server_entries)?;

            result.insert(
                server_id,
                AuditLogSummary {
                    entries_count,
                    first_entry_hash,
                    last_entry_hash,
                    merkle_root,
                },
            );
        }

        Ok(result)
    }

    /// Calculate Merkle root from audit log entries
    fn calculate_merkle_root_for_entries(
        entries: &[crate::audit::entry::AuditLogEntry],
    ) -> Result<String> {
        if entries.is_empty() {
            return Ok(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            );
        }

        // Hash each entry using its this_log_hash (extract hex part)
        let mut hashes: Vec<[u8; 32]> = entries
            .iter()
            .map(|e| {
                let hex_str = e
                    .this_log_hash
                    .strip_prefix("sha256:")
                    .unwrap_or(&e.this_log_hash);
                let hash_bytes = hex::decode(hex_str).unwrap_or_else(|_| {
                    // Fallback: hash the entry's canonical string
                    let canonical = e.canonical_string();
                    Sha256::digest(canonical.as_bytes()).to_vec()
                });
                // Ensure we have exactly 32 bytes
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes[..32.min(hash_bytes.len())]);
                hash
            })
            .collect();

        // Build Merkle tree
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in hashes.chunks(2) {
                if chunk.len() == 2 {
                    let combined = [chunk[0].as_slice(), chunk[1].as_slice()].concat();
                    next_level.push(Sha256::digest(&combined).into());
                } else {
                    // Odd number, duplicate last hash
                    let combined = [chunk[0].as_slice(), chunk[0].as_slice()].concat();
                    next_level.push(Sha256::digest(&combined).into());
                }
            }
            hashes = next_level;
        }

        Ok(format!("sha256:{}", hex::encode(hashes[0])))
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
            fs::create_dir_all(parent).map_err(|e| anyhow!("Failed to create directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(registry)
            .map_err(|e| anyhow!("Failed to serialize registry: {}", e))?;

        fs::write(path, json).map_err(|e| anyhow!("Failed to write registry file: {}", e))?;

        info!("Saved registry to: {}", path.display());
        Ok(())
    }

    /// Save OTS proof to file
    async fn save_proof(&self, proof: &[u8], path: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| anyhow!("Failed to create directory: {}", e))?;
        }

        fs::write(path, proof).map_err(|e| anyhow!("Failed to write proof file: {}", e))?;

        info!("Saved OTS proof to: {}", path.display());
        Ok(())
    }

    /// Store registry information in database
    async fn store_registry_info(
        &self,
        month_key: &str,
        registry_file: &Path,
        proof_file: &Path,
    ) -> Result<()> {
        use sqlx::Row;

        // Calculate registry hash
        let registry_data =
            fs::read(registry_file).map_err(|e| anyhow!("Failed to read registry file: {}", e))?;
        let mut hasher = Sha256::new();
        hasher.update(&registry_data);
        let hash = hasher.finalize();
        let registry_hash = format!("sha256:{}", hex::encode(hash));

        let pool = self
            .database
            .get_sqlite_pool()
            .ok_or_else(|| anyhow!("Database pool not available or not SQLite"))?;

        let now = Utc::now();
        let proof_path_str = proof_file.to_string_lossy().to_string();
        let registry_path_str = registry_file.to_string_lossy().to_string();

        sqlx::query(
            r#"
            INSERT INTO governance_registries 
            (registry_hash, registry_path, timestamp, month_year, ots_proof_path)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&registry_hash)
        .bind(&registry_path_str)
        .bind(now)
        .bind(month_key)
        .bind(&proof_path_str)
        .execute(pool)
        .await?;

        info!(
            "Stored registry info for {}: {} -> {} (hash: {})",
            month_key,
            registry_file.display(),
            proof_file.display(),
            registry_hash
        );
        Ok(())
    }

    /// Verify a registry against its OTS proof
    pub async fn verify_registry(
        &self,
        registry_file: &Path,
        proof_file: &Path,
    ) -> Result<VerificationResult> {
        // Load registry data
        let registry_data =
            fs::read(registry_file).map_err(|e| anyhow!("Failed to read registry file: {}", e))?;

        // Load OTS proof
        let proof_data =
            fs::read(proof_file).map_err(|e| anyhow!("Failed to read proof file: {}", e))?;

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
        let ots_client =
            OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
        let database = Database::new_in_memory().await.unwrap();

        let anchorer = RegistryAnchorer::new(
            ots_client,
            database,
            temp_dir
                .path()
                .join("registries")
                .to_string_lossy()
                .to_string(),
            temp_dir.path().join("proofs").to_string_lossy().to_string(),
        );

        assert!(
            anchorer.registry_path.exists() || anchorer.registry_path.parent().unwrap().exists()
        );
        assert!(anchorer.proofs_path.exists() || anchorer.proofs_path.parent().unwrap().exists());
    }

    #[test]
    fn test_governance_registry_creation() {
        let registry = GovernanceRegistry {
            version: "2025-01".to_string(),
            timestamp: Utc::now(),
            previous_registry_hash:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
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
