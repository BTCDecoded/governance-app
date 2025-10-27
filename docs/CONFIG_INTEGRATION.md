# Configuration Integration

## Overview

The BTCDecoded governance system integrates with configuration files from the governance repository to ensure that the governance-app behavior matches the documented governance rules. This document explains how the app loads and uses these configuration files.

## Configuration File Structure

### Governance Repository Structure

```
governance/
├── config/
│   ├── action-tiers.yml              # Tier definitions and requirements
│   ├── repository-layers.yml         # Layer definitions and requirements
│   ├── tier-classification-rules.yml # PR classification rules
│   ├── economic-nodes.yml            # Economic node configuration
│   ├── emergency-tiers.yml           # Emergency tier definitions
│   ├── governance-fork.yml           # Governance fork configuration
│   ├── maintainers/                  # Maintainer configurations by layer
│   │   ├── layer-1-2.yml
│   │   ├── layer-3.yml
│   │   ├── layer-4.yml
│   │   └── emergency.yml
│   └── repos/                        # Repository-specific configurations
│       ├── orange-paper.yml
│       ├── consensus-proof.yml
│       ├── protocol-engine.yml
│       ├── reference-node.yml
│       └── developer-sdk.yml
```

### Configuration Loading

**Config Loader Implementation**:
```rust
use crate::config::loader::GovernanceConfigFiles;

impl GovernanceConfigFiles {
    pub fn load_from_directory(path: &Path) -> Result<Self, GovernanceError> {
        let action_tiers = Self::load_yaml(path.join("action-tiers.yml"))?;
        let repository_layers = Self::load_yaml(path.join("repository-layers.yml"))?;
        let tier_classification = Self::load_yaml(path.join("tier-classification-rules.yml"))?;
        
        Ok(Self {
            action_tiers,
            repository_layers,
            tier_classification,
        })
    }
}
```

## Action Tiers Configuration

### Tier Definitions

**action-tiers.yml**:
```yaml
tiers:
  tier1:
    name: "Routine Maintenance"
    signatures_required: 3
    signatures_total: 5
    review_period_days: 7
    economic_veto_required: false
    description: "Bug fixes, documentation, performance improvements"
  
  tier2:
    name: "Feature Changes"
    signatures_required: 4
    signatures_total: 5
    review_period_days: 30
    economic_veto_required: false
    description: "New RPC methods, P2P changes, wallet features"
  
  tier3:
    name: "Consensus-Adjacent"
    signatures_required: 5
    signatures_total: 5
    review_period_days: 90
    economic_veto_required: true
    description: "Changes affecting consensus validation"
  
  tier4:
    name: "Emergency Actions"
    signatures_required: 4
    signatures_total: 5
    review_period_days: 0
    economic_veto_required: false
    description: "Critical security patches, network threats"
  
  tier5:
    name: "Governance Changes"
    signatures_required: 6
    signatures_total: 7
    review_period_days: 180
    economic_veto_required: false
    description: "Changes to governance rules themselves"
```

### Tier Loading

**Programmatic Loading**:
```rust
use crate::config::loader::{ActionTiersConfig, TierConfig};

async fn load_action_tiers(config_path: &str) -> Result<ActionTiersConfig, Error> {
    let path = Path::new(config_path).join("action-tiers.yml");
    let content = std::fs::read_to_string(&path)?;
    let config: ActionTiersConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

async fn get_tier_requirements(tier_name: &str) -> Result<TierConfig, Error> {
    let config = load_action_tiers("governance/config").await?;
    config.tiers.get(tier_name)
        .ok_or_else(|| Error::TierNotFound(tier_name.to_string()))
        .cloned()
}
```

## Repository Layers Configuration

### Layer Definitions

