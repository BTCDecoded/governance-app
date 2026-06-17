//! Security Control Validator
//!
//! Automatically classifies PRs based on affected security controls and determines
//! required governance tier. This embeds security controls directly into the governance
//! system, making it self-enforcing.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Security control mapping loaded from config
#[derive(Debug, Clone, Deserialize)]
pub struct SecurityControlMapping {
    pub security_controls: Vec<SecurityControl>,
    pub categories: HashMap<String, ControlCategory>,
    pub priorities: HashMap<String, Priority>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityControl {
    pub id: String,
    pub name: String,
    pub category: String,
    pub priority: String,
    pub description: String,
    pub files: Vec<String>,
    pub required_signatures: String,
    pub review_period_days: u32,
    pub requires_security_audit: bool,
    pub requires_formal_verification: bool,
    pub requires_cryptography_expert: bool,
    pub additional_requirements: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlCategory {
    pub name: String,
    pub description: String,
    pub max_priority: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Priority {
    pub name: String,
    pub description: String,
    pub color: String,
}

/// Security impact analysis result
#[derive(Debug, Clone, Serialize)]
pub struct SecurityImpact {
    pub impact_level: ImpactLevel,
    pub affected_controls: Vec<AffectedControl>,
    pub required_tier: Option<String>,
    pub additional_requirements: Vec<String>,
    pub blocks_production: bool,
    pub blocks_audit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum ImpactLevel {
    None,
    Low,      // P2 controls
    Medium,   // P1 controls
    High,     // P0 controls
    Critical, // Multiple P0 controls
}

#[derive(Debug, Clone, Serialize)]
pub struct AffectedControl {
    pub id: String,
    pub name: String,
    pub category: String,
    pub priority: String,
    pub required_signatures: String,
    pub review_period_days: u32,
    pub requires_security_audit: bool,
    pub requires_formal_verification: bool,
    pub requires_cryptography_expert: bool,
}

/// Security control validator
pub struct SecurityControlValidator {
    mapping: SecurityControlMapping,
}

impl SecurityControlValidator {
    /// Create a new validator from config file
    pub fn new(config_path: &str) -> Result<Self> {
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| anyhow!("Failed to read security control mapping: {}", e))?;

        let mapping: SecurityControlMapping = serde_yaml::from_str(&config_content)
            .map_err(|e| anyhow!("Failed to parse security control mapping: {}", e))?;

        info!(
            "Loaded {} security controls from config",
            mapping.security_controls.len()
        );

        Ok(Self { mapping })
    }

    /// Analyze security impact of changed files
    pub fn analyze_security_impact(&self, changed_files: &[String]) -> Result<SecurityImpact> {
        debug!(
            "Analyzing security impact for {} changed files",
            changed_files.len()
        );

        let mut affected_controls = Vec::new();
        let mut additional_requirements = Vec::new();
        let mut max_priority = "P3".to_string();
        let mut blocks_production = false;
        let mut blocks_audit = false;

        // Check each changed file against security controls
        for file in changed_files {
            debug!("Checking file: {}", file);

            for control in &self.mapping.security_controls {
                if self.file_matches_control(file, control)? {
                    debug!("File {} matches control {}", file, control.id);

                    let affected_control = AffectedControl {
                        id: control.id.clone(),
                        name: control.name.clone(),
                        category: control.category.clone(),
                        priority: control.priority.clone(),
                        required_signatures: control.required_signatures.clone(),
                        review_period_days: control.review_period_days,
                        requires_security_audit: control.requires_security_audit,
                        requires_formal_verification: control.requires_formal_verification,
                        requires_cryptography_expert: control.requires_cryptography_expert,
                    };

                    affected_controls.push(affected_control);

                    // Track highest priority (lower number = higher priority: P0=0, P1=1, etc.)
                    if self.priority_level(&control.priority) < self.priority_level(&max_priority) {
                        max_priority = control.priority.clone();
                    }

                    // Check if this blocks production or audit
                    if control.priority == "P0" {
                        blocks_production = true;
                        blocks_audit = true;
                    }

                    // Collect additional requirements
                    if let Some(reqs) = &control.additional_requirements {
                        additional_requirements.extend(reqs.clone());
                    }
                }
            }
        }

        // Determine impact level and required tier
        let impact_level = self.determine_impact_level(&affected_controls, &max_priority);
        let required_tier = self.determine_required_tier(&impact_level, &affected_controls);

        // Add tier-specific requirements
        if let Some(tier) = &required_tier {
            additional_requirements.extend(self.get_tier_requirements(tier));
        }

        let result = SecurityImpact {
            impact_level,
            affected_controls,
            required_tier,
            additional_requirements,
            blocks_production,
            blocks_audit,
        };

        info!(
            "Security impact analysis complete: {:?}",
            result.impact_level
        );
        Ok(result)
    }

    /// Check if a file matches a security control pattern
    fn file_matches_control(&self, file: &str, control: &SecurityControl) -> Result<bool> {
        for pattern in &control.files {
            if self.matches_pattern(file, pattern)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Simple glob pattern matching
    fn matches_pattern(&self, file: &str, pattern: &str) -> Result<bool> {
        // Handle ** glob patterns (recursive directory matching)
        if pattern.contains("**") {
            // Convert ** pattern to proper matching
            // e.g., "blvm-protocol/**/*.rs" should match "blvm-protocol/src/lib.rs"
            let parts: Vec<&str> = pattern.split("**").collect();
            if parts.len() == 2 {
                let prefix = parts[0].trim_end_matches('/');
                let suffix = parts[1].trim_start_matches('/');

                // Normalize: ensure prefix comparison works
                let normalized_prefix = if prefix.ends_with('/') {
                    &prefix[..prefix.len() - 1]
                } else {
                    prefix
                };

                // Check if file contains normalized prefix (works with absolute paths)
                // For absolute paths, check if any path segment matches
                if !file.contains(normalized_prefix) {
                    return Ok(false);
                }

                // Find the position where prefix appears in file
                let prefix_pos = file.find(normalized_prefix).unwrap_or(0);

                // Get remaining part after prefix (skip the / if present)
                let remaining_start = if file.len() > prefix_pos + normalized_prefix.len() {
                    let next_char_pos = prefix_pos + normalized_prefix.len();
                    let next_char = file.chars().nth(next_char_pos);
                    if next_char == Some('/') {
                        next_char_pos + 1
                    } else {
                        next_char_pos
                    }
                } else {
                    prefix_pos + normalized_prefix.len()
                };

                let remaining = if file.len() > remaining_start {
                    &file[remaining_start..]
                } else {
                    ""
                };

                // For suffix like "/*.rs", we need to match any path ending with .rs
                // The pattern "blvm-protocol/**/*.rs" means: prefix + any dirs + / + *.rs
                if suffix.starts_with("/*") {
                    // Pattern like "/*.rs" - match any file ending with .rs
                    let file_extension = suffix.strip_prefix("/*").unwrap_or(suffix);
                    // ** matches any directories, so remaining just needs to end with extension
                    // Also handle case where suffix is "*.rs" (without leading /)
                    if remaining.ends_with(file_extension) {
                        return Ok(true);
                    }
                    // Try without the leading / in suffix
                    if suffix.starts_with("*") {
                        let alt_extension = suffix.strip_prefix("*").unwrap_or(suffix);
                        if remaining.ends_with(alt_extension) {
                            return Ok(true);
                        }
                    }
                } else if suffix.starts_with("*") {
                    // Pattern like "*.rs" - match any file ending with .rs
                    let file_extension = suffix.strip_prefix("*").unwrap_or(suffix);
                    if remaining.ends_with(file_extension) {
                        return Ok(true);
                    }
                } else if suffix.is_empty() {
                    // Pattern like "**" at the end - matches everything after prefix
                    return Ok(true);
                } else {
                    // Simple suffix match - check if remaining ends with suffix
                    // For patterns like "**/file.rs", suffix is "/file.rs"
                    if remaining.ends_with(suffix) || remaining == suffix.trim_start_matches('/') {
                        return Ok(true);
                    }
                }
            }
            // Fallback to simple replacement
            let pattern = pattern.replace("**", "*");
            return Ok(self.simple_glob_match(file, &pattern));
        }

        // Handle * glob patterns
        if pattern.contains('*') {
            return Ok(self.simple_glob_match(file, pattern));
        }

        // Exact match
        Ok(file == pattern)
    }

    /// Simple glob matching (supports * but not complex patterns)
    fn simple_glob_match(&self, file: &str, pattern: &str) -> bool {
        // Convert glob pattern to regex-like matching
        let pattern_parts: Vec<&str> = pattern.split('*').collect();
        let _file_parts: Vec<&str> = file.split('/').collect();

        if pattern_parts.len() == 1 {
            // No wildcards, exact match
            return file == pattern;
        }

        // Simple prefix/suffix matching
        if pattern.starts_with('*') && pattern.ends_with('*') {
            let inner = &pattern[1..pattern.len() - 1];
            return file.contains(inner);
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return file.starts_with(prefix);
        }

        if let Some(suffix) = pattern.strip_prefix('*') {
            return file.ends_with(suffix);
        }

        // Fallback to contains
        file.contains(pattern)
    }

    /// Determine impact level based on affected controls
    fn determine_impact_level(
        &self,
        controls: &[AffectedControl],
        max_priority: &str,
    ) -> ImpactLevel {
        if controls.is_empty() {
            return ImpactLevel::None;
        }

        let p0_count = controls.iter().filter(|c| c.priority == "P0").count();
        let p1_count = controls.iter().filter(|c| c.priority == "P1").count();

        match max_priority {
            "P0" if p0_count > 1 => ImpactLevel::Critical,
            "P0" => ImpactLevel::High,
            "P1" if p1_count > 1 => ImpactLevel::High,
            "P1" => ImpactLevel::Medium,
            "P2" => ImpactLevel::Low,
            _ => ImpactLevel::None,
        }
    }

    /// Determine required governance tier based on impact
    fn determine_required_tier(
        &self,
        impact_level: &ImpactLevel,
        controls: &[AffectedControl],
    ) -> Option<String> {
        match impact_level {
            ImpactLevel::Critical | ImpactLevel::High => {
                // Check if any P0 controls require cryptography expert
                let needs_crypto_expert = controls
                    .iter()
                    .any(|c| c.priority == "P0" && c.requires_cryptography_expert);

                if needs_crypto_expert {
                    Some("security_critical".to_string())
                } else {
                    Some("security_critical".to_string())
                }
            }
            ImpactLevel::Medium => {
                // Check if any controls require cryptography expert
                let needs_crypto_expert = controls.iter().any(|c| c.requires_cryptography_expert);

                if needs_crypto_expert {
                    Some("cryptographic".to_string())
                } else {
                    Some("security_enhancement".to_string())
                }
            }
            ImpactLevel::Low => Some("security_enhancement".to_string()),
            ImpactLevel::None => None,
        }
    }

    /// Get additional requirements for a tier
    fn get_tier_requirements(&self, tier: &str) -> Vec<String> {
        match tier {
            "security_critical" => vec![
                "All affected P0 controls must be certified".to_string(),
                "No placeholder implementations in diff".to_string(),
                "Formal verification proofs passing".to_string(),
                "Security audit report attached to PR".to_string(),
                "Cryptographer approval required".to_string(),
            ],
            "cryptographic" => vec![
                "Cryptographer approval required".to_string(),
                "Test vectors from standard specifications".to_string(),
                "Side-channel analysis performed".to_string(),
                "Formal verification proofs passing".to_string(),
            ],
            "security_enhancement" => vec![
                "Security review by maintainer".to_string(),
                "Comprehensive test coverage".to_string(),
                "No placeholder implementations".to_string(),
            ],
            _ => vec![],
        }
    }

    /// Get priority level as numeric value
    fn priority_level(&self, priority: &str) -> u32 {
        match priority {
            "P0" => 0,
            "P1" => 1,
            "P2" => 2,
            "P3" => 3,
            _ => 4,
        }
    }

    /// Check if PR should be blocked due to placeholder implementations
    pub fn check_for_placeholders(
        &self,
        changed_files: &[String],
    ) -> Result<Vec<PlaceholderViolation>> {
        let mut violations = Vec::new();

        for file in changed_files {
            if self.is_security_critical_file(file)? {
                if let Ok(content) = std::fs::read_to_string(file) {
                    let violations_in_file = self.find_placeholders_in_content(file, &content);
                    violations.extend(violations_in_file);
                }
            }
        }

        Ok(violations)
    }

    /// Check if file is security-critical
    fn is_security_critical_file(&self, file: &str) -> Result<bool> {
        for control in &self.mapping.security_controls {
            if self.file_matches_control(file, control)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Find placeholder patterns in file content
    fn find_placeholders_in_content(&self, file: &str, content: &str) -> Vec<PlaceholderViolation> {
        let mut violations = Vec::new();

        let placeholder_patterns = [
            "PLACEHOLDER",
            "TODO: Implement",
            "0x00[PLACEHOLDER",
            "0x02[PLACEHOLDER",
            "0x03[PLACEHOLDER",
            "0x04[PLACEHOLDER",
            "return None as a placeholder",
            "return vec![] as a placeholder",
            "return empty vector as a placeholder",
            "For now, return",
            "This is a placeholder",
        ];

        for (line_num, line) in content.lines().enumerate() {
            for pattern in &placeholder_patterns {
                if line.contains(pattern) {
                    violations.push(PlaceholderViolation {
                        file: file.to_string(),
                        line: line_num + 1,
                        pattern: pattern.to_string(),
                        content: line.trim().to_string(),
                    });
                }
            }
        }

        violations
    }

    /// Generate security impact summary for PR comments
    pub fn generate_pr_comment(&self, impact: &SecurityImpact) -> String {
        if impact.affected_controls.is_empty() {
            return "## Security Control Impact\n\n✅ **No security controls affected** - Standard review process applies.".to_string();
        }

        let mut comment = String::new();
        comment.push_str("## Security Control Impact\n\n");

        // Impact level
        match impact.impact_level {
            ImpactLevel::Critical => comment
                .push_str("🚨 **CRITICAL IMPACT** - Multiple P0 security controls affected\n\n"),
            ImpactLevel::High => {
                comment.push_str("🔴 **HIGH IMPACT** - P0 security controls affected\n\n")
            }
            ImpactLevel::Medium => {
                comment.push_str("🟡 **MEDIUM IMPACT** - P1 security controls affected\n\n")
            }
            ImpactLevel::Low => {
                comment.push_str("🟢 **LOW IMPACT** - P2 security controls affected\n\n")
            }
            ImpactLevel::None => {
                comment.push_str("✅ **NO IMPACT** - No security controls affected\n\n")
            }
        }

        // Affected controls
        comment.push_str("### Affected Security Controls\n\n");
        for control in &impact.affected_controls {
            let priority_emoji = match control.priority.as_str() {
                "P0" => "🔴",
                "P1" => "🟡",
                "P2" => "🟢",
                _ => "⚪",
            };

            comment.push_str(&format!(
                "- {} **{}** ({})\n",
                priority_emoji, control.name, control.id
            ));
        }

        // Required tier
        if let Some(tier) = &impact.required_tier {
            comment.push_str(&format!("\n### Required Governance Tier: **{tier}**\n\n"));
        }

        // Additional requirements
        if !impact.additional_requirements.is_empty() {
            comment.push_str("### Additional Requirements\n\n");
            for req in &impact.additional_requirements {
                comment.push_str(&format!("- {req}\n"));
            }
        }

        // Production/audit blocking
        if impact.blocks_production {
            comment.push_str(
                "\n⚠️ **This PR blocks production deployment** until P0 controls are certified.\n",
            );
        }

        if impact.blocks_audit {
            comment.push_str(
                "\n⚠️ **This PR blocks security audit** until P0 controls are implemented.\n",
            );
        }

        comment
    }
}

/// Placeholder violation found in code
#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderViolation {
    pub file: String,
    pub line: usize,
    pub pattern: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_pattern_matching() {
        let validator = create_test_validator();

        // Test exact match
        assert!(
            validator
                .matches_pattern("blvm-protocol/src/lib.rs", "blvm-protocol/src/lib.rs")
                .unwrap()
        );

        // Test wildcard match
        assert!(
            validator
                .matches_pattern("blvm-protocol/src/lib.rs", "blvm-protocol/**/*.rs")
                .unwrap()
        );

        // Test no match
        assert!(
            !validator
                .matches_pattern("other/file.rs", "blvm-protocol/**/*.rs")
                .unwrap()
        );
    }

    #[test]
    fn test_security_impact_analysis() {
        let validator = create_test_validator();

        // Test P0 control impact
        let changed_files = vec!["blvm-protocol/src/lib.rs".to_string()];
        let impact = validator.analyze_security_impact(&changed_files).unwrap();

        assert!(matches!(impact.impact_level, ImpactLevel::High));
        assert!(impact.blocks_production);
        assert!(impact.blocks_audit);
        assert_eq!(impact.required_tier, Some("security_critical".to_string()));
    }

    #[test]
    fn test_placeholder_detection() {
        let validator = create_test_validator();

        // Create a temporary file with placeholder that matches the security control pattern
        // Use a relative path that matches the pattern "blvm-protocol/**/*.rs"
        let temp_dir = tempfile::tempdir().unwrap();
        let test_dir = temp_dir.path().join("blvm-protocol").join("src");
        std::fs::create_dir_all(&test_dir).unwrap();
        let temp_file = test_dir.join("test_security.rs");
        std::fs::write(&temp_file, "// TODO: Implement actual cryptographic verification\nlet key = 0x02[PLACEHOLDER_64_CHAR_HEX];").unwrap();

        // Use the file path relative to temp_dir to match the pattern
        let file_path = temp_file.to_string_lossy().to_string();
        let violations = validator.check_for_placeholders(&[file_path]).unwrap();

        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.pattern == "TODO: Implement"));
        assert!(violations.iter().any(|v| v.pattern == "0x02[PLACEHOLDER"));
    }

    fn create_test_validator() -> SecurityControlValidator {
        let mapping = SecurityControlMapping {
            security_controls: vec![SecurityControl {
                id: "A-001".to_string(),
                name: "Genesis Block Implementation".to_string(),
                category: "consensus_integrity".to_string(),
                priority: "P0".to_string(),
                description: "Proper genesis blocks".to_string(),
                files: vec!["blvm-protocol/**/*.rs".to_string()],
                required_signatures: "7-of-7".to_string(),
                review_period_days: 180,
                requires_security_audit: true,
                requires_formal_verification: true,
                requires_cryptography_expert: false,
                additional_requirements: None,
            }],
            categories: HashMap::new(),
            priorities: HashMap::new(),
        };

        SecurityControlValidator { mapping }
    }
}
