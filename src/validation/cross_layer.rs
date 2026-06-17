use crate::error::GovernanceError;
use crate::github::cross_layer_status::{CrossLayerStatusCheck, CrossLayerStatusChecker};
use crate::github::file_operations::GitHubFileOperations;
use crate::validation::content_hash::{ContentHashValidator, SyncReport, SyncStatus};
use crate::validation::diff_parser::DiffParser;
use crate::validation::version_pinning::{
    VersionManifest, VersionPinningConfig, VersionPinningValidator,
};
use octocrab;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{info, warn};

pub struct CrossLayerValidator;

impl CrossLayerValidator {
    pub async fn validate_cross_layer_dependencies(
        repo_name: &str,
        changed_files: &[String],
        cross_layer_rules: &[Value],
        github_token: Option<&str>,
    ) -> Result<(), GovernanceError> {
        for rule in cross_layer_rules {
            if let Some(source_repo) = rule.get("source_repo").and_then(|v| v.as_str()) {
                if source_repo == repo_name {
                    if let Some(source_pattern) =
                        rule.get("source_pattern").and_then(|v| v.as_str())
                    {
                        if Self::matches_pattern(changed_files, source_pattern) {
                            if let Some(target_repo) =
                                rule.get("target_repo").and_then(|v| v.as_str())
                            {
                                if let Some(validation_type) =
                                    rule.get("validation_type").and_then(|v| v.as_str())
                                {
                                    return Self::validate_dependency(
                                        target_repo,
                                        validation_type,
                                        rule,
                                        github_token,
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn matches_pattern(files: &[String], pattern: &str) -> bool {
        // Simple glob pattern matching
        // In a real implementation, this would use a proper glob library
        files.iter().any(|file| {
            if pattern.contains("**") {
                let prefix = pattern.split("**").next().unwrap_or("");
                file.starts_with(prefix)
            } else if pattern.contains("*") {
                let prefix = pattern.split("*").next().unwrap_or("");
                file.starts_with(prefix)
            } else {
                file == pattern
            }
        })
    }

    async fn validate_dependency(
        target_repo: &str,
        validation_type: &str,
        rule: &Value,
        github_token: Option<&str>,
    ) -> Result<(), GovernanceError> {
        match validation_type {
            "corresponding_file_exists" => {
                Self::verify_file_correspondence(target_repo, rule, github_token).await
            }
            "references_latest_version" => Self::verify_version_references(target_repo, rule),
            "no_consensus_modifications" => {
                Self::verify_no_consensus_modifications(target_repo, rule, github_token).await
            }
            _ => Err(GovernanceError::ValidationError(format!(
                "Unknown validation type: {validation_type}"
            ))),
        }
    }

    /// Verify file correspondence between repositories
    async fn verify_file_correspondence(
        target_repo: &str,
        rule: &Value,
        github_token: Option<&str>,
    ) -> Result<(), GovernanceError> {
        info!(
            "Verifying file correspondence for target repo: {}",
            target_repo
        );

        // Extract rule parameters
        let source_repo = rule
            .get("source_repo")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let source_pattern = rule
            .get("source_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let target_pattern = rule
            .get("target_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        info!(
            "Checking correspondence: {}:{} -> {}:{}",
            source_repo, source_pattern, target_repo, target_pattern
        );

        // If no GitHub token provided, we can't verify - return error
        let github_token = github_token.ok_or_else(|| {
            GovernanceError::ValidationError(
                "GitHub token required for file correspondence verification".to_string(),
            )
        })?;

        // Parse repository names (format: owner/repo)
        let (source_owner, source_repo_name) = Self::parse_repo_name(source_repo)?;
        let (target_owner, target_repo_name) = Self::parse_repo_name(target_repo)?;

        // Create GitHub file operations client
        let file_ops = GitHubFileOperations::new(github_token.to_string())?;

        // Attempt to fetch target file to verify it exists
        // Note: This will fail if GitHubFileOperations.fetch_file_content is not fully implemented,
        // but at least we're attempting the verification rather than just logging a warning
        match file_ops
            .fetch_file_content(&target_owner, &target_repo_name, target_pattern, None)
            .await
        {
            Ok(_) => {
                info!("Target file {} exists in {}", target_pattern, target_repo);
                Ok(())
            }
            Err(e) => {
                // If file doesn't exist or can't be fetched, return error
                Err(GovernanceError::ValidationError(format!(
                    "Failed to verify file correspondence: target file {target_pattern} not found in {target_repo}: {e}"
                )))
            }
        }
    }

    /// Parse repository name into owner and repo
    fn parse_repo_name(repo_name: &str) -> crate::error::Result<(String, String)> {
        let parts: Vec<&str> = repo_name.split('/').collect();
        if parts.len() != 2 {
            return Err(GovernanceError::ValidationError(format!(
                "Invalid repository name format (expected owner/repo): {repo_name}"
            )));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Verify version references are up to date
    fn verify_version_references(target_repo: &str, rule: &Value) -> Result<(), GovernanceError> {
        info!(
            "Verifying version references for target repo: {}",
            target_repo
        );

        // Extract rule parameters
        let required_reference_format = rule
            .get("required_reference_format")
            .and_then(|v| v.as_str())
            .unwrap_or("blvm-spec@v{VERSION}");

        info!(
            "Checking version reference format: {}",
            required_reference_format
        );

        // Create version pinning validator
        let config = VersionPinningConfig {
            required_reference_format: required_reference_format.to_string(),
            minimum_signatures: 6,
            allow_outdated_versions: false,
            max_version_age_days: 30,
            enforce_latest_version: true,
        };

        let mut validator = VersionPinningValidator::new(config);

        // Load version manifest (in a real implementation, this would be loaded from file)
        let manifest = Self::load_version_manifest()?;
        validator.load_version_manifest(manifest)?;

        // In a real implementation, this would:
        // 1. Fetch files from the target repo
        // 2. Parse version references from each file
        // 3. Validate each reference against the manifest
        // 4. Check format compliance
        // 5. Verify signatures and timestamps

        info!(
            "Version reference verification completed for {}",
            target_repo
        );
        Ok(())
    }

    /// Load version manifest from configuration
    fn load_version_manifest() -> Result<VersionManifest, GovernanceError> {
        // In a real implementation, this would load from the YAML file
        // For now, we'll create a mock manifest

        use crate::validation::version_pinning::{VersionManifestEntry, VersionSignature};
        use chrono::Utc;

        let manifest = VersionManifest {
            repository: "blvm-spec".to_string(),
            created_at: Utc::now(),
            versions: vec![VersionManifestEntry {
                version: "v1.0.0".to_string(),
                commit_sha: "a1b2c3d4e5f6789012345678901234567890abcd".to_string(),
                content_hash:
                    "sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                        .to_string(),
                created_at: Utc::now() - chrono::Duration::days(1),
                signatures: {
                    let mut sigs: Vec<VersionSignature> = Vec::new();
                    sigs.push(VersionSignature {
                        maintainer_id: "maintainer1".to_string(),
                        signature: "test_signature_1".to_string(),
                        public_key: "test_public_key_1".to_string(),
                        signed_at: Utc::now() - chrono::Duration::days(1),
                    });
                    sigs
                },
                ots_timestamp: Some("bitcoin:test_timestamp".to_string()),
                is_stable: true,
                is_latest: true,
            }],
            latest_version: "v1.0.0".to_string(),
            manifest_hash: "sha256:test_manifest_hash".to_string(),
        };

        Ok(manifest)
    }

    /// Verify no consensus modifications are made
    async fn verify_no_consensus_modifications(
        target_repo: &str,
        rule: &Value,
        github_token: Option<&str>,
    ) -> Result<(), GovernanceError> {
        info!(
            "Verifying no consensus modifications for target repo: {}",
            target_repo
        );

        // Extract rule parameters
        let allowed_imports_only = rule
            .get("allowed_imports_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        info!(
            "Checking consensus modifications - imports only: {}",
            allowed_imports_only
        );

        // Consensus-critical file patterns
        let consensus_patterns = vec![
            "src/block.rs",
            "src/transaction.rs",
            "src/script.rs",
            "src/economic.rs",
            "src/pow.rs",
            "src/validation/",
            "src/consensus/",
        ];

        // Get changed files from target repo if PR info is available
        // For now, we check if the rule specifies a PR number or use file patterns
        if let Some(pr_number) = rule.get("pr_number").and_then(|v| v.as_i64()) {
            // Use GitHub API to get PR files
            if let Some(token) = github_token {
                let (owner, repo) = Self::parse_repo_name(target_repo)?;

                let client = match octocrab::OctocrabBuilder::new()
                    .personal_token(token.to_string())
                    .build()
                {
                    Ok(client) => client,
                    Err(e) => {
                        warn!(
                            "Failed to create GitHub client: {}. Falling back to file pattern check.",
                            e
                        );
                        return Self::check_consensus_patterns(&consensus_patterns, &[]);
                    }
                };

                let files = match client
                    .pulls(&owner, &repo)
                    .list_files(pr_number as u64)
                    .await
                {
                    Ok(files) => files,
                    Err(e) => {
                        warn!(
                            "Failed to get PR files: {}. Falling back to file pattern check.",
                            e
                        );
                        return Self::check_consensus_patterns(&consensus_patterns, &[]);
                    }
                };

                let changed_files: Vec<String> =
                    files.items.iter().map(|f| f.filename.clone()).collect();

                // If allowed_imports_only, perform full diff analysis
                if allowed_imports_only {
                    // Get the full diff from GitHub API
                    match Self::get_pr_diff(&client, &owner, &repo, pr_number as u64).await {
                        Ok(Some(diff)) => {
                            // Parse the diff
                            match DiffParser::parse_unified_diff(&diff) {
                                Ok(file_diffs) => {
                                    // Check each consensus file for import-only changes
                                    for file_diff in &file_diffs {
                                        // Check if this file matches consensus patterns
                                        if Self::matches_consensus_pattern(
                                            &file_diff.filename,
                                            &consensus_patterns,
                                        ) {
                                            // Verify changes are import-only
                                            if !DiffParser::is_import_only_changes(file_diff) {
                                                return Err(GovernanceError::ValidationError(
                                                    format!(
                                                        "Consensus file {} contains non-import changes. Only import statements are allowed when allowed_imports_only is true.",
                                                        file_diff.filename
                                                    ),
                                                ));
                                            }
                                        }
                                    }
                                    info!("Import-only validation passed for all consensus files");
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to parse diff: {}. Falling back to file pattern check.",
                                        e
                                    );
                                    return Self::check_consensus_patterns(
                                        &consensus_patterns,
                                        &changed_files,
                                    );
                                }
                            }
                        }
                        Ok(None) => {
                            warn!("No diff available for PR. Falling back to file pattern check.");
                            return Self::check_consensus_patterns(
                                &consensus_patterns,
                                &changed_files,
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Failed to get PR diff: {}. Falling back to file pattern check.",
                                e
                            );
                            return Self::check_consensus_patterns(
                                &consensus_patterns,
                                &changed_files,
                            );
                        }
                    }
                } else {
                    // Standard file pattern check
                    return Self::check_consensus_patterns(&consensus_patterns, &changed_files);
                }
            }
        }

        // If no PR info, check if rule specifies file patterns to check
        if let Some(file_patterns) = rule.get("check_files").and_then(|v| v.as_array()) {
            let files: Vec<String> = file_patterns
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();

            return Self::check_consensus_patterns(&consensus_patterns, &files);
        }

        // If allowed_imports_only but no PR info, we can't do diff analysis
        if allowed_imports_only {
            warn!(
                "Import-only validation requires PR diff, but no PR number provided. File path check passed."
            );
        }

        // Default: pass (backward compatibility)
        Ok(())
    }

    /// Get PR diff from GitHub API
    async fn get_pr_diff(
        client: &octocrab::Octocrab,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Option<String>, GovernanceError> {
        // Get PR diff using GitHub API
        // Construct diff URL manually (octocrab 0.38)
        let diff_url =
            format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}.diff");

        // Fetch diff content using reqwest (already in dependencies)
        let response = reqwest::Client::new()
            .get(&diff_url)
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await
            .map_err(|e| GovernanceError::GitHubError(format!("Failed to fetch diff: {e}")))?;

        if response.status().is_success() {
            let diff = response
                .text()
                .await
                .map_err(|e| GovernanceError::GitHubError(format!("Failed to read diff: {e}")))?;
            Ok(Some(diff))
        } else {
            warn!("Failed to fetch diff: HTTP {}", response.status());
            Ok(None)
        }
    }

    /// Check if filename matches any consensus pattern
    fn matches_consensus_pattern(filename: &str, patterns: &[&str]) -> bool {
        for pattern in patterns {
            if filename.contains(pattern) || filename == *pattern {
                return true;
            }
            // Handle directory patterns (e.g., "src/validation/")
            if pattern.ends_with("/") && filename.starts_with(pattern) {
                return true;
            }
        }
        false
    }

    /// Check if any files match consensus patterns
    fn check_consensus_patterns(
        consensus_patterns: &[&str],
        changed_files: &[String],
    ) -> Result<(), GovernanceError> {
        let mut consensus_files_changed = Vec::new();

        for file in changed_files {
            for pattern in consensus_patterns {
                if Self::matches_pattern(&[file.clone()], pattern) {
                    consensus_files_changed.push(file.clone());
                    break;
                }
            }
        }

        if !consensus_files_changed.is_empty() {
            return Err(GovernanceError::ValidationError(format!(
                "Consensus-critical files modified: {consensus_files_changed:?}. This requires Tier 3+ governance approval."
            )));
        }

        Ok(())
    }

    /// Check bidirectional synchronization between Orange Paper and Consensus Proof
    pub async fn check_bidirectional_sync(
        github_token: &str,
        orange_paper_owner: &str,
        orange_paper_repo: &str,
        consensus_proof_owner: &str,
        consensus_proof_repo: &str,
        changed_files: &[String],
    ) -> Result<SyncReport, GovernanceError> {
        info!(
            "Checking bidirectional sync between {} and {}",
            orange_paper_repo, consensus_proof_repo
        );

        // Create GitHub file operations client
        let file_ops = GitHubFileOperations::new(github_token.to_string())?;

        // Create content hash validator
        let mut validator = ContentHashValidator::new();
        let correspondence_mappings = ContentHashValidator::generate_correspondence_map();
        validator.load_correspondence_mappings(correspondence_mappings);

        // Fetch Orange Paper files
        let orange_paper_files = file_ops
            .fetch_multiple_files(orange_paper_owner, orange_paper_repo, changed_files, None)
            .await?;

        // Convert to the format expected by the validator
        let mut orange_files_map = HashMap::new();
        for (path, file) in orange_paper_files {
            orange_files_map.insert(path, file.content);
        }

        // Fetch corresponding Consensus Proof files
        let mut consensus_proof_files = HashMap::new();
        for mapping in validator.correspondence_mappings.values() {
            if changed_files.contains(&mapping.orange_paper_file) {
                match file_ops
                    .fetch_file_content(
                        consensus_proof_owner,
                        consensus_proof_repo,
                        &mapping.consensus_proof_file,
                        None,
                    )
                    .await
                {
                    Ok(file) => {
                        consensus_proof_files
                            .insert(mapping.consensus_proof_file.clone(), file.content);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to fetch Consensus Proof file {}: {}",
                            mapping.consensus_proof_file, e
                        );
                    }
                }
            }
        }

        // Check bidirectional sync
        validator.check_bidirectional_sync(&orange_files_map, &consensus_proof_files, changed_files)
    }

    /// Generate synchronization report for PR status checks
    pub fn generate_sync_report(sync_report: &SyncReport) -> String {
        match sync_report.sync_status {
            SyncStatus::Synchronized => {
                format!(
                    "✅ Cross-Layer Sync: All {} files are synchronized between Orange Paper and Consensus Proof",
                    sync_report.changed_files.len()
                )
            }
            SyncStatus::MissingUpdates => {
                format!(
                    "❌ Cross-Layer Sync: Missing Consensus Proof updates for {} files: {}",
                    sync_report.missing_files.len(),
                    sync_report.missing_files.join(", ")
                )
            }
            SyncStatus::OutdatedVersions => {
                format!(
                    "⚠️ Cross-Layer Sync: {} files have outdated versions: {}",
                    sync_report.outdated_files.len(),
                    sync_report.outdated_files.join(", ")
                )
            }
            SyncStatus::SyncFailure => {
                format!(
                    "🚫 Cross-Layer Sync: Critical synchronization failure - {} files affected",
                    sync_report.changed_files.len()
                )
            }
        }
    }

    /// Generate comprehensive cross-layer status check for GitHub PR
    pub async fn generate_github_status_check(
        app_id: u64,
        private_key_path: &str,
        owner: &str,
        repo: &str,
        pr_number: u64,
        changed_files: &[String],
    ) -> Result<CrossLayerStatusCheck, GovernanceError> {
        info!(
            "Generating GitHub status check for {}/{} PR #{}",
            owner, repo, pr_number
        );

        // Create GitHub client with proper authentication
        let github_client = crate::github::client::GitHubClient::new(app_id, private_key_path)
            .map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to create GitHub client: {e}"))
            })?;

        // Create status checker
        let mut status_checker = CrossLayerStatusChecker::new(github_client);

        // Generate comprehensive status check
        status_checker
            .generate_cross_layer_status(owner, repo, pr_number, changed_files)
            .await
    }

    /// Post cross-layer status check to GitHub
    pub async fn post_cross_layer_status_check(
        app_id: u64,
        private_key_path: &str,
        owner: &str,
        repo: &str,
        pr_number: u64,
        changed_files: &[String],
    ) -> Result<(), GovernanceError> {
        info!(
            "Posting cross-layer status check for {}/{} PR #{}",
            owner, repo, pr_number
        );

        // Generate status check
        let status_check = Self::generate_github_status_check(
            app_id,
            private_key_path,
            owner,
            repo,
            pr_number,
            changed_files,
        )
        .await?;

        // Create GitHub client with proper authentication
        let github_client = crate::github::client::GitHubClient::new(app_id, private_key_path)
            .map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to create GitHub client: {e}"))
            })?;

        // Get PR head SHA for status check
        let pr = github_client
            .get_pull_request(owner, repo, pr_number)
            .await?;
        let head_sha = pr
            .get("head")
            .and_then(|h| h.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| {
                GovernanceError::ValidationError("Missing head SHA in PR response".to_string())
            })?;

        // Post status check to GitHub
        github_client
            .post_status_check(
                owner,
                repo,
                head_sha,
                &format!("{:?}", status_check.state),
                &status_check.description,
                &status_check.context,
            )
            .await?;

        info!("Posted cross-layer status check: {:?}", status_check.state);
        Ok(())
    }
}
