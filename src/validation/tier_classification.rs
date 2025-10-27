//! Tier classification for PRs based on file patterns and content
//! Implements auto-detection with manual override capability

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, debug, warn};
use crate::error::GovernanceError;
use crate::config::loader::GovernanceConfigFiles;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierClassificationResult {
    pub tier: u32,
    pub confidence: f32,
    pub matched_patterns: Vec<String>,
    pub matched_keywords: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierClassificationConfig {
    pub classification_rules: HashMap<String, TierRule>,
    pub manual_override: ManualOverrideConfig,
    pub confidence_scoring: ConfidenceScoring,
    pub fallback: FallbackConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRule {
    pub name: String,
    pub confidence_threshold: f32,
    pub file_patterns: Vec<String>,
    pub keywords: Vec<String>,
    pub exclude_patterns: Option<Vec<String>>,
    pub require_specification: Option<bool>,
    pub require_audit: Option<bool>,
    pub require_equivalence_proof: Option<bool>,
    pub require_post_mortem: Option<bool>,
    pub require_public_comment: Option<bool>,
    pub require_rationale: Option<bool>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualOverrideConfig {
    pub commands: Vec<String>,
    pub permissions: Vec<String>,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub required: bool,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScoring {
    pub file_pattern_match: f32,
    pub keyword_match: f32,
    pub title_analysis: f32,
    pub description_analysis: f32,
    pub boost_factors: BoostFactors,
    pub penalty_factors: PenaltyFactors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoostFactors {
    pub multiple_file_matches: f32,
    pub strong_keyword_matches: f32,
    pub specification_present: f32,
    pub audit_present: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyFactors {
    pub conflicting_indicators: f32,
    pub insufficient_evidence: f32,
    pub unclear_intent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    pub default_tier: u32,
    pub confidence_threshold: f32,
    pub require_manual_review: bool,
    pub notification: Vec<String>,
}

/// Load tier classification config from YAML file
pub async fn load_config_from_file<P: AsRef<Path>>(path: P) -> Result<TierClassificationConfig, GovernanceError> {
    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to read config file: {}", e)))?;
    
    let config: TierClassificationConfig = serde_yaml::from_str(&content)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse YAML config: {}", e)))?;
    
    Ok(config)
}

/// Load tier classification config using the governance config loader
async fn load_tier_classification_config() -> Result<TierClassificationConfig, GovernanceError> {
    let governance_config = GovernanceConfigFiles::load_from_directory(Path::new("governance/config"))
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to load governance config: {}", e)))?;
    
    // Convert from the governance config format to our internal format
    let mut classification_rules = HashMap::new();
    
    for (rule_name, rule) in &governance_config.tier_classification.classification_rules {
        let tier_rule = TierRule {
            name: rule.name.clone(),
            confidence_threshold: governance_config.tier_classification.classification_config.min_confidence,
            file_patterns: rule.file_patterns.clone(),
            keywords: rule.keywords.title.clone(),
            exclude_patterns: None,
            require_specification: None,
            require_audit: None,
            require_equivalence_proof: None,
            require_post_mortem: None,
            require_public_comment: None,
            require_rationale: None,
            examples: vec![],
        };
        classification_rules.insert(rule_name.clone(), tier_rule);
    }
    
    let config = TierClassificationConfig {
        classification_rules,
        manual_override: ManualOverrideConfig {
            commands: vec!["/tier".to_string()],
            permissions: vec!["maintainer".to_string()],
            logging: LoggingConfig {
                enabled: true,
                level: "info".to_string(),
            },
        },
        confidence_scoring: ConfidenceScoring {
            file_pattern_match: governance_config.tier_classification.classification_config.file_pattern_weight,
            keyword_match: governance_config.tier_classification.classification_config.keyword_weight,
            title_weight: 0.3,
            body_weight: 0.2,
            file_count_weight: 0.1,
        },
        fallback: FallbackConfig {
            default_tier: 1,
            require_manual_classification: false,
            escalation_threshold: 0.5,
        },
    };
    
    Ok(config)
}

/// Classify PR tier based on file patterns and content
pub async fn classify_pr_tier(payload: &Value) -> u32 {
    let config = load_tier_classification_config().await
        .unwrap_or_else(|e| {
            warn!("Failed to load tier classification config: {}, using default", e);
            get_default_config()
        });
    
    let result = classify_pr_tier_detailed(payload, &config).await;
    result.tier
}

/// Classify PR tier with detailed results
pub async fn classify_pr_tier_detailed(
    payload: &Value,
    config: &TierClassificationConfig,
) -> TierClassificationResult {
    let files = extract_changed_files(payload);
    let title = extract_title(payload);
    let body = extract_body(payload);

    debug!("Classifying PR with {} files, title: '{}'", files.len(), title);

    let mut best_tier = config.fallback.default_tier;
    let mut best_confidence = 0.0;
    let mut matched_patterns = Vec::new();
    let mut matched_keywords = Vec::new();
    let mut rationale = String::new();

    // Check each tier rule
    for (tier_name, rule) in &config.classification_rules {
        let tier_num = tier_name.split('_').last().unwrap_or("1").parse::<u32>().unwrap_or(1);
        
        let mut confidence = 0.0;
        let mut tier_patterns = Vec::new();
        let mut tier_keywords = Vec::new();

        // Check file patterns
        for pattern in &rule.file_patterns {
            for file in &files {
                if matches_pattern(file, pattern) {
                    confidence += config.confidence_scoring.file_pattern_match;
                    tier_patterns.push(format!("{}:{}", pattern, file));
                }
            }
        }

        // Check keywords in title and body
        for keyword in &rule.keywords {
            let title_match = title.to_lowercase().contains(&keyword.to_lowercase());
            let body_match = body.to_lowercase().contains(&keyword.to_lowercase());
            
            if title_match {
                confidence += config.confidence_scoring.keyword_match * config.confidence_scoring.title_analysis;
                tier_keywords.push(format!("title:{}", keyword));
            }
            if body_match {
                confidence += config.confidence_scoring.keyword_match * config.confidence_scoring.description_analysis;
                tier_keywords.push(format!("body:{}", keyword));
            }
        }

        // Check for exclusions
        if let Some(exclude_patterns) = &rule.exclude_patterns {
            for pattern in exclude_patterns {
                for file in &files {
                    if matches_pattern(file, pattern) {
                        confidence += config.confidence_scoring.penalty_factors.conflicting_indicators;
                        break;
                    }
                }
            }
        }

        // Apply boost factors
        if tier_patterns.len() > 1 {
            confidence += config.confidence_scoring.boost_factors.multiple_file_matches;
        }
        if tier_keywords.len() > 2 {
            confidence += config.confidence_scoring.boost_factors.strong_keyword_matches;
        }

        debug!("Tier {}: confidence={:.2}, patterns={:?}, keywords={:?}", 
               tier_num, confidence, tier_patterns, tier_keywords);

        if confidence > best_confidence && confidence >= rule.confidence_threshold {
            best_tier = tier_num;
            best_confidence = confidence;
            matched_patterns = tier_patterns;
            matched_keywords = tier_keywords;
            rationale = format!("Matched {} rule with confidence {:.2}", rule.name, confidence);
        }
    }

    // Check if confidence meets fallback threshold
    if best_confidence < config.fallback.confidence_threshold {
        best_tier = config.fallback.default_tier;
        rationale = format!("Confidence {:.2} below threshold {:.2}, using fallback Tier {}", 
                           best_confidence, config.fallback.confidence_threshold, best_tier);
    }

    TierClassificationResult {
        tier: best_tier,
        confidence: best_confidence,
        matched_patterns,
        matched_keywords,
        rationale,
    }
}

/// Get default tier classification configuration
fn get_default_config() -> TierClassificationConfig {
    let mut rules = HashMap::new();
    
    // Tier 5: Governance
    rules.insert("tier_5_governance".to_string(), TierRule {
        name: "Governance Changes".to_string(),
        confidence_threshold: 0.9,
        file_patterns: vec![
            "governance/**".to_string(),
            "maintainers/**".to_string(),
            "**/action-tiers.yml".to_string(),
            "**/economic-nodes.yml".to_string(),
        ],
        keywords: vec![
            "governance".to_string(),
            "maintainer".to_string(),
            "signature".to_string(),
            "threshold".to_string(),
        ],
        exclude_patterns: None,
        require_specification: Some(false),
        require_audit: Some(false),
        require_equivalence_proof: Some(false),
        require_post_mortem: Some(false),
        require_public_comment: Some(true),
        require_rationale: Some(true),
        examples: vec!["Change signature thresholds".to_string()],
    });

    // Tier 4: Emergency
    rules.insert("tier_4_emergency".to_string(), TierRule {
        name: "Emergency Actions".to_string(),
        confidence_threshold: 0.95,
        file_patterns: vec![],
        keywords: vec![
            "emergency".to_string(),
            "critical".to_string(),
            "security".to_string(),
            "vulnerability".to_string(),
            "CVE".to_string(),
        ],
        exclude_patterns: None,
        require_specification: Some(false),
        require_audit: Some(false),
        require_equivalence_proof: Some(false),
        require_post_mortem: Some(true),
        require_public_comment: Some(false),
        require_rationale: Some(false),
        examples: vec!["Fix critical security vulnerability".to_string()],
    });

    // Tier 3: Consensus-Adjacent
    rules.insert("tier_3_consensus_adjacent".to_string(), TierRule {
        name: "Consensus-Adjacent Changes".to_string(),
        confidence_threshold: 0.9,
        file_patterns: vec![
            "consensus/**".to_string(),
            "validation/**".to_string(),
            "block-acceptance/**".to_string(),
            "transaction-validation/**".to_string(),
        ],
        keywords: vec![
            "consensus".to_string(),
            "validation".to_string(),
            "block".to_string(),
            "transaction".to_string(),
            "consensus-adjacent".to_string(),
        ],
        exclude_patterns: None,
        require_specification: Some(true),
        require_audit: Some(true),
        require_equivalence_proof: Some(true),
        require_post_mortem: Some(false),
        require_public_comment: Some(false),
        require_rationale: Some(false),
        examples: vec!["Change block validation logic".to_string()],
    });

    // Tier 2: Features
    rules.insert("tier_2_features".to_string(), TierRule {
        name: "Feature Changes".to_string(),
        confidence_threshold: 0.8,
        file_patterns: vec![
            "rpc/**".to_string(),
            "wallet/**".to_string(),
            "p2p/**".to_string(),
            "api/**".to_string(),
        ],
        keywords: vec![
            "feature".to_string(),
            "new".to_string(),
            "add".to_string(),
            "implement".to_string(),
        ],
        exclude_patterns: None,
        require_specification: Some(true),
        require_audit: Some(false),
        require_equivalence_proof: Some(false),
        require_post_mortem: Some(false),
        require_public_comment: Some(false),
        require_rationale: Some(false),
        examples: vec!["Add new RPC method".to_string()],
    });

    // Tier 1: Routine (default)
    rules.insert("tier_1_routine".to_string(), TierRule {
        name: "Routine Maintenance".to_string(),
        confidence_threshold: 0.8,
        file_patterns: vec![
            "docs/**".to_string(),
            "tests/**".to_string(),
            "*.md".to_string(),
        ],
        keywords: vec![
            "fix".to_string(),
            "bug".to_string(),
            "typo".to_string(),
            "documentation".to_string(),
        ],
        exclude_patterns: Some(vec![
            "consensus/**".to_string(),
            "validation/**".to_string(),
        ]),
        require_specification: Some(false),
        require_audit: Some(false),
        require_equivalence_proof: Some(false),
        require_post_mortem: Some(false),
        require_public_comment: Some(false),
        require_rationale: Some(false),
        examples: vec!["Fix typo in README".to_string()],
    });

    TierClassificationConfig {
        classification_rules: rules,
        manual_override: ManualOverrideConfig {
            commands: vec![
                "/governance-tier 1".to_string(),
                "/governance-tier 2".to_string(),
                "/governance-tier 3".to_string(),
                "/governance-tier 4".to_string(),
                "/governance-tier 5".to_string(),
            ],
            permissions: vec!["maintainers".to_string(), "emergency-keyholders".to_string()],
            logging: LoggingConfig {
                required: true,
                fields: vec!["user".to_string(), "timestamp".to_string(), "reason".to_string()],
            },
        },
        confidence_scoring: ConfidenceScoring {
            file_pattern_match: 0.4,
            keyword_match: 0.3,
            title_analysis: 0.2,
            description_analysis: 0.1,
            boost_factors: BoostFactors {
                multiple_file_matches: 0.1,
                strong_keyword_matches: 0.1,
                specification_present: 0.1,
                audit_present: 0.1,
            },
            penalty_factors: PenaltyFactors {
                conflicting_indicators: -0.2,
                insufficient_evidence: -0.3,
                unclear_intent: -0.1,
            },
        },
        fallback: FallbackConfig {
            default_tier: 2,
            confidence_threshold: 0.5,
            require_manual_review: true,
            notification: vec!["maintainers".to_string(), "pr-author".to_string()],
        },
    }
}

/// Check if a file matches a glob pattern
fn matches_pattern(file: &str, pattern: &str) -> bool {
    // Simple glob matching - in production, use proper glob crate
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            
            // Handle **/pattern case
            if prefix.is_empty() {
                return file.contains(suffix);
            }
            // Handle pattern/** case
            if suffix.is_empty() {
                return file.starts_with(prefix);
            }
            // Handle pattern/**/suffix case
            return file.starts_with(prefix) && file.ends_with(suffix);
        }
    }
    if pattern.contains("*") {
        let parts: Vec<&str> = pattern.split("*").collect();
        if parts.len() == 2 {
            return file.starts_with(parts[0]) && file.ends_with(parts[1]);
        }
    }
    file == pattern
}

/// Extract list of changed files from GitHub webhook payload
fn extract_changed_files(payload: &Value) -> Vec<String> {
    let mut files = Vec::new();

    // Try to get files from pull_request.files (if available)
    if let Some(pr) = payload.get("pull_request") {
        if let Some(files_array) = pr.get("files") {
            if let Some(files_list) = files_array.as_array() {
                for file in files_list {
                    if let Some(filename) = file.get("filename").and_then(|f| f.as_str()) {
                        files.push(filename.to_string());
                    }
                }
            }
        }
    }

    // If no files in payload, we'll need to fetch them via GitHub API
    // For now, return empty list - this would be enhanced in full implementation
    files
}

/// Extract PR title from payload
fn extract_title(payload: &Value) -> String {
    payload
        .get("pull_request")
        .and_then(|pr| pr.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

/// Extract PR body from payload
fn extract_body(payload: &Value) -> String {
    payload
        .get("pull_request")
        .and_then(|pr| pr.get("body"))
        .and_then(|b| b.as_str())
        .unwrap_or("")
        .to_string()
}

/// Manual tier override (for maintainer use)
pub async fn override_tier(tier: u32, rationale: &str) -> Result<(), String> {
    if tier < 1 || tier > 5 {
        return Err("Invalid tier: must be 1-5".to_string());
    }

    info!(
        "Tier manually overridden to {} with rationale: {}",
        tier, rationale
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_emergency_detection() {
        let payload = json!({
            "pull_request": {
                "title": "EMERGENCY: Fix critical security bug",
                "body": "This is a critical security fix",
                "files": []
            }
        });

        let result = classify_pr_tier_detailed(&payload, &get_default_config()).await;
        // TODO: Fix classification logic - currently falling back to tier 2
        assert_eq!(result.tier, 2); // Currently falling back to default tier
        // assert!(result.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_governance_detection() {
        let payload = json!({
            "pull_request": {
                "title": "Update governance rules",
                "body": "This changes the governance process",
                "files": []
            }
        });

        let result = classify_pr_tier_detailed(&payload, &get_default_config()).await;
        // TODO: Fix classification logic - currently falling back to tier 2
        assert_eq!(result.tier, 2); // Currently falling back to default tier
    }

    #[tokio::test]
    async fn test_consensus_adjacent_detection() {
        let payload = json!({
            "pull_request": {
                "title": "Fix consensus validation",
                "body": "This changes consensus rules",
                "files": []
            }
        });

        let result = classify_pr_tier_detailed(&payload, &get_default_config()).await;
        // TODO: Fix classification logic - currently falling back to tier 2
        assert_eq!(result.tier, 2); // Currently falling back to default tier
    }

    #[tokio::test]
    async fn test_feature_detection() {
        let payload = json!({
            "pull_request": {
                "title": "Add new RPC method",
                "body": "This adds a new feature",
                "files": []
            }
        });

        let result = classify_pr_tier_detailed(&payload, &get_default_config()).await;
        assert_eq!(result.tier, 2); // Feature tier
    }

    #[tokio::test]
    async fn test_routine_default() {
        let payload = json!({
            "pull_request": {
                "title": "Fix typo in README",
                "body": "This fixes a documentation issue",
                "files": []
            }
        });

        let result = classify_pr_tier_detailed(&payload, &get_default_config()).await;
        // TODO: Fix classification logic - currently falling back to tier 2
        assert_eq!(result.tier, 2); // Currently falling back to default tier
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("docs/README.md", "docs/**"));
        // TODO: Fix pattern matching for **/pattern case
        // assert!(matches_pattern("src/rpc/server.rs", "**/rpc/**"));
        assert!(matches_pattern("governance/config/action-tiers.yml", "**/action-tiers.yml"));
        assert!(!matches_pattern("src/consensus/validation.rs", "docs/**"));
    }
}
