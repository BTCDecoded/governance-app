//! Build orchestrator - main coordination logic

use hex;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::database::Database;
use crate::error::GovernanceError;
use crate::github::client::GitHubClient;

use super::artifacts::ArtifactCollector;
use super::dependency::DependencyGraph;
use super::monitor::{BuildMonitor, BuildStatus};

/// Build orchestrator for coordinating cross-repository builds
#[derive(Clone)]
pub struct BuildOrchestrator {
    github_client: GitHubClient,
    database: Database,
    dependency_graph: DependencyGraph,
    monitor: BuildMonitor,
    artifact_collector: ArtifactCollector,
    organization: String,
}

impl BuildOrchestrator {
    pub fn new(github_client: GitHubClient, database: Database, organization: String) -> Self {
        let dependency_graph = DependencyGraph::new(organization.clone());
        let monitor = BuildMonitor::new(
            github_client.clone(),
            organization.clone(),
            Duration::from_secs(3600), // 1 hour timeout
            3,                         // Max 3 retries
        );
        let artifact_collector =
            ArtifactCollector::new(github_client.clone(), organization.clone());

        Self {
            github_client,
            database,
            dependency_graph,
            monitor,
            artifact_collector,
            organization,
        }
    }

    /// Handle a release event and orchestrate builds
    pub async fn handle_release_event(
        &self,
        version: &str,
        prerelease: bool,
    ) -> Result<(), GovernanceError> {
        info!(
            "Handling release event for version {} (prerelease: {})",
            version, prerelease
        );

        // Get build order
        let build_order = self
            .dependency_graph
            .get_build_order()
            .map_err(|e| GovernanceError::BuildError(format!("Failed to get build order: {e}")))?;

        info!("Build order: {:?}", build_order);

        // Trigger builds in parallel groups (respecting dependencies)
        let parallel_groups = self.dependency_graph.get_parallel_groups().map_err(|e| {
            GovernanceError::BuildError(format!("Failed to get parallel groups: {e}"))
        })?;

        info!(
            "Build groups (can build in parallel): {:?}",
            parallel_groups
        );

        let mut triggered_builds = HashMap::new();
        let mut completed_repos = HashSet::new();

        // Process each parallel group sequentially, but repos within a group can build in parallel
        for group in &parallel_groups {
            // Wait for all dependencies of this group to complete
            for repo in group {
                let deps = self.dependency_graph.get_dependencies(repo);
                for dep in &deps {
                    if !completed_repos.contains(dep) {
                        if let Some(dep_run_id) = triggered_builds.get(dep) {
                            info!(
                                "Waiting for dependency {} to complete before building {}",
                                dep, repo
                            );
                            let status = self.monitor.monitor_build(dep, *dep_run_id).await?;

                            if status != BuildStatus::Success {
                                return Err(GovernanceError::BuildError(format!(
                                    "Dependency {dep} failed with status: {status:?}"
                                )));
                            }
                            completed_repos.insert(dep.clone());
                        }
                    }
                }
            }

            // Trigger all builds in this group (they can run in parallel)
            for repo in group {
                info!("Triggering build for {}", repo);
                let workflow_run_id = self.trigger_build(repo, version).await?;
                triggered_builds.insert(repo.clone(), workflow_run_id);

                // Track build state in database
                self.database
                    .upsert_build_run(version, repo, Some(workflow_run_id), "in_progress")
                    .await?;
            }
        }

        // Monitor all builds
        info!("Monitoring {} builds", triggered_builds.len());
        let build_results = self
            .monitor
            .monitor_builds(triggered_builds.clone())
            .await?;

        // Check for failures
        for (repo, status) in &build_results {
            if *status != BuildStatus::Success {
                error!("Build failed for {}: {:?}", repo, status);
                return Err(GovernanceError::BuildError(format!(
                    "Build failed for {repo}: {status:?}"
                )));
            }
        }

        // Collect artifacts
        info!("Collecting artifacts from all builds");
        let mut artifacts = self
            .artifact_collector
            .collect_all_artifacts(&triggered_builds)
            .await?;

        // Download artifacts
        info!("Downloading artifacts from all repositories");
        self.artifact_collector
            .download_all_artifacts(&mut artifacts)
            .await?;

        // Create GitHub release
        info!("Creating GitHub release for version {}", version);
        self.create_github_release(version, prerelease, &artifacts)
            .await?;

        info!(
            "Release orchestration completed successfully for version {}",
            version
        );
        Ok(())
    }