**repository-layers.yml**:
```yaml
layers:
  layer1:
    name: "Orange Paper"
    description: "Constitutional layer - fundamental Bitcoin principles"
    repositories: ["orange-paper"]
    signatures:
      required: 6
      total: 7
    review_period_days: 180
    economic_veto_required: false
    rationale: "Constitutional changes require highest consensus"
  
  layer2:
    name: "Consensus Proof"
    description: "Mathematical proofs and consensus validation"
    repositories: ["consensus-proof"]
    signatures:
      required: 6
      total: 7
    review_period_days: 180
    economic_veto_required: false
    rationale: "Consensus validation requires highest consensus"
  
  layer3:
    name: "Protocol Engine"
    description: "Bitcoin protocol implementation"
    repositories: ["protocol-engine"]
    signatures:
      required: 4
      total: 5
    review_period_days: 90
    economic_veto_required: false
    rationale: "Protocol implementation requires technical consensus"
  
  layer4:
    name: "Reference Node"
    description: "Complete Bitcoin node implementation"
    repositories: ["reference-node"]
    signatures:
      required: 3
      total: 5
    review_period_days: 60
    economic_veto_required: false
    rationale: "Node implementation requires operational consensus"
  
  layer5:
    name: "Developer SDK"
    description: "Developer tools and libraries"
    repositories: ["developer-sdk"]
    signatures:
      required: 2
      total: 3
    review_period_days: 14
    economic_veto_required: false
    rationale: "Developer tools require minimal consensus"
```

### Layer Loading

**Programmatic Loading**:
```rust
use crate::config::loader::{RepositoryLayersConfig, LayerConfig};

async fn load_repository_layers(config_path: &str) -> Result<RepositoryLayersConfig, Error> {
    let path = Path::new(config_path).join("repository-layers.yml");
    let content = std::fs::read_to_string(&path)?;
    let config: RepositoryLayersConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

async fn get_layer_requirements(layer_name: &str) -> Result<LayerConfig, Error> {
    let config = load_repository_layers("governance/config").await?;
    config.layers.get(layer_name)
        .ok_or_else(|| Error::LayerNotFound(layer_name.to_string()))
        .cloned()
}
```

## Tier Classification Rules

### Classification Configuration

**tier-classification-rules.yml**:
```yaml
classification_rules:
  consensus_adjacent:
    priority: 100
    name: "Consensus-Adjacent Changes"
    file_patterns:
      - "src/consensus/*.rs"
      - "src/validation/*.rs"
      - "src/blockchain/*.rs"
    keywords:
      title:
        - "consensus"
        - "validation"
        - "blockchain"
        - "mining"
      body:
        - "consensus rules"
        - "validation logic"
        - "blockchain validation"
    confidence_boost: 0.3
  
  feature_changes:
    priority: 80
    name: "Feature Changes"
    file_patterns:
      - "src/rpc/*.rs"
      - "src/p2p/*.rs"
      - "src/wallet/*.rs"
    keywords:
      title:
        - "rpc"
        - "p2p"
        - "wallet"
        - "feature"
      body:
        - "new rpc method"
        - "p2p protocol"
        - "wallet functionality"
    confidence_boost: 0.2
  
  routine_maintenance:
    priority: 60
    name: "Routine Maintenance"
    file_patterns:
      - "src/utils/*.rs"
      - "src/common/*.rs"
      - "docs/*.md"
    keywords:
      title:
        - "fix"
        - "bug"
        - "documentation"
        - "performance"
      body:
        - "bug fix"
        - "documentation update"
        - "performance improvement"
    confidence_boost: 0.1

classification_config:
  min_confidence: 0.5
  file_pattern_weight: 0.7
  keyword_weight: 0.3
```

### Classification Loading

**Programmatic Loading**:
```rust
use crate::config::loader::{TierClassificationConfig, ClassificationRule};

async fn load_tier_classification(config_path: &str) -> Result<TierClassificationConfig, Error> {
    let path = Path::new(config_path).join("tier-classification-rules.yml");
    let content = std::fs::read_to_string(&path)?;
    let config: TierClassificationConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

async fn classify_pr(pr_data: &PullRequestData) -> Result<String, Error> {
    let config = load_tier_classification("governance/config").await?;
    let mut best_match = None;
    let mut best_confidence = 0.0;
    
    for (tier_name, rule) in &config.classification_rules {
        let confidence = calculate_confidence(pr_data, rule, &config.classification_config)?;
        if confidence > best_confidence && confidence >= config.classification_config.min_confidence {
            best_confidence = confidence;
            best_match = Some(tier_name.clone());
        }
    }
    
    best_match.ok_or_else(|| Error::ClassificationFailed)
}
```

