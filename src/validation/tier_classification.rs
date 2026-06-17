//! Tier classification for PRs based on file patterns and content
//! Implements auto-detection with manual override capability

use crate::config::loader::GovernanceConfigFiles;
use crate::error::GovernanceError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

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
pub async fn load_config_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<TierClassificationConfig, GovernanceError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to read config file: {e}")))?;

    let config: TierClassificationConfig = serde_yaml::from_str(&content)
        .map_err(|e| GovernanceError::ConfigError(format!("Failed to parse YAML config: {e}")))?;

    Ok(config)
}

/// Load tier classification config using the governance config loader
async fn load_tier_classification_config() -> Result<TierClassificationConfig, GovernanceError> {
    let governance_config = GovernanceConfigFiles::load_from_directory(Path::new(
        "governance/config",
    ))
    .map_err(|e| GovernanceError::ConfigError(format!("Failed to load governance config: {e}")))?;

    // Convert from the governance config format to our internal format
    let mut classification_rules = HashMap::new();

    for (rule_name, rule) in &governance_config.tier_classification.classification_rules {
        let tier_rule = TierRule {
            name: rule.name.clone(),
            confidence_threshold: governance_config
                .tier_classification
                .classification_config
                .min_confidence,
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
                required: true,
                fields: vec![
                    "user".to_string(),
                    "timestamp".to_string(),
                    "reason".to_string(),
                ],
            },
        },
        confidence_scoring: ConfidenceScoring {
            file_pattern_match: governance_config
                .tier_classification
                .classification_config
                .file_pattern_weight,
            keyword_match: governance_config
                .tier_classification
                .classification_config
                .keyword_weight,
            title_analysis: 0.3,
            description_analysis: 0.2,
            boost_factors: BoostFactors {
                multiple_file_matches: 0.1,
                strong_keyword_matches: 0.15,
                specification_present: 0.2,
                audit_present: 0.25,
            },
            penalty_factors: PenaltyFactors {
                conflicting_indicators: -0.2,
                insufficient_evidence: -0.3,
                unclear_intent: -0.15,
            },
        },
        fallback: FallbackConfig {
            default_tier: 1,
            confidence_threshold: 0.5,
            require_manual_review: false,
            notification: vec![],
        },
    };

    Ok(config)
}

/// Classify PR tier based on file patterns and content
pub async fn classify_pr_tier(payload: &Value) -> u32 {
    let config = load_tier_classification_config().await.unwrap_or_else(|e| {
        warn!(
            "Failed to load tier classification config: {}, using default",
            e
        );
        get_default_config()
    });

    let result = classify_pr_tier_detailed(payload, &config).await;
    result.tier
}

/// Classify PR tier with database override checking
/// Checks for tier override first, then falls back to automated classification
pub async fn classify_pr_tier_with_db(
    database: &crate::database::Database,
    payload: &Value,
    repo_name: &str,
    pr_number: i32,
) -> u32 {
    // Check for tier override first
    match database.get_tier_override(repo_name, pr_number).await {
        Ok(Some(override_record)) => {
            info!(
                "Using tier override for PR #{} in {}: Tier {} (justification: {})",
                pr_number, repo_name, override_record.override_tier, override_record.justification
            );
            return override_record.override_tier;
        }
        Ok(None) => {
            // No override, proceed with automated classification
        }
        Err(e) => {
            warn!(
                "Failed to check for tier override: {}, using automated classification",
                e
            );
        }
    }

    // Fall back to automated classification
    classify_pr_tier(payload).await
}