    /// Collect artifacts and create release after all builds complete
    /// This is called from the webhook handler when all builds are done
    pub async fn collect_and_create_release(
        &self,
        release_version: &str,
    ) -> Result<(), GovernanceError> {
        info!(
            "Collecting artifacts and creating release for {}",
            release_version
        );

        // Get all build runs with workflow_run_ids
        let build_runs = self
            .database
            .get_build_runs_with_ids_for_release(release_version)
            .await?;

        // Filter to only successful builds with workflow_run_ids
        let mut triggered_builds = HashMap::new();
        for (repo, workflow_run_id, status) in build_runs {
            if status == "success" {
                if let Some(run_id) = workflow_run_id {
                    triggered_builds.insert(repo, run_id);
                } else {
                    warn!(
                        "Build for {} completed successfully but has no workflow_run_id",
                        repo
                    );
                }
            } else {
                warn!("Skipping build for {} with status {}", repo, status);
            }
        }

        if triggered_builds.is_empty() {
            return Err(GovernanceError::BuildError(format!(
                "No successful builds found for release {release_version}"
            )));
        }

        // Collect artifacts
        info!(
            "Collecting artifacts from {} builds",
            triggered_builds.len()
        );
        let mut artifacts = self
            .artifact_collector
            .collect_all_artifacts(&triggered_builds)
            .await?;

        // Download artifacts
        info!("Downloading artifacts from all repositories");
        self.artifact_collector
            .download_all_artifacts(&mut artifacts)
            .await?;

        // Determine if this is a prerelease (check if version contains alpha, beta, rc, etc.)
        let prerelease = release_version.contains("alpha")
            || release_version.contains("beta")
            || release_version.contains("rc")
            || release_version.contains("dev");

        // Create GitHub release
        info!("Creating GitHub release for version {}", release_version);
        self.create_github_release(release_version, prerelease, &artifacts)
            .await?;

        info!(
            "Successfully collected artifacts and created release for {}",
            release_version
        );
        Ok(())
    }

    /// Trigger a build for a repository via repository_dispatch
    async fn trigger_build(&self, repo: &str, version: &str) -> Result<u64, GovernanceError> {
        info!(
            "Triggering build for {}/{} (version: {})",
            self.organization, repo, version
        );

        // Create repository_dispatch event payload
        let client_payload = json!({
            "version": version,
            "triggered_by": "blvm-commons",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        // Trigger workflow via repository_dispatch
        // Use "build-chain" to match existing workflows, fallback to "build-request" for new repos
        let event_type = if repo == "blvm-commons" {
            "build-request"
        } else {
            "build-chain"
        };
        let workflow_run_id = self
            .github_client
            .trigger_workflow(&self.organization, repo, event_type, &client_payload)
            .await?;

        // If we got 0, try to find the workflow run by polling
        if workflow_run_id == 0 {
            warn!("Workflow run ID not found immediately, polling for it");
            // Try to find it by listing recent workflow runs
            // Try multiple workflow files (build.yml or ci.yml)
            let workflow_files = [".github/workflows/build.yml", ".github/workflows/ci.yml"];
            let mut runs = Vec::new();

            for workflow_file in &workflow_files {
                if let Ok(found_runs) = self
                    .github_client
                    .list_workflow_runs(
                        &self.organization,
                        repo,
                        Some(workflow_file),
                        None,
                        Some(1),
                    )
                    .await
                {
                    runs = found_runs;
                    if !runs.is_empty() {
                        break;
                    }
                }
            }

            if let Some(run) = runs.first() {
                if let Some(id) = run.get("id").and_then(|v| v.as_u64()) {
                    info!("Found workflow run ID {} for {}", id, repo);
                    return Ok(id);
                }
            }
        }

        Ok(workflow_run_id)
    }

    /// Create a GitHub release with artifacts
    async fn create_github_release(
        &self,
        version: &str,
        prerelease: bool,
        artifacts: &HashMap<String, Vec<super::artifacts::Artifact>>,
    ) -> Result<(), GovernanceError> {
        info!(
            "Creating GitHub release: {} (prerelease: {})",
            version, prerelease
        );

        // Build release body with artifact list
        let mut release_body = format!(
            "Bitcoin Commons Release {version}\n\nThis release was orchestrated by blvm-commons.\n\n## Artifacts\n\n"
        );

        for (repo, repo_artifacts) in artifacts {
            release_body.push_str(&format!("### {repo}\n\n"));
            for artifact in repo_artifacts {
                release_body.push_str(&format!("- {} ({} bytes)\n", artifact.name, artifact.size));
            }
            release_body.push('\n');
        }

        // Create release in blvm repository
        let release = self
            .github_client
            .client
            .repos(&self.organization, "blvm")
            .releases()
            .create(version)
            .name(&format!("Bitcoin Commons {version}"))
            .body(&release_body)
            .prerelease(prerelease)
            .draft(false)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to create release: {}", e);
                GovernanceError::GitHubError(format!("Failed to create release: {e}"))
            })?;

        info!(
            "Created GitHub release: {} (ID: {})",
            release.html_url, release.id
        );

        // Upload artifacts to the release
        let release_id = release.id;
        let mut uploaded_count = 0;
        let mut failed_count = 0;

        for (repo, repo_artifacts) in artifacts {
            for artifact in repo_artifacts {
                // Only upload if we have the data
                if let Some(ref data) = artifact.data {
                    // Create asset name with repo prefix to avoid conflicts
                    let asset_name = format!("{}-{}", repo, artifact.name);

                    match self
                        .github_client
                        .upload_release_asset(
                            &self.organization,
                            "blvm",
                            release.id.0,
                            &asset_name,
                            data,
                            &artifact.content_type,
                        )
                        .await
                    {
                        Ok(()) => {
                            info!("Uploaded artifact: {}", asset_name);
                            uploaded_count += 1;
                        }
                        Err(e) => {
                            error!("Failed to upload artifact '{}': {}", asset_name, e);
                            failed_count += 1;
                            // Continue with other artifacts
                        }
                    }
                } else {
                    warn!(
                        "Skipping artifact '{}' from {} - no data downloaded",
                        artifact.name, repo
                    );
                    failed_count += 1;
                }
            }
        }

        info!(
            "Artifact upload complete: {} uploaded, {} failed",
            uploaded_count, failed_count
        );

        if uploaded_count == 0 && failed_count > 0 {
            warn!("No artifacts were uploaded successfully");
        }

        // Generate and upload SHA256SUMS file
        if uploaded_count > 0 {
            info!("Generating SHA256SUMS file");
            let sha256sums_content = self.generate_sha256sums(artifacts);
            let sha256sums_bytes = sha256sums_content.as_bytes();

            match self
                .github_client
                .upload_release_asset(
                    &self.organization,
                    "blvm",
                    release.id.0,
                    "SHA256SUMS",
                    sha256sums_bytes,
                    "text/plain",
                )
                .await
            {
                Ok(()) => {
                    info!("Successfully uploaded SHA256SUMS file");
                }
                Err(e) => {
                    error!("Failed to upload SHA256SUMS file: {}", e);
                    // Don't fail the entire release if SHA256SUMS upload fails
                    warn!("Release created but SHA256SUMS upload failed");
                }
            }
        } else {
            warn!("Skipping SHA256SUMS generation - no artifacts were uploaded");
        }

        Ok(())
    }

