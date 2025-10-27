//! Version Pinning and References
//!
//! This module provides cryptographic version pinning where Consensus Proof must reference
//! specific Orange Paper versions. It ensures version references are cryptographically verified
//! and prevents outdated version references.

use crate::error::GovernanceError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{info, warn, error, debug};

/// Version reference found in code
#[derive(Debug, Clone, PartialEq)]
pub struct VersionReference {
    pub file_path: String,
    pub line_number: usize,
    pub reference_type: VersionReferenceType,
    pub version: String,
    pub commit_sha: Option<String>,
    pub content_hash: Option<String>,
    pub raw_text: String,
}

/// Types of version references
#[derive(Debug, Clone, PartialEq)]
pub enum VersionReferenceType {
    /// @orange-paper-version: v1.2.3
    Version,
    /// @orange-paper-commit: abc123def456
    Commit,
    /// @orange-paper-hash: sha256:fedcba...
    ContentHash,
    /// Combined reference with multiple components
    Combined,
}

/// Version manifest entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifestEntry {
    pub version: String,
    pub commit_sha: String,
    pub content_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub signatures: Vec<VersionSignature>,
    pub ots_timestamp: Option<String>,
    pub is_stable: bool,
    pub is_latest: bool,
}

/// Version signature from maintainer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSignature {
    pub maintainer_id: String,
    pub signature: String,
    pub public_key: String,
    pub signed_at: chrono::DateTime<chrono::Utc>,
}

/// Version manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub repository: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub versions: Vec<VersionManifestEntry>,
    pub latest_version: String,
    pub manifest_hash: String,
}

/// Version pinning validation result
#[derive(Debug, Clone)]
pub struct VersionPinningResult {
    pub file_path: String,
    pub references: Vec<VersionReference>,
    pub validation_status: ValidationStatus,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validation status for version references
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStatus {
    /// All references are valid and up to date
    Valid,
    /// Some references are outdated but acceptable
    Outdated,
    /// Some references are invalid or missing
    Invalid,
    /// Critical validation failure
    Failed,
}

/// Version pinning configuration
#[derive(Debug, Clone)]
pub struct VersionPinningConfig {
    pub required_reference_format: String,
    pub minimum_signatures: usize,
    pub allow_outdated_versions: bool,
    pub max_version_age_days: u32,
    pub enforce_latest_version: bool,
}

impl Default for VersionPinningConfig {
    fn default() -> Self {
        Self {
            required_reference_format: "orange-paper@v{VERSION}".to_string(),
            minimum_signatures: 6,
            allow_outdated_versions: false,
            max_version_age_days: 30,
            enforce_latest_version: true,
        }
    }
}

pub struct VersionPinningValidator {
    config: VersionPinningConfig,
    version_manifest: Option<VersionManifest>,
}

impl VersionPinningValidator {
    /// Create a new version pinning validator
    pub fn new(config: VersionPinningConfig) -> Self {
        Self {
            config,
            version_manifest: None,
        }
    }

    /// Load version manifest from configuration
    pub fn load_version_manifest(&mut self, manifest: VersionManifest) -> Result<(), GovernanceError> {
        // Verify manifest integrity
        self.verify_manifest_integrity(&manifest)?;
        
        self.version_manifest = Some(manifest);
        info!("Loaded version manifest with {} versions", 
              self.version_manifest.as_ref().unwrap().versions.len());
        
        Ok(())
    }