## Combined Requirements

### Layer + Tier Combination

**Combined Requirements Calculation**:
```rust
use crate::config::loader::GovernanceConfigFiles;

impl GovernanceConfigFiles {
    pub fn get_combined_requirements(
        &self,
        layer_name: &str,
        tier_name: &str,
    ) -> Result<CombinedRequirements, Error> {
        let layer = self.repository_layers.layers.get(layer_name)
            .ok_or_else(|| Error::LayerNotFound(layer_name.to_string()))?;
        
        let tier = self.action_tiers.tiers.get(tier_name)
            .ok_or_else(|| Error::TierNotFound(tier_name.to_string()))?;
        
        // Use most restrictive requirements
        let signatures_required = std::cmp::max(
            layer.signatures.required,
            tier.signatures_required
        );
        
        let signatures_total = std::cmp::max(
            layer.signatures.total,
            tier.signatures_total
        );
        
        let review_period_days = std::cmp::max(
            layer.review_period_days,
            tier.review_period_days
        );
        
        let economic_veto_required = layer.economic_veto_required || tier.economic_veto_required;
        
        Ok(CombinedRequirements {
            signatures_required,
            signatures_total,
            review_period_days,
            economic_veto_required,
            layer_name: layer_name.to_string(),
            tier_name: tier_name.to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CombinedRequirements {
    pub signatures_required: usize,
    pub signatures_total: usize,
    pub review_period_days: i64,
    pub economic_veto_required: bool,
    pub layer_name: String,
    pub tier_name: String,
}
```

## Configuration Validation

### Validation Rules

**Configuration Validation**:
```rust
impl GovernanceConfigFiles {
    pub fn validate(&self) -> Result<(), GovernanceError> {
        // Validate action tiers
        for (tier_name, tier) in &self.action_tiers.tiers {
            if tier.signatures_required > tier.signatures_total {
                return Err(GovernanceError::InvalidConfig(
                    format!("Tier {}: required signatures > total signatures", tier_name)
                ));
            }
            
            if tier.review_period_days < 0 {
                return Err(GovernanceError::InvalidConfig(
                    format!("Tier {}: negative review period", tier_name)
                ));
            }
        }
        
        // Validate repository layers
        for (layer_name, layer) in &self.repository_layers.layers {
            if layer.signatures.required > layer.signatures.total {
                return Err(GovernanceError::InvalidConfig(
                    format!("Layer {}: required signatures > total signatures", layer_name)
                ));
            }
            
            if layer.review_period_days < 0 {
                return Err(GovernanceError::InvalidConfig(
                    format!("Layer {}: negative review period", layer_name)
                ));
            }
        }
        
        // Validate tier classification
        for (rule_name, rule) in &self.tier_classification.classification_rules {
            if rule.file_patterns.is_empty() {
                return Err(GovernanceError::InvalidConfig(
                    format!("Rule {}: no file patterns", rule_name)
                ));
            }
            
            if rule.confidence_boost < 0.0 || rule.confidence_boost > 1.0 {
                return Err(GovernanceError::InvalidConfig(
                    format!("Rule {}: invalid confidence boost", rule_name)
                ));
            }
        }
        
        Ok(())
    }
}
```

## Configuration Loading in Application

### Main Application Integration

**Configuration Loading**:
```rust
use crate::config::loader::GovernanceConfigFiles;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load app configuration
    let app_config = AppConfig::load()?;
    
    // Load governance configuration
    let governance_config = GovernanceConfigFiles::load_from_directory(
        Path::new("governance/config")
    )?;
    
    // Validate configuration
    governance_config.validate()?;
    
    // Initialize database
    let database = Database::new(&app_config.database_url).await?;
    
    // Initialize governance app with config
    let governance_app = GovernanceApp::new(
        app_config,
        governance_config,
        database,
    )?;
    
    // Start application
    governance_app.run().await?;
    
    Ok(())
}
```

### Tier Classification Integration