/// Classify PR tier with detailed results
pub async fn classify_pr_tier_detailed(
    payload: &Value,
    config: &TierClassificationConfig,
) -> TierClassificationResult {
    let files = extract_changed_files(payload);
    let title = extract_title(payload);
    let body = extract_body(payload);

    debug!(
        "Classifying PR with {} files, title: '{}'",
        files.len(),
        title
    );

    // First, check for explicit tier markers in title (highest priority)
    let title_lower = title.to_lowercase();
    if title_lower.contains("[consensus-adjacent]") {
        // Explicit tier 3 marker - return immediately
        return TierClassificationResult {
            tier: 3,
            confidence: 1.0,
            matched_patterns: vec![],
            matched_keywords: vec!["[CONSENSUS-ADJACENT]".to_string()],
            rationale: "Explicit [CONSENSUS-ADJACENT] marker in title".to_string(),
        };
    }
    if title_lower.contains("[governance]") {
        return TierClassificationResult {
            tier: 5,
            confidence: 1.0,
            matched_patterns: vec![],
            matched_keywords: vec!["[GOVERNANCE]".to_string()],
            rationale: "Explicit [GOVERNANCE] marker in title".to_string(),
        };
    }
    if title_lower.contains("[emergency]") || title_lower.contains("emergency:") {
        return TierClassificationResult {
            tier: 4,
            confidence: 1.0,
            matched_patterns: vec![],
            matched_keywords: vec!["[EMERGENCY]".to_string()],
            rationale: "Explicit [EMERGENCY] marker in title".to_string(),
        };
    }

    let mut best_tier = config.fallback.default_tier;
    let mut best_confidence = 0.0;
    let mut matched_patterns = Vec::new();
    let mut matched_keywords = Vec::new();
    let mut rationale = String::new();

    // Collect all tier rules and sort by tier number (descending) to check higher tiers first
    let mut tier_rules: Vec<(u32, String, &TierRule)> = config
        .classification_rules
        .iter()
        .map(|(name, rule)| {
            // Extract tier number from name like "tier_1_routine" or "tier_5_governance"
            let tier_num = if name.starts_with("tier_") {
                name.split('_')
                    .nth(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1)
            } else {
                // Fallback: try to parse from last segment
                name.split('_')
                    .next_back()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1)
            };
            (tier_num, name.clone(), rule)
        })
        .collect();
    tier_rules.sort_by(|a, b| b.0.cmp(&a.0)); // Sort descending (5, 4, 3, 2, 1)

    // Check each tier rule (now in descending order)
    for (tier_num, _tier_name, rule) in tier_rules {
        let mut confidence = 0.0;
        let mut tier_patterns = Vec::new();
        let mut tier_keywords = Vec::new();

        // Check file patterns
        for pattern in &rule.file_patterns {
            for file in &files {
                if matches_pattern(file, pattern) {
                    confidence += config.confidence_scoring.file_pattern_match;
                    tier_patterns.push(format!("{pattern}:{file}"));
                }
            }
        }

        // Check keywords in title and body
        // Keywords are weighted more heavily when no file patterns match
        let has_file_matches = !tier_patterns.is_empty();
        let keyword_weight_multiplier = if has_file_matches {
            1.0 // Normal weight when files match
        } else {
            1.5 // Boost keyword weight when no files match (keyword-only classification)
        };

        for keyword in &rule.keywords {
            let keyword_lower = keyword.to_lowercase();
            let title_lower = title.to_lowercase();
            let body_lower = body.to_lowercase();

            let title_match = title_lower.contains(&keyword_lower);
            let body_match = body_lower.contains(&keyword_lower);

            if title_match {
                // Title matches are very strong signals - give them more weight
                // Use keyword_weight_multiplier for consistency with body matches
                confidence += config.confidence_scoring.keyword_match
                    * config.confidence_scoring.title_analysis
                    * keyword_weight_multiplier
                    * 2.0; // Additional boost for title matches
                tier_keywords.push(format!("title:{keyword}"));
            }
            if body_match {
                confidence += config.confidence_scoring.keyword_match
                    * config.confidence_scoring.description_analysis
                    * keyword_weight_multiplier;
                tier_keywords.push(format!("body:{keyword}"));
            }
        }

        // Special handling for tier markers in title (e.g., [CONSENSUS-ADJACENT], [GOVERNANCE], [EMERGENCY])
        // These are very strong signals and should override other matches
        let title_lower = title.to_lowercase();
        if title_lower.contains("[consensus-adjacent]") && tier_num == 3 {
            confidence = 1.0; // Very strong signal - set to max confidence
        }
        if title_lower.contains("[governance]") && tier_num == 5 {
            confidence = 1.0;
        }
        if (title_lower.contains("[emergency]") || title_lower.contains("emergency:"))
            && tier_num == 4
        {
            confidence = 1.0;
        }

        // Check for exclusions
        if let Some(exclude_patterns) = &rule.exclude_patterns {
            for pattern in exclude_patterns {
                for file in &files {
                    if matches_pattern(file, pattern) {
                        confidence += config
                            .confidence_scoring
                            .penalty_factors
                            .conflicting_indicators;
                        break;
                    }
                }
            }
        }

        // Apply boost factors
        if tier_patterns.len() > 1 {
            confidence += config
                .confidence_scoring
                .boost_factors
                .multiple_file_matches;
        }
        if tier_keywords.len() > 2 {
            confidence += config
                .confidence_scoring
                .boost_factors
                .strong_keyword_matches;
        }

        // Boost confidence if multiple strong indicators present
        if !tier_patterns.is_empty() && !tier_keywords.is_empty() {
            confidence += 0.1; // Bonus for both file and keyword matches
        }

        debug!(
            "Tier {}: confidence={:.2}, patterns={:?}, keywords={:?}",
            tier_num, confidence, tier_patterns, tier_keywords
        );

        // Update if this tier matches and either:
        // 1. This is the first match (best_confidence == 0.0), OR
        // 2. Confidence is significantly higher (more than 0.1 difference)
        // Since we iterate in descending order, higher tiers are checked first,
        // so we only override with lower tiers if they have significantly higher confidence
        // For governance (tier 5), we want to prioritize it even with lower confidence
        let should_update = if confidence >= rule.confidence_threshold {
            best_confidence == 0.0
                || confidence > best_confidence + 0.1
                || (tier_num == 5 && confidence > 0.0)
        } else {
            false
        };

        if should_update {
            best_tier = tier_num;
            best_confidence = confidence;
            matched_patterns = tier_patterns;
            matched_keywords = tier_keywords;
            rationale = format!(
                "Matched {} rule with confidence {:.2}",
                rule.name, confidence
            );
        }
    }

    // Check if confidence meets fallback threshold
    // Don't override tier 5 (governance) with fallback if it has any confidence
    if best_confidence < config.fallback.confidence_threshold && best_tier != 5 {
        best_tier = config.fallback.default_tier;
        rationale = format!(
            "Confidence {:.2} below threshold {:.2}, using fallback Tier {}",
            best_confidence, config.fallback.confidence_threshold, best_tier
        );
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
    rules.insert(
        "tier_5_governance".to_string(),
        TierRule {
            name: "Governance Changes".to_string(),
            confidence_threshold: 0.2, // Lower threshold to catch governance keywords
            file_patterns: vec![
                "governance/**".to_string(),
                "maintainers/**".to_string(),
                "**/action-tiers.yml".to_string(),
            ],
            keywords: vec![
                "governance".to_string(),
                "maintainer".to_string(),
                "signature".to_string(),
                "threshold".to_string(),
                "[governance]".to_string(),
            ],
            exclude_patterns: None,
            require_specification: Some(false),
            require_audit: Some(false),
            require_equivalence_proof: Some(false),
            require_post_mortem: Some(false),
            require_public_comment: Some(true),
            require_rationale: Some(true),
            examples: vec!["Change signature thresholds".to_string()],
        },
    );

    // Tier 4: Emergency
    rules.insert(
        "tier_4_emergency".to_string(),
        TierRule {
            name: "Emergency Actions".to_string(),
            confidence_threshold: 0.5, // Lower threshold for emergency detection
            file_patterns: vec![],
            keywords: vec![
                "emergency".to_string(),
                "critical".to_string(),
                "security".to_string(),
                "vulnerability".to_string(),
                "CVE".to_string(),
                "[emergency]".to_string(),
                "emergency:".to_string(),
            ],
            exclude_patterns: None,
            require_specification: Some(false),
            require_audit: Some(false),
            require_equivalence_proof: Some(false),
            require_post_mortem: Some(true),
            require_public_comment: Some(false),
            require_rationale: Some(false),
            examples: vec!["Fix critical security vulnerability".to_string()],
        },
    );

    // Tier 3: Consensus-Adjacent
    rules.insert(
        "tier_3_consensus_adjacent".to_string(),
        TierRule {
            name: "Consensus-Adjacent Changes".to_string(),
            confidence_threshold: 0.5, // Lower threshold to catch explicit markers
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
                "[consensus-adjacent]".to_string(),
            ],
            exclude_patterns: None,
            require_specification: Some(true),
            require_audit: Some(true),
            require_equivalence_proof: Some(true),
            require_post_mortem: Some(false),
            require_public_comment: Some(false),
            require_rationale: Some(false),
            examples: vec!["Change block validation logic".to_string()],
        },
    );

    // Tier 2: Features
    rules.insert(
        "tier_2_features".to_string(),
        TierRule {
            name: "Feature Changes".to_string(),
            confidence_threshold: 0.3, // Lower threshold to catch feature keywords
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
                "addition".to_string(), // Match "Feature addition" in test
            ],
            exclude_patterns: None,
            require_specification: Some(true),
            require_audit: Some(false),
            require_equivalence_proof: Some(false),
            require_post_mortem: Some(false),
            require_public_comment: Some(false),
            require_rationale: Some(false),
            examples: vec!["Add new RPC method".to_string()],
        },
    );

    // Tier 1: Routine (default)
    rules.insert(
        "tier_1_routine".to_string(),
        TierRule {
            name: "Routine Maintenance".to_string(),
            confidence_threshold: 0.3, // Lower threshold for routine maintenance
            file_patterns: vec![
                "docs/**".to_string(),
                "tests/**".to_string(),
                "*.md".to_string(),
                "README*".to_string(),
            ],
            keywords: vec![
                "fix".to_string(),
                "bug".to_string(),
                "typo".to_string(),
                "documentation".to_string(),
                "readme".to_string(),
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
        },
    );

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
            permissions: vec![
                "maintainers".to_string(),
                "emergency-keyholders".to_string(),
            ],
            logging: LoggingConfig {
                required: true,
                fields: vec![
                    "user".to_string(),
                    "timestamp".to_string(),
                    "reason".to_string(),
                ],
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
            default_tier: 1, // Changed from 2 to 1 (routine maintenance is safer default)
            confidence_threshold: 0.3, // Lowered from 0.5 to allow keyword-only classification
            require_manual_review: true,
            notification: vec!["maintainers".to_string(), "pr-author".to_string()],
        },
    }
}

