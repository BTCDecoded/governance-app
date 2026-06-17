//! Governance Fork Executor
//!
//! Handles the execution of governance forks, including detection, migration,
//! and coordination between different governance rulesets.

use chrono::Utc;
use hex;
use secp256k1::SecretKey;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::info;

use super::adoption::AdoptionTracker;
use super::export::GovernanceExporter;
use super::types::*;
use super::versioning::RulesetVersioning;
use crate::error::GovernanceError;

/// Executes governance forks and manages ruleset transitions
pub struct ForkExecutor {
    current_ruleset: Option<Ruleset>,
    available_rulesets: HashMap<String, Ruleset>,
    adoption_tracker: AdoptionTracker,
    exporter: GovernanceExporter,
    versioning: RulesetVersioning,
    fork_thresholds: ForkThresholds,
    executor_secret_key: Option<SecretKey>,
}

impl ForkExecutor {
    /// Create a new fork executor
    pub fn new(
        export_path: &str,
        pool: sqlx::SqlitePool,
        fork_thresholds: Option<ForkThresholds>,
    ) -> Result<Self, GovernanceError> {
        Self::new_with_key(export_path, pool, fork_thresholds, None)
    }

    /// Create a new fork executor with optional secret key for signing fork decisions
    pub fn new_with_key(
        export_path: &str,
        pool: sqlx::SqlitePool,
        fork_thresholds: Option<ForkThresholds>,
        executor_secret_key: Option<SecretKey>,
    ) -> Result<Self, GovernanceError> {
        let exporter = GovernanceExporter::new(export_path);
        let adoption_tracker = AdoptionTracker::new(pool);
        let versioning = RulesetVersioning::new();

        Ok(Self {
            current_ruleset: None,
            available_rulesets: HashMap::new(),
            adoption_tracker,
            exporter,
            versioning,
            fork_thresholds: fork_thresholds.unwrap_or_default(),
            executor_secret_key,
        })
    }

    /// Set the executor secret key for signing fork decisions
    pub fn set_secret_key(&mut self, secret_key: SecretKey) {
        self.executor_secret_key = Some(secret_key);
    }

    /// Initialize the fork executor with current governance state
    pub async fn initialize(
        &mut self,
        governance_config_path: &str,
    ) -> Result<(), GovernanceError> {
        info!("Initializing governance fork executor...");

        // Load current governance configuration
        let current_config = self.load_governance_config(governance_config_path).await?;

        // Create current ruleset
        let current_ruleset = self.create_ruleset_from_config(&current_config, "current")?;
        self.current_ruleset = Some(current_ruleset.clone());
        self.available_rulesets
            .insert("current".to_string(), current_ruleset);

        // Load available rulesets from export directory
        self.load_available_rulesets().await?;

        // Check for fork conditions
        self.check_fork_conditions().await?;

        info!("Fork executor initialized successfully");
        Ok(())
    }