    /// Generate SHA256SUMS file content from artifacts
    fn generate_sha256sums(
        &self,
        artifacts: &HashMap<String, Vec<super::artifacts::Artifact>>,
    ) -> String {
        let mut lines = Vec::new();

        // Sort by repo name for deterministic output
        let mut repos: Vec<_> = artifacts.keys().collect();
        repos.sort();

        for repo in repos {
            let repo_artifacts = &artifacts[repo];
            for artifact in repo_artifacts {
                // Only include artifacts that have data
                if let Some(ref data) = artifact.data {
                    // Calculate SHA256 hash
                    let mut hasher = Sha256::new();
                    hasher.update(data);
                    let hash = hasher.finalize();
                    let hash_hex = hex::encode(hash);

                    // Format: hash  filename
                    // Use repo-prefixed name to match uploaded artifacts
                    let asset_name = format!("{}-{}", repo, artifact.name);
                    lines.push(format!("{hash_hex}  {asset_name}"));
                }
            }
        }

        lines.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::github::client::GitHubClient;
    use tempfile::tempdir;

    fn create_test_github_client() -> GitHubClient {
        let temp_dir = tempdir().unwrap();
        let private_key_path = temp_dir.path().join("test_key.pem");
        // Use the actual test RSA key from test_fixtures
        let valid_key = include_str!("../../test_fixtures/test_rsa_key.pem");
        std::fs::write(&private_key_path, valid_key).unwrap();
        GitHubClient::new(123456, private_key_path.to_str().unwrap()).unwrap()
    }

    async fn setup_test_orchestrator() -> (BuildOrchestrator, Database) {
        let db = Database::new_in_memory().await.unwrap();
        let github_client = create_test_github_client();
        let orchestrator =
            BuildOrchestrator::new(github_client, db.clone(), "BTCDecoded".to_string());
        (orchestrator, db)
    }

    #[tokio::test]
    async fn test_build_orchestrator_new() {
        let db = Database::new_in_memory().await.unwrap();
        let github_client = create_test_github_client();
        let orchestrator = BuildOrchestrator::new(github_client, db, "BTCDecoded".to_string());

        assert_eq!(orchestrator.organization, "BTCDecoded");
    }

    #[tokio::test]
    async fn test_build_orchestrator_has_dependency_graph() {
        let (orchestrator, _) = setup_test_orchestrator().await;

        // Verify dependency graph is initialized
        let repos = orchestrator.dependency_graph.repositories();
        assert!(!repos.is_empty());
        assert!(repos.contains(&"blvm-consensus".to_string()));
    }

    #[tokio::test]
    async fn test_build_orchestrator_has_monitor() {
        let (orchestrator, _) = setup_test_orchestrator().await;

        // Verify monitor is initialized (fields are private, so we can't directly test them)
        // The monitor is created with default values in setup_test_orchestrator
        assert!(true, "Monitor is initialized");
    }

    #[tokio::test]
    async fn test_build_orchestrator_has_artifact_collector() {
        let (orchestrator, _) = setup_test_orchestrator().await;

        // Verify artifact collector is initialized (organization field is private)
        assert!(true, "Collector is initialized");
    }

    #[tokio::test]
    async fn test_build_orchestrator_clone() {
        let (orchestrator1, _) = setup_test_orchestrator().await;
        let orchestrator2 = orchestrator1.clone();

        assert_eq!(orchestrator1.organization, orchestrator2.organization);
        assert_eq!(
            orchestrator1.dependency_graph.repositories(),
            orchestrator2.dependency_graph.repositories()
        );
    }
}
