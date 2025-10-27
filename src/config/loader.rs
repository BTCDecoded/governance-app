//! Configuration file loader for governance system
//! Loads YAML configuration files from the governance repository

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use crate::error::GovernanceError;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionTiersConfig {
    pub tiers: std::collections::HashMap<String, TierConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TierConfig {
    pub name: String,
    pub signatures_required: usize,
    pub signatures_total: usize,
    pub review_period_days: i64,
    pub economic_veto_required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RepositoryLayersConfig {
    pub layers: std::collections::HashMap<String, LayerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayerConfig {
    pub name: String,
    pub description: String,
    pub repositories: Vec<String>,
    pub signatures: SignatureConfig,
    pub review_period_days: i64,
    pub economic_veto_required: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignatureConfig {
    pub required: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TierClassificationConfig {
    pub classification_rules: std::collections::HashMap<String, ClassificationRule>,
    pub classification_config: ClassificationConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClassificationRule {
    pub priority: u32,
    pub name: String,
    pub file_patterns: Vec<String>,
    pub keywords: KeywordConfig,
    pub confidence_boost: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeywordConfig {
    pub title: Vec<String>,
    pub body: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClassificationConfig {
    pub min_confidence: f32,
    pub file_pattern_weight: f32,
    pub keyword_weight: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GovernanceConfigFiles {
    pub action_tiers: ActionTiersConfig,
    pub repository_layers: RepositoryLayersConfig,
    pub tier_classification: TierClassificationConfig,
}

impl GovernanceConfigFiles {
    /// Load all configuration files from a directory
    pub fn load_from_directory(path: &Path) -> Result<Self, GovernanceError> {
        info!("Loading governance configuration from: {:?}", path);

        let action_tiers = Self::load_yaml(path.join("action-tiers.yml"))?;
        let repository_layers = Self::load_yaml(path.join("repository-layers.yml"))?;
        let tier_classification = Self::load_yaml(path.join("tier-classification-rules.yml"))?;

        info!("Successfully loaded all governance configuration files");

        Ok(Self {
            action_tiers,
            repository_layers,
            tier_classification,
        })
    }

    /// Load a YAML file and deserialize it
    fn load_yaml<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, GovernanceError> {
        if !path.exists() {
            return Err(GovernanceError::ConfigError(format!(
                "Configuration file not found: {:?}",
                path
            )));
        }

        let contents = fs::read_to_string(&path)
            .map_err(|e| {
                GovernanceError::ConfigError(format!(
                    "Failed to read {:?}: {}",
                    path, e
                ))
            })?;

        serde_yaml::from_str(&contents)
            .map_err(|e| {
                GovernanceError::ConfigError(format!(
                    "Failed to parse {:?}: {}",
                    path, e
                ))
            })
    }

    /// Validate the loaded configuration
    pub fn validate(&self) -> Result<(), GovernanceError> {
        info!("Validating governance configuration");

        // Validate action tiers
        self.validate_action_tiers()?;

        // Validate repository layers
        self.validate_repository_layers()?;

        // Validate tier classification
        self.validate_tier_classification()?;

        info!("Configuration validation completed successfully");
        Ok(())
    }

    fn validate_action_tiers(&self) -> Result<(), GovernanceError> {
        if self.action_tiers.tiers.is_empty() {
            return Err(GovernanceError::ConfigError(
                "No action tiers defined".to_string(),
            ));
        }

        for (tier_name, tier_config) in &self.action_tiers.tiers {
            if tier_config.signatures_required > tier_config.signatures_total {
                return Err(GovernanceError::ConfigError(format!(
                    "Tier {}: signatures_required ({}) > signatures_total ({})",
                    tier_name, tier_config.signatures_required, tier_config.signatures_total
                )));
            }

            if tier_config.review_period_days < 0 {
                return Err(GovernanceError::ConfigError(format!(
                    "Tier {}: review_period_days ({}) cannot be negative",
                    tier_name, tier_config.review_period_days
                )));
            }
        }

        Ok(())
    }

    fn validate_repository_layers(&self) -> Result<(), GovernanceError> {
        if self.repository_layers.layers.is_empty() {
            return Err(GovernanceError::ConfigError(
                "No repository layers defined".to_string(),
            ));
        }

        for (layer_name, layer_config) in &self.repository_layers.layers {
            if layer_config.signatures.required > layer_config.signatures.total {
                return Err(GovernanceError::ConfigError(format!(
                    "Layer {}: signatures.required ({}) > signatures.total ({})",
                    layer_name, layer_config.signatures.required, layer_config.signatures.total
                )));
            }

            if layer_config.review_period_days < 0 {
                return Err(GovernanceError::ConfigError(format!(
                    "Layer {}: review_period_days ({}) cannot be negative",
                    layer_name, layer_config.review_period_days
                )));
            }

            if layer_config.repositories.is_empty() {
                return Err(GovernanceError::ConfigError(format!(
                    "Layer {}: no repositories defined",
                    layer_name
                )));
            }
        }

        Ok(())
    }

    fn validate_tier_classification(&self) -> Result<(), GovernanceError> {
        if self.tier_classification.classification_rules.is_empty() {
            return Err(GovernanceError::ConfigError(
                "No tier classification rules defined".to_string(),
            ));
        }

        let config = &self.tier_classification.classification_config;
        if config.min_confidence < 0.0 || config.min_confidence > 1.0 {
            return Err(GovernanceError::ConfigError(format!(
                "min_confidence ({}) must be between 0.0 and 1.0",
                config.min_confidence
            )));
        }

        if config.file_pattern_weight < 0.0 || config.file_pattern_weight > 1.0 {
            return Err(GovernanceError::ConfigError(format!(
                "file_pattern_weight ({}) must be between 0.0 and 1.0",
                config.file_pattern_weight
            )));
        }

        if config.keyword_weight < 0.0 || config.keyword_weight > 1.0 {
            return Err(GovernanceError::ConfigError(format!(
                "keyword_weight ({}) must be between 0.0 and 1.0",
                config.keyword_weight
            )));
        }

        Ok(())
    }

    /// Get tier configuration by tier number
    pub fn get_tier_config(&self, tier: u32) -> Option<&TierConfig> {
        let tier_name = format!("tier_{}", tier);
        self.action_tiers.tiers.get(&tier_name)
    }

    /// Get layer configuration by layer number
    pub fn get_layer_config(&self, layer: i32) -> Option<&LayerConfig> {
        let layer_name = match layer {
            1 | 2 => "layer_1_2_constitutional",
            3 => "layer_3_implementation",
            4 => "layer_4_application",
            5 => "layer_5_extension",
            _ => return None,
        };
        self.repository_layers.layers.get(layer_name)
    }

    /// Get tier classification rules
    pub fn get_classification_rules(&self) -> &std::collections::HashMap<String, ClassificationRule> {
        &self.tier_classification.classification_rules
    }

    /// Get classification configuration
    pub fn get_classification_config(&self) -> &ClassificationConfig {
        &self.tier_classification.classification_config
    }
}

/// Configuration cache for runtime updates
pub struct ConfigCache {
    config: GovernanceConfigFiles,
    last_updated: std::time::SystemTime,
    auto_reload: bool,
    config_path: PathBuf,
}

impl ConfigCache {
    /// Create a new configuration cache
    pub fn new(config: GovernanceConfigFiles, config_path: PathBuf, auto_reload: bool) -> Self {
        Self {
            config,
            last_updated: std::time::SystemTime::now(),
            auto_reload,
            config_path,
        }
    }

    /// Get the current configuration
    pub fn get_config(&self) -> &GovernanceConfigFiles {
        &self.config
    }

    /// Check if configuration needs to be reloaded
    pub fn needs_reload(&self) -> bool {
        if !self.auto_reload {
            return false;
        }

        // Check if any config file has been modified
        let config_files = [
            "action-tiers.yml",
            "repository-layers.yml",
            "tier-classification-rules.yml",
        ];

        for file in &config_files {
            let path = self.config_path.join(file);
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if modified > self.last_updated {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<(), GovernanceError> {
        info!("Reloading governance configuration");
        
        let new_config = GovernanceConfigFiles::load_from_directory(&self.config_path)?;
        new_config.validate()?;
        
        self.config = new_config;
        self.last_updated = std::time::SystemTime::now();
        
        info!("Configuration reloaded successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_config_validation() {
        let mut tiers = HashMap::new();
        tiers.insert("tier_1".to_string(), TierConfig {
            name: "Routine".to_string(),
            signatures_required: 3,
            signatures_total: 5,
            review_period_days: 7,
            economic_veto_required: false,
            description: "Routine maintenance".to_string(),
        });

        let action_tiers = ActionTiersConfig { tiers };
        let repository_layers = RepositoryLayersConfig {
            layers: HashMap::new(),
        };
        let tier_classification = TierClassificationConfig {
            classification_rules: HashMap::new(),
            classification_config: ClassificationConfig {
                min_confidence: 0.6,
                file_pattern_weight: 0.7,
                keyword_weight: 0.3,
            },
        };

        let config = GovernanceConfigFiles {
            action_tiers,
            repository_layers,
            tier_classification,
        };

        // This should fail because repository_layers is empty
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tier_config_access() {
        let mut tiers = HashMap::new();
        tiers.insert("tier_1".to_string(), TierConfig {
            name: "Routine".to_string(),
            signatures_required: 3,
            signatures_total: 5,
            review_period_days: 7,
            economic_veto_required: false,
            description: "Routine maintenance".to_string(),
        });

        let action_tiers = ActionTiersConfig { tiers };
        let repository_layers = RepositoryLayersConfig {
            layers: HashMap::new(),
        };
        let tier_classification = TierClassificationConfig {
            classification_rules: HashMap::new(),
            classification_config: ClassificationConfig {
                min_confidence: 0.6,
                file_pattern_weight: 0.7,
                keyword_weight: 0.3,
            },
        };

        let config = GovernanceConfigFiles {
            action_tiers,
            repository_layers,
            tier_classification,
        };

        let tier_1 = config.get_tier_config(1);
        assert!(tier_1.is_some());
        assert_eq!(tier_1.unwrap().name, "Routine");

        let tier_2 = config.get_tier_config(2);
        assert!(tier_2.is_none());
    }
}