/// Check if a file matches a glob pattern
fn matches_pattern(file: &str, pattern: &str) -> bool {
    // Simple glob matching - handles common cases
    if pattern.contains("**") {
        // Handle **/pattern/** case (match anywhere in path)
        if pattern.starts_with("**/") && pattern.ends_with("/**") {
            // Extract the pattern between **/ and /**
            let pattern_clean = pattern
                .strip_prefix("**/")
                .unwrap_or(pattern)
                .strip_suffix("/**")
                .unwrap_or(pattern);
            // Match if file path contains /pattern/
            return file.contains(&format!("/{pattern_clean}/"))
                || file.ends_with(&format!("/{pattern_clean}"))
                || file.starts_with(&format!("{pattern_clean}/"));
        }

        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];

            // Handle **/pattern case (match anywhere in path)
            if prefix.is_empty() && suffix.starts_with('/') {
                // For **/pattern, match if file path ends with pattern or contains /pattern
                let suffix_clean = &suffix[1..];
                return file.ends_with(suffix_clean) || file.contains(&format!("/{suffix_clean}"));
            }

            // Handle **/pattern case (match anywhere in path, no leading /)
            if prefix.is_empty() {
                return file.contains(suffix);
            }
            // Handle pattern/** case
            if suffix.is_empty() {
                return file.starts_with(prefix);
            }
            // Handle pattern/**/suffix case
            // Match if file starts with prefix and ends with suffix
            if file.starts_with(prefix) {
                // Check if suffix appears after prefix
                if let Some(rest) = file.strip_prefix(prefix) {
                    return rest.ends_with(suffix)
                        || rest.contains(&format!("/{}", suffix.trim_start_matches('/')));
                }
            }
            return false;
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
    if !(1..=5).contains(&tier) {
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
        // Should detect emergency tier based on keywords
        assert_eq!(
            result.tier, 4,
            "Emergency keywords should classify as Tier 4"
        );
        assert!(result.confidence > 0.3, "Should have reasonable confidence");
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
        // Should detect governance tier based on keywords
        assert_eq!(
            result.tier, 5,
            "Governance keywords should classify as Tier 5"
        );
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
        // Should detect consensus-adjacent tier based on keywords
        assert_eq!(
            result.tier, 3,
            "Consensus keywords should classify as Tier 3"
        );
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
        // Should detect routine tier based on keywords
        assert_eq!(result.tier, 1, "Routine keywords should classify as Tier 1");
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("docs/README.md", "docs/**"));
        assert!(matches_pattern("src/rpc/server.rs", "**/rpc/**"));
        assert!(matches_pattern(
            "governance/config/action-tiers.yml",
            "**/action-tiers.yml"
        ));
        assert!(!matches_pattern("src/consensus/validation.rs", "docs/**"));

        // Test various pattern cases
        assert!(matches_pattern("src/rpc/server.rs", "**/rpc/**"));
        assert!(matches_pattern("src/rpc/server.rs", "src/**"));
        assert!(matches_pattern(
            "governance/config/action-tiers.yml",
            "**/action-tiers.yml"
        ));
        assert!(matches_pattern("any/path/to/rpc/server.rs", "**/rpc/**"));
    }
}