**Updated Tier Classification**:
```rust
use crate::config::loader::GovernanceConfigFiles;

pub struct TierClassifier {
    config: TierClassificationConfig,
}

impl TierClassifier {
    pub fn new(governance_config: &GovernanceConfigFiles) -> Self {
        Self {
            config: governance_config.tier_classification.clone(),
        }
    }
    
    pub async fn classify_pr(&self, pr_data: &PullRequestData) -> Result<String, Error> {
        let mut best_match = None;
        let mut best_confidence = 0.0;
        
        for (tier_name, rule) in &self.config.classification_rules {
            let confidence = self.calculate_confidence(pr_data, rule)?;
            if confidence > best_confidence && confidence >= self.config.classification_config.min_confidence {
                best_confidence = confidence;
                best_match = Some(tier_name.clone());
            }
        }
        
        best_match.ok_or_else(|| Error::ClassificationFailed)
    }
    
    fn calculate_confidence(
        &self,
        pr_data: &PullRequestData,
        rule: &ClassificationRule,
    ) -> Result<f32, Error> {
        let mut confidence = 0.0;
        
        // File pattern matching
        let file_matches = self.match_file_patterns(&pr_data.changed_files, &rule.file_patterns);
        confidence += file_matches * self.config.classification_config.file_pattern_weight;
        
        // Keyword matching
        let keyword_matches = self.match_keywords(&pr_data.title, &pr_data.body, &rule.keywords);
        confidence += keyword_matches * self.config.classification_config.keyword_weight;
        
        // Apply confidence boost
        confidence += rule.confidence_boost;
        
        Ok(confidence.min(1.0))
    }
}
```

## Configuration File Precedence

### Loading Order

1. **Default Configuration**: Built-in default values
2. **File Configuration**: Configuration files from governance repo
3. **Environment Variables**: Environment variable overrides
4. **Command Line Arguments**: Command line argument overrides

### Override Mechanism

**Configuration Override**:
```rust
impl AppConfig {
    pub fn load_with_overrides(
        config_file: Option<&str>,
        env_overrides: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::load_default()?;
        
        // Load from file if specified
        if let Some(file_path) = config_file {
            let file_config = Self::load_from_file(file_path)?;
            config.merge(file_config);
        }
        
        // Apply environment overrides
        if env_overrides {
            config.apply_env_overrides()?;
        }
        
        Ok(config)
    }
    
    fn merge(&mut self, other: Self) {
        // Merge configuration values
        if !other.database_url.is_empty() {
            self.database_url = other.database_url;
        }
        // ... merge other fields
    }
}
```

## Troubleshooting

### Common Issues

1. **Configuration File Not Found**
   - Check file path is correct
   - Verify file exists and is readable
   - Check file permissions

2. **Configuration Parse Error**
   - Validate YAML syntax
   - Check required fields are present
   - Verify field types match expected types

3. **Configuration Validation Failed**
   - Check signature requirements are valid
   - Verify review periods are positive
   - Check classification rules are complete

### Debug Commands

```bash
# Validate configuration
governance-app config validate --config-path governance/config

# Test configuration loading
governance-app config test --config-path governance/config

# Show configuration
governance-app config show --config-path governance/config
```

### Log Analysis

```bash
# Check configuration loading logs
sudo journalctl -u governance-app | grep "config.*load"

# Check configuration validation
sudo journalctl -u governance-app | grep "config.*validate"

# Check configuration errors
sudo journalctl -u governance-app | grep "config.*error"
```

## Best Practices

### Configuration Management

1. **Version Control**: Keep configuration files in version control
2. **Validation**: Validate configuration on startup
3. **Documentation**: Document all configuration options
4. **Testing**: Test configuration changes thoroughly

### File Organization

1. **Logical Grouping**: Group related configurations together
2. **Clear Naming**: Use clear, descriptive file names
3. **Consistent Format**: Use consistent YAML formatting
4. **Comments**: Add comments for complex configurations

### Security

1. **File Permissions**: Set appropriate file permissions
2. **Sensitive Data**: Don't store sensitive data in config files
3. **Validation**: Validate all configuration inputs
4. **Audit**: Audit configuration changes

## References

- [Configuration Reference](CONFIGURATION.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
- [Main Governance Documentation](../governance/README.md)
- [Layer + Tier Model](../governance/LAYER_TIER_MODEL.md)
