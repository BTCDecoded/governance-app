//! Governance Fork Types and Data Structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Governance ruleset with versioning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub id: String,
    pub name: String,
    pub version: RulesetVersion,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub config: serde_json::Value,
    pub description: Option<String>,
}

/// Semantic version for governance rulesets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesetVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
    pub build_metadata: Option<String>,
}

impl RulesetVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build_metadata: None,
        }
    }

    pub fn to_string(&self) -> String {
        let mut version = format!("{}.{}.{}", self.major, self.minor, self.patch);

        if let Some(pre) = &self.pre_release {
            version.push('-');
            version.push_str(pre);
        }

        if let Some(build) = &self.build_metadata {
            version.push('+');
            version.push_str(build);
        }

        version
    }

    pub fn from_string(version_str: &str) -> Result<Self, String> {
        // Simple semantic version parsing
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid version format".to_string());
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| "Invalid major version")?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| "Invalid minor version")?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| "Invalid patch version")?;

        Ok(Self::new(major, minor, patch))
    }
}

/// Adoption metrics for a governance ruleset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptionMetrics {
    pub ruleset_id: String,
    pub node_count: u32,
    pub hashpower_percentage: f64,
    pub economic_activity_percentage: f64,
    pub total_weight: f64,
    pub last_updated: DateTime<Utc>,
}

/// Complete adoption statistics across all rulesets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdoptionStatistics {
    pub total_nodes: u32,
    pub total_hashpower: f64,
    pub total_economic_activity: f64,
    pub rulesets: Vec<AdoptionMetrics>,
    pub winning_ruleset: Option<String>,
    pub adoption_percentage: f64,
    pub last_updated: DateTime<Utc>,
}

/// Governance configuration export format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceExport {
    pub version: String,
    pub ruleset_id: String,
    pub ruleset_version: RulesetVersion,
    pub created_at: DateTime<Utc>,
    pub action_tiers: serde_json::Value,
    pub economic_nodes: serde_json::Value,
    pub maintainers: serde_json::Value,
    pub repositories: serde_json::Value,
    pub governance_fork: serde_json::Value,
    pub metadata: ExportMetadata,
}

/// Export metadata and provenance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub exported_by: String,
    pub source_repository: String,
    pub commit_hash: String,
    pub export_tool_version: String,
    pub signature: Option<String>,
    pub verification_url: Option<String>,
}

/// Fork decision for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkDecision {
    pub node_id: String,
    pub node_type: String,
    pub chosen_ruleset: String,
    pub decision_reason: String,
    pub weight: f64,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// Governance fork event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkEvent {
    pub event_id: String,
    pub event_type: ForkEventType,
    pub ruleset_id: String,
    pub node_id: String,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Types of governance fork events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkEventType {
    RulesetCreated,
    RulesetAdopted,
    RulesetAbandoned,
    ForkDecision,
    AdoptionThresholdMet,
    GovernanceFork,
}

impl ForkEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForkEventType::RulesetCreated => "ruleset_created",
            ForkEventType::RulesetAdopted => "ruleset_adopted",
            ForkEventType::RulesetAbandoned => "ruleset_abandoned",
            ForkEventType::ForkDecision => "fork_decision",
            ForkEventType::AdoptionThresholdMet => "adoption_threshold_met",
            ForkEventType::GovernanceFork => "governance_fork",
        }
    }
}

/// Configuration for governance fork thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkThresholds {
    pub minimum_adoption_percentage: f64,
    pub minimum_node_count: u32,
    pub minimum_hashpower_percentage: f64,
    pub minimum_economic_activity_percentage: f64,
    pub grace_period_days: u32,
}

impl Default for ForkThresholds {
    fn default() -> Self {
        Self {
            minimum_adoption_percentage: 50.0,
            minimum_node_count: 10,
            minimum_hashpower_percentage: 30.0,
            minimum_economic_activity_percentage: 40.0,
            grace_period_days: 30,
        }
    }
}




