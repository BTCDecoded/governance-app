//! Governance Fork Executor
//! 
//! Handles the execution of governance forks, including detection, migration,
//! and coordination between different governance rulesets.

use std::collections::HashMap;
use std::path::Path;
use std::fs;
use chrono::{DateTime, Utc};
use serde_json;
use tracing::{info, warn, error};

use crate::error::GovernanceError;
use super::types::*;
use super::export::GovernanceExporter;
use super::adoption::AdoptionTracker;
use super::versioning::RulesetVersioning;

/// Executes governance forks and manages ruleset transitions
pub struct ForkExecutor {
    current_ruleset: Option<Ruleset>,
    available_rulesets: HashMap<String, Ruleset>,
    adoption_tracker: AdoptionTracker,
    exporter: GovernanceExporter,
    versioning: RulesetVersioning,
    fork_thresholds: ForkThresholds,
}

impl ForkExecutor {
    /// Create a new fork executor
    pub fn new(
        export_path: &str,
        fork_thresholds: Option<ForkThresholds>,
    ) -> Result<Self, GovernanceError> {
        let exporter = GovernanceExporter::new(export_path)?;
        let adoption_tracker = AdoptionTracker::new()?;
        let versioning = RulesetVersioning::new()?;
        
        Ok(Self {
            current_ruleset: None,
            available_rulesets: HashMap::new(),
            adoption_tracker,
            exporter,
            versioning,
            fork_thresholds: fork_thresholds.unwrap_or_default(),
        })
    }

    /// Initialize the fork executor with current governance state
    pub async fn initialize(&mut self, governance_config_path: &str) -> Result<(), GovernanceError> {
        info!("Initializing governance fork executor...");
        
        // Load current governance configuration
        let current_config = self.load_governance_config(governance_config_path).await?;
        
        // Create current ruleset
        let current_ruleset = self.create_ruleset_from_config(&current_config, "current")?;
        self.current_ruleset = Some(current_ruleset.clone());
        self.available_rulesets.insert("current".to_string(), current_ruleset);
        
        // Load available rulesets from export directory
        self.load_available_rulesets().await?;
        
        // Check for fork conditions
        self.check_fork_conditions().await?;
        
        info!("Fork executor initialized successfully");
        Ok(())
    }

    /// Load governance configuration from files
    async fn load_governance_config(&self, config_path: &str) -> Result<serde_json::Value, GovernanceError> {
        let path = Path::new(config_path);
        
        if !path.exists() {
            return Err(GovernanceError::ConfigError(
                format!("Governance config path does not exist: {}", config_path)
            ));
        }
        
        // Load all governance configuration files
        let mut config = serde_json::Map::new();
        
        // Load action tiers
        let action_tiers_path = path.join("action-tiers.yml");
        if action_tiers_path.exists() {
            let content = fs::read_to_string(&action_tiers_path)?;
            let action_tiers: serde_json::Value = serde_yaml::from_str(&content)
                .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse action-tiers.yml: {}", e)))?;
            config.insert("action_tiers".to_string(), action_tiers);
        }
        
        // Load economic nodes
        let economic_nodes_path = path.join("economic-nodes.yml");
        if economic_nodes_path.exists() {
            let content = fs::read_to_string(&economic_nodes_path)?;
            let economic_nodes: serde_json::Value = serde_yaml::from_str(&content)
                .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse economic-nodes.yml: {}", e)))?;
            config.insert("economic_nodes".to_string(), economic_nodes);
        }
        
        // Load maintainers
        let maintainers_path = path.join("maintainers");
        if maintainers_path.exists() {
            let mut maintainers = serde_json::Map::new();
            for entry in fs::read_dir(&maintainers_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                    let content = fs::read_to_string(&path)?;
                    let maintainer_config: serde_json::Value = serde_yaml::from_str(&content)
                        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse {}: {}", path.display(), e)))?;
                    maintainers.insert(
                        path.file_stem().unwrap().to_string_lossy().to_string(),
                        maintainer_config
                    );
                }
            }
            config.insert("maintainers".to_string(), serde_json::Value::Object(maintainers));
        }
        
        // Load repositories
        let repos_path = path.join("repos");
        if repos_path.exists() {
            let mut repositories = serde_json::Map::new();
            for entry in fs::read_dir(&repos_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("yml") {
                    let content = fs::read_to_string(&path)?;
                    let repo_config: serde_json::Value = serde_yaml::from_str(&content)
                        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse {}: {}", path.display(), e)))?;
                    repositories.insert(
                        path.file_stem().unwrap().to_string_lossy().to_string(),
                        repo_config
                    );
                }
            }
            config.insert("repositories".to_string(), serde_json::Value::Object(repositories));
        }
        
