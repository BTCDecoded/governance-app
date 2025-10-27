//! Governance Ruleset Versioning
//!
//! Handles semantic versioning and cryptographic hashing for governance rulesets

use chrono::Utc;
use sha2::{Digest, Sha256};

use super::types::*;
use crate::error::GovernanceError;

pub struct RulesetVersioning;

impl RulesetVersioning {
    /// Generate semantic version for a ruleset based on changes
    pub fn version_ruleset(
        &self,
        current_version: Option<&RulesetVersion>,
        change_type: VersionChangeType,
    ) -> Result<RulesetVersion, GovernanceError> {
        let version = match current_version {
            Some(v) => self.increment_version(v, change_type)?,
            None => RulesetVersion::new(1, 0, 0),
        };

        Ok(version)
    }

    /// Generate cryptographic hash for a ruleset
    pub fn generate_ruleset_hash(
        &self,
        ruleset_id: &str,
        version: &RulesetVersion,
        config: &serde_json::Value,
    ) -> Result<String, GovernanceError> {
        let version_string = version.to_string();
        let config_string = serde_json::to_string(config).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to serialize config: {}", e))
        })?;

        let mut hasher = Sha256::new();
        hasher.update(ruleset_id.as_bytes());
        hasher.update(version_string.as_bytes());
        hasher.update(config_string.as_bytes());
        hasher.update(Utc::now().to_rfc3339().as_bytes());

        let hash = hasher.finalize();
        Ok(hex::encode(hash))
    }

    /// Create a new ruleset with versioning
    pub fn create_ruleset(
        &self,
        id: &str,
        name: &str,
        config: serde_json::Value,
        description: Option<&str>,
    ) -> Result<Ruleset, GovernanceError> {
        let version = RulesetVersion::new(1, 0, 0);
        let hash = self.generate_ruleset_hash(id, &version, &config)?;

        Ok(Ruleset {
            id: id.to_string(),
            name: name.to_string(),
            version,
            hash,
            created_at: Utc::now(),
            config,
            description: description.map(|s| s.to_string()),
        })
    }

    /// Update an existing ruleset with new version
    pub fn update_ruleset(
        &self,
        mut ruleset: Ruleset,
        new_config: serde_json::Value,
        change_type: VersionChangeType,
    ) -> Result<Ruleset, GovernanceError> {
        // Update version
        ruleset.version = self.increment_version(&ruleset.version, change_type)?;

        // Update hash
        ruleset.hash = self.generate_ruleset_hash(&ruleset.id, &ruleset.version, &new_config)?;

        // Update config
        ruleset.config = new_config;

        Ok(ruleset)
    }

    /// Compare two ruleset versions
    pub fn compare_versions(&self, v1: &RulesetVersion, v2: &RulesetVersion) -> VersionComparison {
        match v1.major.cmp(&v2.major) {
            std::cmp::Ordering::Equal => match v1.minor.cmp(&v2.minor) {
                std::cmp::Ordering::Equal => match v1.patch.cmp(&v2.patch) {
                    std::cmp::Ordering::Equal => VersionComparison::Equal,
                    std::cmp::Ordering::Less => VersionComparison::Older,
                    std::cmp::Ordering::Greater => VersionComparison::Newer,
                },
                std::cmp::Ordering::Less => VersionComparison::Older,
                std::cmp::Ordering::Greater => VersionComparison::Newer,
            },
            std::cmp::Ordering::Less => VersionComparison::Older,
            std::cmp::Ordering::Greater => VersionComparison::Newer,
        }
    }

    /// Check if a version is compatible with another
    pub fn is_compatible(&self, version1: &RulesetVersion, version2: &RulesetVersion) -> bool {
        // Same major version means compatible
        version1.major == version2.major
    }

    /// Increment version based on change type
    fn increment_version(
        &self,
        current: &RulesetVersion,
        change_type: VersionChangeType,
    ) -> Result<RulesetVersion, GovernanceError> {
        match change_type {
            VersionChangeType::Major => Ok(RulesetVersion::new(current.major + 1, 0, 0)),
            VersionChangeType::Minor => {
                Ok(RulesetVersion::new(current.major, current.minor + 1, 0))
            }
            VersionChangeType::Patch => Ok(RulesetVersion::new(
                current.major,
                current.minor,
                current.patch + 1,
            )),
        }
    }
}

/// Type of version change
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionChangeType {
    Major, // Breaking changes
    Minor, // New features, backward compatible
    Patch, // Bug fixes, backward compatible
}

/// Version comparison result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionComparison {
    Older,
    Equal,
    Newer,
}
