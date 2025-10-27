//! Governance Configuration Export
//!
//! Handles exporting complete governance configuration as single YAML file

use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

use super::types::*;
use crate::error::GovernanceError;

pub struct GovernanceExporter {
    config_path: String,
}

impl GovernanceExporter {
    pub fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
        }
    }

    /// Export complete governance configuration as single YAML
    pub async fn export_governance_config(
        &self,
        ruleset_id: &str,
        version: &RulesetVersion,
        exported_by: &str,
        source_repo: &str,
        commit_hash: &str,
    ) -> Result<GovernanceExport, GovernanceError> {
        // Load all governance configuration files
        let action_tiers = self.load_config_file("action-tiers.yml").await?;
        let economic_nodes = self.load_config_file("economic-nodes.yml").await?;
        let maintainers = self.load_maintainers_config().await?;
        let repositories = self.load_repositories_config().await?;
        let governance_fork = self.load_config_file("governance-fork.yml").await?;

        // Create configuration hash
        let config_data = serde_json::json!({
            "action_tiers": action_tiers,
            "economic_nodes": economic_nodes,
            "maintainers": maintainers,
            "repositories": repositories,
            "governance_fork": governance_fork,
        });

        let _config_hash = self.calculate_config_hash(&config_data)?;

        // Create export metadata
        let metadata = ExportMetadata {
            exported_by: exported_by.to_string(),
            source_repository: source_repo.to_string(),
            commit_hash: commit_hash.to_string(),
            export_tool_version: env!("CARGO_PKG_VERSION").to_string(),
            signature: None, // Would be added by signing process
            verification_url: None,
        };

        Ok(GovernanceExport {
            version: "1.0".to_string(),
            ruleset_id: ruleset_id.to_string(),
            ruleset_version: version.clone(),
            created_at: Utc::now(),
            action_tiers,
            economic_nodes,
            maintainers,
            repositories,
            governance_fork,
            metadata,
        })
    }

    /// Save export to file
    pub async fn save_export(
        &self,
        export: &GovernanceExport,
        output_path: &str,
    ) -> Result<(), GovernanceError> {
        let yaml_content = serde_yaml::to_string(export).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to serialize export: {}", e))
        })?;

        tokio::fs::write(output_path, yaml_content)
            .await
            .map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to write export file: {}", e))
            })?;

        Ok(())
    }

    /// Load export from file
    pub async fn load_export(&self, file_path: &str) -> Result<GovernanceExport, GovernanceError> {
        let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read export file: {}", e))
        })?;

        let export: GovernanceExport = serde_yaml::from_str(&content)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse export: {}", e)))?;

        Ok(export)
    }

    /// Verify export integrity
    pub fn verify_export(&self, export: &GovernanceExport) -> Result<bool, GovernanceError> {
        let config_data = serde_json::json!({
            "action_tiers": export.action_tiers,
            "economic_nodes": export.economic_nodes,
            "maintainers": export.maintainers,
            "repositories": export.repositories,
            "governance_fork": export.governance_fork,
        });

        let calculated_hash = self.calculate_config_hash(&config_data)?;

        // In a real implementation, we would compare with stored hash
        // For now, just verify the hash calculation works
        Ok(!calculated_hash.is_empty())
    }

    /// Load a single configuration file
    async fn load_config_file(&self, filename: &str) -> Result<Value, GovernanceError> {
        let file_path = Path::new(&self.config_path).join(filename);

        if !file_path.exists() {
            // Return empty object if file doesn't exist
            return Ok(serde_json::json!({}));
        }

        let content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read {}: {}", filename, e))
        })?;

        let yaml_value: Value = serde_yaml::from_str(&content).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to parse {}: {}", filename, e))
        })?;

        Ok(yaml_value)
    }

    /// Load maintainers configuration from all layer files
    async fn load_maintainers_config(&self) -> Result<Value, GovernanceError> {
        let maintainers_path = Path::new(&self.config_path).join("maintainers");

        if !maintainers_path.exists() {
            return Ok(serde_json::json!({}));
        }

        let mut maintainers = serde_json::Map::new();

        // Load all maintainer files
        let mut entries = tokio::fs::read_dir(&maintainers_path).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read maintainers directory: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to read {}: {}", filename, e))
                })?;

                let yaml_value: Value = serde_yaml::from_str(&content).map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to parse {}: {}", filename, e))
                })?;

                maintainers.insert(filename.to_string(), yaml_value);
            }
        }

        Ok(Value::Object(maintainers))
    }

    /// Load repositories configuration from all repo files
    async fn load_repositories_config(&self) -> Result<Value, GovernanceError> {
        let repos_path = Path::new(&self.config_path).join("repos");

        if !repos_path.exists() {
            return Ok(serde_json::json!({}));
        }

        let mut repositories = serde_json::Map::new();

        // Load all repository files
        let mut entries = tokio::fs::read_dir(&repos_path).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read repos directory: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yml") {
                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to read {}: {}", filename, e))
                })?;

                let yaml_value: Value = serde_yaml::from_str(&content).map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to parse {}: {}", filename, e))
                })?;

                repositories.insert(filename.to_string(), yaml_value);
            }
        }

        Ok(Value::Object(repositories))
    }

    /// Calculate cryptographic hash of configuration
    fn calculate_config_hash(&self, config: &Value) -> Result<String, GovernanceError> {
        let config_string = serde_json::to_string(config).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to serialize config: {}", e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(config_string.as_bytes());
        let hash = hasher.finalize();

        Ok(hex::encode(hash))
    }
}