        Ok(serde_json::Value::Object(config))
    }

    /// Create a ruleset from governance configuration
    fn create_ruleset_from_config(&self, config: &serde_json::Value, ruleset_id: &str) -> Result<Ruleset, GovernanceError> {
        let version = RulesetVersion::new(1, 0, 0);
        let hash = self.calculate_config_hash(config)?;
        
        Ok(Ruleset {
            id: ruleset_id.to_string(),
            name: format!("Governance Ruleset {}", ruleset_id),
            version,
            hash,
            created_at: Utc::now(),
            config: config.clone(),
            description: Some("Current governance configuration".to_string()),
        })
    }

    /// Calculate hash of governance configuration
    fn calculate_config_hash(&self, config: &serde_json::Value) -> Result<String, GovernanceError> {
        use sha2::{Digest, Sha256};
        
        let config_str = serde_json::to_string(config)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        
        let mut hasher = Sha256::new();
        hasher.update(config_str.as_bytes());
        let hash = hasher.finalize();
        
        Ok(hex::encode(hash))
    }

    /// Load available rulesets from export directory
    async fn load_available_rulesets(&mut self) -> Result<(), GovernanceError> {
        info!("Loading available rulesets...");
        
        let export_dir = self.exporter.get_export_directory();
        if !export_dir.exists() {
            info!("No export directory found, creating empty ruleset registry");
            return Ok(());
        }
        
        for entry in fs::read_dir(&export_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(export) = serde_json::from_str::<GovernanceExport>(&content) {
                        let ruleset = Ruleset {
                            id: export.ruleset_id.clone(),
                            name: format!("Ruleset {}", export.ruleset_id),
                            version: export.ruleset_version,
                            hash: self.calculate_config_hash(&export.config)?,
                            created_at: export.created_at,
                            config: export.config,
                            description: Some(format!("Exported ruleset from {}", export.metadata.source_repository)),
                        };
                        
                        self.available_rulesets.insert(export.ruleset_id, ruleset);
                        info!("Loaded ruleset: {}", export.ruleset_id);
                    }
                }
            }
        }
        
        info!("Loaded {} available rulesets", self.available_rulesets.len());
        Ok(())
    }

    /// Check for fork conditions and execute if necessary
    async fn check_fork_conditions(&mut self) -> Result<(), GovernanceError> {
        info!("Checking for governance fork conditions...");
        
        // Get adoption statistics
        let adoption_stats = self.adoption_tracker.get_adoption_statistics().await?;
        
        // Check if any ruleset meets fork thresholds
        for (ruleset_id, metrics) in &adoption_stats.rulesets {
            if self.should_execute_fork(metrics) {
                info!("Fork conditions met for ruleset: {}", ruleset_id);
                self.execute_fork(ruleset_id).await?;
                break; // Only execute one fork at a time
            }
        }
        
        Ok(())
    }

    /// Determine if a fork should be executed based on thresholds
    fn should_execute_fork(&self, metrics: &AdoptionMetrics) -> bool {
        metrics.node_count >= self.fork_thresholds.minimum_node_count &&
        metrics.hashpower_percentage >= self.fork_thresholds.minimum_hashpower_percentage &&
        metrics.economic_activity_percentage >= self.fork_thresholds.minimum_economic_activity_percentage &&
        metrics.total_weight >= self.fork_thresholds.minimum_adoption_percentage
    }

    /// Execute a governance fork
    async fn execute_fork(&mut self, target_ruleset_id: &str) -> Result<(), GovernanceError> {
        info!("Executing governance fork to ruleset: {}", target_ruleset_id);
        
        // Get target ruleset
        let target_ruleset = self.available_rulesets.get(target_ruleset_id)
            .ok_or_else(|| GovernanceError::ConfigError(
                format!("Target ruleset not found: {}", target_ruleset_id)
            ))?;
        
        // Validate target ruleset
        self.validate_ruleset(target_ruleset)?;
        
        // Create fork event
        let fork_event = ForkEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: ForkEventType::GovernanceFork,
            ruleset_id: target_ruleset_id.to_string(),
            node_id: "governance-app".to_string(),
            details: serde_json::json!({
                "from_ruleset": self.current_ruleset.as_ref().map(|r| &r.id),
                "to_ruleset": target_ruleset_id,
                "reason": "Adoption threshold met"
            }),
            timestamp: Utc::now(),
        };
        
        // Log fork event
        self.log_fork_event(&fork_event).await?;
        
        // Execute the fork
        self.perform_fork_transition(target_ruleset).await?;
        
        info!("Governance fork executed successfully to: {}", target_ruleset_id);
        Ok(())
    }

    /// Validate a ruleset before fork execution
    fn validate_ruleset(&self, ruleset: &Ruleset) -> Result<(), GovernanceError> {
        // Check if ruleset has required components
        if !ruleset.config.get("action_tiers").is_some() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing action_tiers configuration".to_string()
            ));
        }
        
        if !ruleset.config.get("economic_nodes").is_some() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing economic_nodes configuration".to_string()
            ));
        }
        
        if !ruleset.config.get("maintainers").is_some() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing maintainers configuration".to_string()
            ));
        }
        
        // Validate ruleset version compatibility
        if let Some(current) = &self.current_ruleset {
            if !self.versioning.is_compatible(&current.version, &ruleset.version) {
                return Err(GovernanceError::ConfigError(
                    format!("Incompatible ruleset version: {} -> {}", 
                        current.version.to_string(), 
                        ruleset.version.to_string()
                    )
                ));
            }
        }
        
        Ok(())
    }

    /// Perform the actual fork transition
    async fn perform_fork_transition(&mut self, target_ruleset: &Ruleset) -> Result<(), GovernanceError> {
        info!("Performing fork transition to: {}", target_ruleset.id);
        
        // Update current ruleset
        self.current_ruleset = Some(target_ruleset.clone());
        
        // Update available rulesets
        self.available_rulesets.insert("current".to_string(), target_ruleset.clone());
        
        // Notify adoption tracker
        self.adoption_tracker.record_fork_decision(
            "governance-app".to_string(),
            target_ruleset.id.clone(),
            "Fork executed by governance-app".to_string(),
            1.0,
        ).await?;
        
        // Export new current ruleset
        self.exporter.export_ruleset(target_ruleset).await?;
        
        info!("Fork transition completed successfully");
        Ok(())
    }

    /// Log a fork event
    async fn log_fork_event(&self, event: &ForkEvent) -> Result<(), GovernanceError> {
        let log_entry = serde_json::to_string_pretty(event)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to serialize fork event: {}", e)))?;
        
        let log_path = "logs/fork-events.jsonl";
        fs::create_dir_all("logs")?;
        
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        
        use std::io::Write;
        writeln!(file, "{}", log_entry)?;
        
        info!("Fork event logged: {}", event.event_id);
        Ok(())
    }

    /// Get current ruleset
    pub fn get_current_ruleset(&self) -> Option<&Ruleset> {
        self.current_ruleset.as_ref()
    }

    /// Get available rulesets
    pub fn get_available_rulesets(&self) -> &HashMap<String, Ruleset> {
        &self.available_rulesets
    }

    /// Get adoption statistics
    pub async fn get_adoption_statistics(&self) -> Result<AdoptionStatistics, GovernanceError> {
        self.adoption_tracker.get_adoption_statistics().await
    }

    /// Export current ruleset
    pub async fn export_current_ruleset(&self) -> Result<(), GovernanceError> {
        if let Some(current) = &self.current_ruleset {
            self.exporter.export_ruleset(current).await?;
            info!("Current ruleset exported successfully");
        } else {
            warn!("No current ruleset to export");
        }
        Ok(())
    }

    /// Check if a fork is in progress
    pub fn is_fork_in_progress(&self) -> bool {
        // This would check for ongoing fork processes
        // For now, always return false
        false
    }

    /// Get fork status
    pub fn get_fork_status(&self) -> ForkStatus {
        ForkStatus {
            current_ruleset: self.current_ruleset.as_ref().map(|r| r.id.clone()),
            available_rulesets: self.available_rulesets.keys().cloned().collect(),
            fork_in_progress: self.is_fork_in_progress(),
            last_check: Utc::now(),
        }
    }
}

/// Fork status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkStatus {
    pub current_ruleset: Option<String>,
    pub available_rulesets: Vec<String>,
    pub fork_in_progress: bool,
    pub last_check: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_fork_executor_initialization() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");
        
        let executor = ForkExecutor::new(export_path.to_str().unwrap(), None);
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_ruleset_validation() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");
        
        let mut executor = ForkExecutor::new(export_path.to_str().unwrap(), None).unwrap();
        
        // Create a valid ruleset
        let config = serde_json::json!({
            "action_tiers": {},
            "economic_nodes": {},
            "maintainers": {},
            "repositories": {}
        });
        
        let ruleset = executor.create_ruleset_from_config(&config, "test").unwrap();
        assert_eq!(ruleset.id, "test");
        assert_eq!(ruleset.version.major, 1);
    }
}