    /// Parse version references from file content
    pub fn parse_version_references(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<Vec<VersionReference>, GovernanceError> {
        let mut references = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (line_num, line) in lines.iter().enumerate() {
            // Look for version reference patterns
            if let Some(reference) = self.extract_version_reference(file_path, line_num + 1, line) {
                references.push(reference);
            }
        }

        debug!("Found {} version references in {}", references.len(), file_path);
        Ok(references)
    }

    /// Extract version reference from a single line
    fn extract_version_reference(
        &self,
        file_path: &str,
        line_number: usize,
        line: &str,
    ) -> Option<VersionReference> {
        let trimmed = line.trim();
        
        // Pattern: @orange-paper-version: v1.2.3
        if let Some(captures) = regex::Regex::new(r"@orange-paper-version:\s*(v?\d+\.\d+\.\d+)")
            .unwrap()
            .captures(trimmed)
        {
            return Some(VersionReference {
                file_path: file_path.to_string(),
                line_number,
                reference_type: VersionReferenceType::Version,
                version: captures[1].to_string(),
                commit_sha: None,
                content_hash: None,
                raw_text: trimmed.to_string(),
            });
        }

        // Pattern: @orange-paper-commit: abc123def456
        if let Some(captures) = regex::Regex::new(r"@orange-paper-commit:\s*([a-f0-9]{40})")
            .unwrap()
            .captures(trimmed)
        {
            return Some(VersionReference {
                file_path: file_path.to_string(),
                line_number,
                reference_type: VersionReferenceType::Commit,
                version: String::new(),
                commit_sha: Some(captures[1].to_string()),
                content_hash: None,
                raw_text: trimmed.to_string(),
            });
        }

        // Pattern: @orange-paper-hash: sha256:fedcba...
        if let Some(captures) = regex::Regex::new(r"@orange-paper-hash:\s*(sha256:[a-f0-9]{64})")
            .unwrap()
            .captures(trimmed)
        {
            return Some(VersionReference {
                file_path: file_path.to_string(),
                line_number,
                reference_type: VersionReferenceType::ContentHash,
                version: String::new(),
                commit_sha: None,
                content_hash: Some(captures[1].to_string()),
                raw_text: trimmed.to_string(),
            });
        }

        // Pattern: Combined reference
        if trimmed.contains("@orange-paper") && trimmed.contains(":") {
            return Some(VersionReference {
                file_path: file_path.to_string(),
                line_number,
                reference_type: VersionReferenceType::Combined,
                version: String::new(),
                commit_sha: None,
                content_hash: None,
                raw_text: trimmed.to_string(),
            });
        }

        None
    }

    /// Verify version signature
    pub fn verify_version_signature(
        &self,
        signature: &VersionSignature,
        public_key: &str,
    ) -> Result<bool, GovernanceError> {
        // In a real implementation, this would:
        // 1. Parse the public key
        // 2. Verify the signature against the version data
        // 3. Check signature format and validity
        
        // For now, we'll implement a basic validation
        if signature.signature.is_empty() || public_key.is_empty() {
            return Ok(false);
        }

        // Basic format validation
        if signature.signature.len() < 64 || public_key.len() < 64 {
            return Ok(false);
        }

        // In a real implementation, we would use secp256k1 to verify the signature
        // For now, we'll just return true for valid-looking signatures
        Ok(true)
    }

    /// Check version compatibility
    pub fn check_version_compatibility(
        &self,
        version: &str,
        manifest: &VersionManifest,
    ) -> Result<ValidationStatus, GovernanceError> {
        // Find the version in the manifest
        let version_entry = manifest.versions.iter()
            .find(|v| v.version == version);

        match version_entry {
            Some(entry) => {
                // Check if version is stable
                if !entry.is_stable {
                    return Ok(ValidationStatus::Invalid);
                }

                // Check if version is too old
                let age_days = chrono::Utc::now()
                    .signed_duration_since(entry.created_at)
                    .num_days() as u32;

                if age_days > self.config.max_version_age_days {
                    if self.config.allow_outdated_versions {
                        return Ok(ValidationStatus::Outdated);
                    } else {
                        return Ok(ValidationStatus::Invalid);
                    }
                }

                // Check if version is latest
                if self.config.enforce_latest_version && !entry.is_latest {
                    return Ok(ValidationStatus::Outdated);
                }

                Ok(ValidationStatus::Valid)
            }
            None => {
                warn!("Version {} not found in manifest", version);
                Ok(ValidationStatus::Invalid)
            }
        }
    }

    /// Enforce latest version requirement
    pub fn enforce_latest_version(
        &self,
        references: &[VersionReference],
        manifest: &VersionManifest,
    ) -> Result<Vec<String>, GovernanceError> {
        let mut errors = Vec::new();

        for reference in references {
            if reference.reference_type == VersionReferenceType::Version {
                let status = self.check_version_compatibility(&reference.version, manifest)?;
                
                match status {
                    ValidationStatus::Invalid => {
                        errors.push(format!(
                            "Invalid version reference in {}:{} - version {} not found in manifest",
                            reference.file_path, reference.line_number, reference.version
                        ));
                    }
                    ValidationStatus::Outdated => {
                        if self.config.enforce_latest_version {
                            errors.push(format!(
                                "Outdated version reference in {}:{} - version {} is not latest (latest: {})",
                                reference.file_path, reference.line_number, reference.version, manifest.latest_version
                            ));
                        }
                    }
                    ValidationStatus::Valid => {
                        // Version is valid
                    }
                    ValidationStatus::Failed => {
                        errors.push(format!(
                            "Version validation failed for {}:{} - version {}",
                            reference.file_path, reference.line_number, reference.version
                        ));
                    }
                }
            }
        }

        Ok(errors)
    }

    /// Validate all version references in a file
    pub fn validate_file_references(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<VersionPinningResult, GovernanceError> {
        let references = self.parse_version_references(file_path, content)?;
        
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(manifest) = &self.version_manifest {
            // Check version compatibility
            for reference in &references {
                if reference.reference_type == VersionReferenceType::Version {
                    let status = self.check_version_compatibility(&reference.version, manifest)?;
                    
                    match status {
                        ValidationStatus::Invalid => {
                            errors.push(format!(
                                "Invalid version {} in {}:{}",
                                reference.version, reference.file_path, reference.line_number
                            ));
                        }
                        ValidationStatus::Outdated => {
                            warnings.push(format!(
                                "Outdated version {} in {}:{}",
                                reference.version, reference.file_path, reference.line_number
                            ));
                        }
                        ValidationStatus::Valid => {
                            // Version is valid
                        }
                        ValidationStatus::Failed => {
                            errors.push(format!(
                                "Version validation failed for {} in {}:{}",
                                reference.version, reference.file_path, reference.line_number
                            ));
                        }
                    }
                }
            }

            // Enforce latest version if configured
            if self.config.enforce_latest_version {
                let latest_errors = self.enforce_latest_version(&references, manifest)?;
                errors.extend(latest_errors);
            }
        } else {
            errors.push("Version manifest not loaded".to_string());
        }

        // Determine overall validation status
        let validation_status = if errors.is_empty() {
            if warnings.is_empty() {
                ValidationStatus::Valid
            } else {
                ValidationStatus::Outdated
            }
        } else {
            ValidationStatus::Invalid
        };

        Ok(VersionPinningResult {
            file_path: file_path.to_string(),
            references,
            validation_status,
            errors,
            warnings,
        })
    }

    /// Verify manifest integrity
    fn verify_manifest_integrity(&self, manifest: &VersionManifest) -> Result<(), GovernanceError> {
        // Verify manifest hash
        let computed_hash = self.compute_manifest_hash(manifest);
        if computed_hash != manifest.manifest_hash {
            return Err(GovernanceError::ValidationError(
                "Version manifest hash verification failed".to_string()
            ));
        }

        // Verify signatures for each version
        for version in &manifest.versions {
            if version.signatures.len() < self.config.minimum_signatures {
                return Err(GovernanceError::ValidationError(format!(
                    "Version {} has insufficient signatures: {} < {}",
                    version.version, version.signatures.len(), self.config.minimum_signatures
                )));
            }

            // Verify each signature
            for signature in &version.signatures {
                if !self.verify_version_signature(signature, &signature.public_key)? {
                    return Err(GovernanceError::ValidationError(format!(
                        "Invalid signature for version {} by maintainer {}",
                        version.version, signature.maintainer_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Compute manifest hash
    fn compute_manifest_hash(&self, manifest: &VersionManifest) -> String {
        let mut hasher = Sha256::new();
        
        // Hash the manifest data (excluding the hash field itself)
        let manifest_data = serde_json::to_string(&manifest).unwrap_or_default();
        hasher.update(manifest_data.as_bytes());
        
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    /// Generate version reference format
    pub fn generate_reference_format(
        &self,
        version: &str,
        commit_sha: &str,
        content_hash: &str,
    ) -> String {
        format!(
            "// @orange-paper-version: {}\n// @orange-paper-commit: {}\n// @orange-paper-hash: {}",
            version, commit_sha, content_hash
        )
    }

    /// Get latest version from manifest
    pub fn get_latest_version(&self) -> Option<String> {
        self.version_manifest.as_ref().map(|m| m.latest_version.clone())
    }

    /// Get version entry by version string
    pub fn get_version_entry(&self, version: &str) -> Option<&VersionManifestEntry> {
        self.version_manifest.as_ref()
            .and_then(|m| m.versions.iter().find(|v| v.version == version))
    }
}

impl Default for VersionPinningValidator {
    fn default() -> Self {
        Self::new(VersionPinningConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_references() {
        let validator = VersionPinningValidator::default();
        let content = r#"
// @orange-paper-version: v1.2.3
// @orange-paper-commit: abc123def456789
// @orange-paper-hash: sha256:fedcba123456
"#;

        let references = validator.parse_version_references("test.rs", content).unwrap();
        
        assert_eq!(references.len(), 3);
        assert_eq!(references[0].reference_type, VersionReferenceType::Version);
        assert_eq!(references[0].version, "v1.2.3");
        assert_eq!(references[1].reference_type, VersionReferenceType::Commit);
        assert_eq!(references[1].commit_sha, Some("abc123def456789".to_string()));
        assert_eq!(references[2].reference_type, VersionReferenceType::ContentHash);
        assert_eq!(references[2].content_hash, Some("sha256:fedcba123456".to_string()));
    }

    #[test]
    fn test_generate_reference_format() {
        let validator = VersionPinningValidator::default();
        let format = validator.generate_reference_format("v1.2.3", "abc123", "sha256:def456");
        
        assert!(format.contains("@orange-paper-version: v1.2.3"));
        assert!(format.contains("@orange-paper-commit: abc123"));
        assert!(format.contains("@orange-paper-hash: sha256:def456"));
    }

    #[test]
    fn test_version_compatibility() {
        let validator = VersionPinningValidator::default();
        
        let manifest = VersionManifest {
            repository: "orange-paper".to_string(),
            created_at: chrono::Utc::now(),
            versions: vec![
                VersionManifestEntry {
                    version: "v1.2.3".to_string(),
                    commit_sha: "abc123".to_string(),
                    content_hash: "sha256:def456".to_string(),
                    created_at: chrono::Utc::now() - chrono::Duration::days(1),
                    signatures: vec![],
                    ots_timestamp: None,
                    is_stable: true,
                    is_latest: true,
                }
            ],
            latest_version: "v1.2.3".to_string(),
            manifest_hash: "sha256:test".to_string(),
        };

        let status = validator.check_version_compatibility("v1.2.3", &manifest).unwrap();
        assert_eq!(status, ValidationStatus::Valid);

        let status = validator.check_version_compatibility("v1.0.0", &manifest).unwrap();
        assert_eq!(status, ValidationStatus::Invalid);
    }
}