    /// Load governance configuration from files
    async fn load_governance_config(
        &self,
        config_path: &str,
    ) -> Result<serde_json::Value, GovernanceError> {
        let path = Path::new(config_path);

        if !path.exists() {
            return Err(GovernanceError::ConfigError(format!(
                "Governance config path does not exist: {config_path}"
            )));
        }

        // Load all governance configuration files
        let mut config = serde_json::Map::new();

        // Load action tiers
        let action_tiers_path = path.join("action-tiers.yml");
        if action_tiers_path.exists() {
            let content = fs::read_to_string(&action_tiers_path)?;
            let action_tiers: serde_json::Value = serde_yaml::from_str(&content).map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to parse action-tiers.yml: {e}"))
            })?;
            config.insert("action_tiers".to_string(), action_tiers);
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
                        .map_err(|e| {
                            GovernanceError::ConfigError(format!(
                                "Failed to parse {}: {}",
                                path.display(),
                                e
                            ))
                        })?;
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                        GovernanceError::ConfigError(format!(
                            "Invalid file name in maintainers directory: {}",
                            path.display()
                        ))
                    })?;
                    maintainers.insert(file_stem.to_string(), maintainer_config);
                }
            }
            config.insert(
                "maintainers".to_string(),
                serde_json::Value::Object(maintainers),
            );
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
                    let repo_config: serde_json::Value =
                        serde_yaml::from_str(&content).map_err(|e| {
                            GovernanceError::ConfigError(format!(
                                "Failed to parse {}: {}",
                                path.display(),
                                e
                            ))
                        })?;
                    let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                        GovernanceError::ConfigError(format!(
                            "Invalid file name in repos directory: {}",
                            path.display()
                        ))
                    })?;
                    repositories.insert(file_stem.to_string(), repo_config);
                }
            }
            config.insert(
                "repositories".to_string(),
                serde_json::Value::Object(repositories),
            );
        }

        Ok(serde_json::Value::Object(config))
    }

    /// Create a ruleset from governance configuration
    pub fn create_ruleset_from_config(
        &self,
        config: &serde_json::Value,
        ruleset_id: &str,
    ) -> Result<Ruleset, GovernanceError> {
        let version = RulesetVersion::new(1, 0, 0);
        let hash = self.calculate_config_hash(config)?;

        Ok(Ruleset {
            id: ruleset_id.to_string(),
            name: format!("Governance Ruleset {ruleset_id}"),
            version,
            hash,
            created_at: Utc::now(),
            config: config.clone(),
            description: Some("Current governance configuration".to_string()),
        })
    }

    /// Calculate hash of governance configuration
    pub fn calculate_config_hash(
        &self,
        config: &serde_json::Value,
    ) -> Result<String, GovernanceError> {
        use sha2::{Digest, Sha256};

        let config_str = serde_json::to_string(config).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to serialize config: {e}"))
        })?;

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
                        // Construct config from individual fields
                        let config = serde_json::json!({
                            "action_tiers": export.action_tiers,
                            "maintainers": export.maintainers,
                            "repositories": export.repositories,
                            "governance_fork": export.governance_fork,
                        });
                        let ruleset_id = export.ruleset_id.clone();
                        let ruleset = Ruleset {
                            id: ruleset_id.clone(),
                            name: format!("Ruleset {ruleset_id}"),
                            version: export.ruleset_version,
                            hash: self.calculate_config_hash(&config)?,
                            created_at: export.created_at,
                            config,
                            description: Some(format!(
                                "Exported ruleset from {}",
                                export.metadata.source_repository
                            )),
                        };

                        self.available_rulesets.insert(ruleset_id.clone(), ruleset);
                        info!("Loaded ruleset: {}", ruleset_id);
                    }
                }
            }
        }

        info!(
            "Loaded {} available rulesets",
            self.available_rulesets.len()
        );
        Ok(())
    }

    /// Check for fork conditions and execute if necessary
    async fn check_fork_conditions(&mut self) -> Result<(), GovernanceError> {
        info!("Checking for governance fork conditions...");

        // Get adoption statistics
        let adoption_stats = self.adoption_tracker.get_adoption_statistics().await?;

        // Check if any ruleset meets fork thresholds
        for metrics in &adoption_stats.rulesets {
            if self.should_execute_fork(metrics) {
                info!("Fork conditions met for ruleset: {}", metrics.ruleset_id);
                self.execute_fork(&metrics.ruleset_id).await?;
                break; // Only execute one fork at a time
            }
        }

        Ok(())
    }

    /// Determine if a fork should be executed based on thresholds
    pub fn should_execute_fork(&self, metrics: &AdoptionMetrics) -> bool {
        metrics.node_count >= self.fork_thresholds.minimum_node_count
            && metrics.hashpower_percentage >= self.fork_thresholds.minimum_hashpower_percentage
            && metrics.economic_activity_percentage
                >= self.fork_thresholds.minimum_economic_activity_percentage
            && metrics.total_weight >= self.fork_thresholds.minimum_adoption_percentage
    }

    /// Execute a governance fork
    async fn execute_fork(&mut self, target_ruleset_id: &str) -> Result<(), GovernanceError> {
        info!("Executing governance fork to: {}", target_ruleset_id);

        // Get target ruleset (clone to avoid borrow checker issues)
        let target_ruleset = self
            .available_rulesets
            .get(target_ruleset_id)
            .ok_or_else(|| {
                GovernanceError::ConfigError(format!(
                    "Target ruleset not found: {target_ruleset_id}"
                ))
            })?
            .clone();

        // Validate target ruleset
        self.validate_ruleset(&target_ruleset)?;

        // Create fork event (extract current_ruleset_id first to avoid borrow)
        let current_ruleset_id = self.current_ruleset.as_ref().map(|r| r.id.clone());
        let fork_event = ForkEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: ForkEventType::GovernanceFork,
            ruleset_id: target_ruleset_id.to_string(),
            node_id: "blvm-commons".to_string(),
            details: serde_json::json!({
                "from_ruleset": current_ruleset_id,
                "to_ruleset": target_ruleset_id,
                "reason": "Adoption threshold met"
            }),
            timestamp: Utc::now(),
        };

        // Log fork event
        self.log_fork_event(&fork_event).await?;

        // Execute the fork
        self.perform_fork_transition(&target_ruleset).await?;

        info!(
            "Governance fork executed successfully to: {}",
            target_ruleset_id
        );
        Ok(())
    }

    /// Validate a ruleset before fork execution
    pub fn validate_ruleset(&self, ruleset: &Ruleset) -> Result<(), GovernanceError> {
        // Check if ruleset has required components
        if ruleset.config.get("action_tiers").is_none() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing action_tiers".to_string(),
            ));
        }

        if ruleset.config.get("maintainers").is_none() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing maintainers".to_string(),
            ));
        }

        if ruleset.config.get("repositories").is_none() {
            return Err(GovernanceError::ConfigError(
                "Ruleset missing repositories".to_string(),
            ));
        }

        Ok(())
    }

    /// Log a fork event
    async fn log_fork_event(&self, event: &ForkEvent) -> Result<(), GovernanceError> {
        // Store fork event in database via adoption tracker
        // Extract fields from ForkEvent struct - use details field
        self.adoption_tracker
            .log_fork_event(
                event.event_type.clone(),
                &event.ruleset_id,
                &event.node_id,
                &event.details,
            )
            .await?;
        Ok(())
    }

    /// Perform the actual fork transition
    async fn perform_fork_transition(
        &mut self,
        target_ruleset: &Ruleset,
    ) -> Result<(), GovernanceError> {
        // Update current ruleset
        self.current_ruleset = Some(target_ruleset.clone());

        // Update available rulesets
        self.available_rulesets
            .insert("current".to_string(), target_ruleset.clone());

        info!("Fork transition completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn setup_test_executor() -> ForkExecutor {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        ForkExecutor::new(export_path.to_str().unwrap(), pool, None).unwrap()
    }

    #[tokio::test]
    async fn test_fork_executor_initialization() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let executor = ForkExecutor::new(export_path.to_str().unwrap(), pool, None);
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_ruleset_validation() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");

        // Create in-memory database for testing
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let executor = ForkExecutor::new(export_path.to_str().unwrap(), pool, None).unwrap();

        // Create a valid ruleset
        let config = serde_json::json!({
            "action_tiers": {},
            "maintainers": {},
            "repositories": {}
        });

        let ruleset = executor
            .create_ruleset_from_config(&config, "test")
            .unwrap();
        assert_eq!(ruleset.id, "test");
        assert_eq!(ruleset.version.major, 1);

        // Validate it
        let result = executor.validate_ruleset(&ruleset);
        assert!(result.is_ok(), "Valid ruleset should pass validation");
    }

    #[tokio::test]
    async fn test_ruleset_validation_missing_action_tiers() {
        let executor = setup_test_executor().await;

        let config = serde_json::json!({
            "maintainers": {},
            "repositories": {}
        });

        let ruleset = executor
            .create_ruleset_from_config(&config, "test")
            .unwrap();
        let result = executor.validate_ruleset(&ruleset);
        assert!(
            result.is_err(),
            "Should fail validation without action_tiers"
        );
    }

    #[tokio::test]
    async fn test_ruleset_validation_missing_maintainers() {
        let executor = setup_test_executor().await;

        let config = serde_json::json!({
            "action_tiers": {},
            "repositories": {}
        });

        let ruleset = executor
            .create_ruleset_from_config(&config, "test")
            .unwrap();
        let result = executor.validate_ruleset(&ruleset);
        assert!(
            result.is_err(),
            "Should fail validation without maintainers"
        );
    }

    #[tokio::test]
    async fn test_ruleset_validation_missing_repositories() {
        let executor = setup_test_executor().await;

        let config = serde_json::json!({
            "action_tiers": {},
            "maintainers": {}
        });

        let ruleset = executor
            .create_ruleset_from_config(&config, "test")
            .unwrap();
        let result = executor.validate_ruleset(&ruleset);
        assert!(
            result.is_err(),
            "Should fail validation without repositories"
        );
    }

    #[tokio::test]
    async fn test_calculate_config_hash() {
        let executor = setup_test_executor().await;

        let config1 = serde_json::json!({"key": "value1"});
        let config2 = serde_json::json!({"key": "value2"});

        let hash1 = executor.calculate_config_hash(&config1).unwrap();
        let hash2 = executor.calculate_config_hash(&config2).unwrap();

        assert_ne!(
            hash1, hash2,
            "Different configs should have different hashes"
        );
        assert!(!hash1.is_empty(), "Hash should not be empty");
        assert!(!hash2.is_empty(), "Hash should not be empty");
    }

    #[tokio::test]
    async fn test_calculate_config_hash_deterministic() {
        let executor = setup_test_executor().await;

        let config = serde_json::json!({"key": "value"});

        let hash1 = executor.calculate_config_hash(&config).unwrap();
        let hash2 = executor.calculate_config_hash(&config).unwrap();

        assert_eq!(hash1, hash2, "Same config should produce same hash");
    }

    #[tokio::test]
    async fn test_should_execute_fork_meets_thresholds() {
        let executor = setup_test_executor().await;

        let metrics = AdoptionMetrics {
            ruleset_id: "test".to_string(),
            node_count: 100,
            hashpower_percentage: 35.0,
            economic_activity_percentage: 45.0,
            total_weight: 50.0,
            last_updated: chrono::Utc::now(),
        };

        // Default thresholds are 0, so should pass
        assert!(
            executor.should_execute_fork(&metrics),
            "Should execute fork when thresholds met"
        );
    }

    #[tokio::test]
    async fn test_should_execute_fork_below_node_count() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        let thresholds = ForkThresholds {
            minimum_node_count: 100,
            minimum_hashpower_percentage: 0.0,
            minimum_economic_activity_percentage: 0.0,
            minimum_adoption_percentage: 0.0,
            grace_period_days: 30,
        };

        let executor =
            ForkExecutor::new(export_path.to_str().unwrap(), pool, Some(thresholds)).unwrap();

        let metrics = AdoptionMetrics {
            ruleset_id: "test".to_string(),
            node_count: 50, // Below threshold
            hashpower_percentage: 35.0,
            economic_activity_percentage: 45.0,
            total_weight: 50.0,
            last_updated: chrono::Utc::now(),
        };

        assert!(
            !executor.should_execute_fork(&metrics),
            "Should not execute fork when node count below threshold"
        );
    }

    #[tokio::test]
    async fn test_should_execute_fork_below_hashpower() {
        let temp_dir = tempdir().unwrap();
        let export_path = temp_dir.path().join("exports");
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        let thresholds = ForkThresholds {
            minimum_node_count: 0,
            minimum_hashpower_percentage: 30.0,
            minimum_economic_activity_percentage: 0.0,
            minimum_adoption_percentage: 0.0,
            grace_period_days: 30,
        };

        let executor =
            ForkExecutor::new(export_path.to_str().unwrap(), pool, Some(thresholds)).unwrap();

        let metrics = AdoptionMetrics {
            ruleset_id: "test".to_string(),
            node_count: 100,
            hashpower_percentage: 25.0, // Below threshold
            economic_activity_percentage: 45.0,
            total_weight: 50.0,
            last_updated: chrono::Utc::now(),
        };

        assert!(
            !executor.should_execute_fork(&metrics),
            "Should not execute fork when hashpower below threshold"
        );
    }

    #[tokio::test]
    async fn test_set_secret_key() {
        let mut executor = setup_test_executor().await;
        use secp256k1::rand::rngs::OsRng;

        let secret_key = SecretKey::new(&mut OsRng);
        executor.set_secret_key(secret_key);

        // Just verify it doesn't panic
        assert!(true, "Should set secret key without error");
    }

    #[tokio::test]
    async fn test_create_ruleset_from_config() {
        let executor = setup_test_executor().await;

        let config = serde_json::json!({
            "action_tiers": {},
            "maintainers": {},
            "repositories": {}
        });

        let ruleset = executor
            .create_ruleset_from_config(&config, "test-ruleset")
            .unwrap();

        assert_eq!(ruleset.id, "test-ruleset");
        assert_eq!(ruleset.name, "Governance Ruleset test-ruleset");
        assert_eq!(ruleset.version.major, 1);
        assert_eq!(ruleset.version.minor, 0);
        assert_eq!(ruleset.version.patch, 0);
        assert!(!ruleset.hash.is_empty());
    }
}
