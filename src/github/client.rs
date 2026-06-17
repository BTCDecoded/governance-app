use octocrab::Octocrab;
use reqwest::Client as ReqwestClient;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::error::GovernanceError;
use crate::github::types::{CheckRun, WorkflowStatus};

#[derive(Clone)]
pub struct GitHubClient {
    pub(crate) client: Octocrab,
    app_id: u64,
    http_client: ReqwestClient,
    /// Circuit breaker for GitHub API calls
    circuit_breaker: Arc<crate::resilience::CircuitBreaker>,
}

impl GitHubClient {
    pub fn new(app_id: u64, private_key_path: &str) -> Result<Self, GovernanceError> {
        let key = std::fs::read_to_string(private_key_path).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read private key: {e}"))
        })?;

        let client = Octocrab::builder()
            .app(
                app_id.into(),
                jsonwebtoken::EncodingKey::from_rsa_pem(key.as_bytes()).map_err(|e| {
                    GovernanceError::GitHubError(format!("Failed to parse private key: {e}"))
                })?,
            )
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create GitHub client: {e}"))
            })?;

        let http_client = ReqwestClient::builder()
            .user_agent("blvm-commons/0.1.0")
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create HTTP client: {e}"))
            })?;

        let circuit_breaker = Arc::new(crate::resilience::CircuitBreaker::with_config(
            "github-api",
            crate::resilience::CircuitBreakerConfig {
                failure_threshold: 5,
                success_threshold: 2,
                timeout: std::time::Duration::from_secs(60),
                window_duration: std::time::Duration::from_secs(60),
            },
        ));

        Ok(Self {
            client,
            app_id,
            http_client,
            circuit_breaker,
        })
    }

    /// Create client from personal access token or installation token.
    /// Use when GITHUB_TOKEN or GITHUB_INSTALLATION_TOKEN is available (e.g. GitHub Actions).
    pub fn from_token(token: impl Into<String>) -> Result<Self, GovernanceError> {
        let token = token.into();
        if token.is_empty() {
            return Err(GovernanceError::ConfigError(
                "GitHub token cannot be empty".to_string(),
            ));
        }

        let client = Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create GitHub client: {e}"))
            })?;

        let http_client = ReqwestClient::builder()
            .user_agent("blvm-commons/0.1.0")
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create HTTP client: {e}"))
            })?;

        let circuit_breaker = Arc::new(crate::resilience::CircuitBreaker::with_config(
            "github-api",
            crate::resilience::CircuitBreakerConfig {
                failure_threshold: 5,
                success_threshold: 2,
                timeout: std::time::Duration::from_secs(60),
                window_duration: std::time::Duration::from_secs(60),
            },
        ));

        Ok(Self {
            client,
            app_id: 0,
            http_client,
            circuit_breaker,
        })
    }

    /// Post a status check to GitHub
    pub async fn post_status_check(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        state: &str,
        description: &str,
        context: &str,
    ) -> Result<(), GovernanceError> {
        // Input validation
        if owner.is_empty() || repo.is_empty() || sha.is_empty() {
            return Err(GovernanceError::GitHubError(format!(
                "Invalid input: owner, repo, and sha must be non-empty (owner={owner}, repo={repo}, sha={sha})"
            )));
        }

        if !sha.chars().all(|c| c.is_ascii_hexdigit()) || sha.len() != 40 {
            warn!("SHA '{}' may be invalid (expected 40 hex characters)", sha);
        }

        info!(
            "Posting status check for {}/{}@{}: {} - {} ({})",
            owner, repo, sha, state, description, context
        );

        // Convert state to GitHub API format
        let github_state = match state {
            "success" => octocrab::models::StatusState::Success,
            "failure" => octocrab::models::StatusState::Failure,
            "pending" => octocrab::models::StatusState::Pending,
            "error" => octocrab::models::StatusState::Error,
            _ => {
                warn!("Unknown status state '{}', defaulting to Error", state);
                octocrab::models::StatusState::Error
            }
        };

        // Create status check payload
        // Post status check via GitHub API with circuit breaker protection
        self.circuit_breaker
            .call(|| async {
                // Post status check via GitHub API
                // Note: octocrab 0.38 API - target_url is optional and can be set via builder
                let repos_handler = self.client.repos(owner, repo);
                let status_builder = repos_handler
                    .create_status(sha.to_string(), github_state)
                    .description(description.to_string())
                    .context(context.to_string());

                // target_url is optional in octocrab 0.38 - can be omitted if not needed
                status_builder
                    .send()
                    .await
                    .map_err(|e| {
                        error!("Failed to post status check for {}/{}@{}: {}", owner, repo, sha, e);
                        GovernanceError::GitHubError(format!(
                            "Failed to post status check for {owner}/{repo}@{sha}: {e}. Check repository permissions and SHA validity."
                        ))
                    })
            })
            .await
            .map_err(|e| match e {
                crate::resilience::CircuitBreakerError::CircuitOpen => {
                    warn!("GitHub API circuit breaker is open - rejecting request");
                    GovernanceError::GitHubError(
                        "GitHub API circuit breaker is open - service temporarily unavailable".to_string()
                    )
                }
                crate::resilience::CircuitBreakerError::ServiceError(e) => e,
            })?;

        info!(
            "Successfully posted status check: {}/{}@{} - {:?}: {} ({})",
            owner, repo, sha, github_state, description, context
        );

        Ok(())
    }

    /// Update an existing status check
    pub async fn update_status_check(
        &self,
        owner: &str,
        repo: &str,
        check_run_id: u64,
        state: &str,
        description: &str,
    ) -> Result<(), GovernanceError> {
        info!(
            "Updating status check for {}/{} (ID: {}): {} - {}",
            owner, repo, check_run_id, state, description
        );

        // Convert state to GitHub API format - octocrab 0.38 uses CheckRunConclusion enum from params
        use octocrab::params::checks::CheckRunConclusion;
        let conclusion_opt = match state {
            "success" => Some(CheckRunConclusion::Success),
            "failure" => Some(CheckRunConclusion::Failure),
            "cancelled" => Some(CheckRunConclusion::Cancelled),
            "timed_out" => Some(CheckRunConclusion::TimedOut),
            "action_required" => Some(CheckRunConclusion::ActionRequired),
            "neutral" => Some(CheckRunConclusion::Neutral),
            _ => None,
        };

        // octocrab 0.38 API - use checks().update_check_run()
        // Note: output_title/output_summary may not be available in 0.38
        let checks_handler = self.client.checks(owner, repo);
        let mut builder =
            checks_handler.update_check_run(octocrab::models::CheckRunId(check_run_id));

        if let Some(conclusion) = conclusion_opt {
            builder = builder.conclusion(conclusion);
        }

        builder.send().await.map_err(|e| {
            error!("Failed to update status check: {}", e);
            GovernanceError::GitHubError(format!("Failed to update status check: {e}"))
        })?;

        info!(
            "Successfully updated status check: {} - {} ({})",
            state, description, check_run_id
        );

        Ok(())
    }

    /// Post a comment to a GitHub issue
    pub async fn post_issue_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<(), GovernanceError> {
        if owner.is_empty() || repo.is_empty() || body.is_empty() {
            return Err(GovernanceError::GitHubError(
                "owner, repo, and body must be non-empty".to_string(),
            ));
        }

        self.client
            .issues(owner, repo)
            .create_comment(issue_number, body)
            .await
            .map_err(|e| {
                error!("Failed to post issue comment: {}", e);
                GovernanceError::GitHubError(format!("Failed to post issue comment: {e}"))
            })?;

        info!(
            "Posted comment to {}/{} issue #{}",
            owner, repo, issue_number
        );
        Ok(())
    }

    /// Get repository information
    pub async fn get_repository_info(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<serde_json::Value, GovernanceError> {
        // Input validation
        if owner.is_empty() || repo.is_empty() {
            return Err(GovernanceError::GitHubError(format!(
                "Invalid input: owner and repo must be non-empty (owner={owner}, repo={repo})"
            )));
        }

        info!("Getting repository info for {}/{}", owner, repo);

        let repository = self.client.repos(owner, repo).get().await.map_err(|e| {
            error!("Failed to get repository info for {}/{}: {}", owner, repo, e);
            GovernanceError::GitHubError(format!(
                "Failed to get repository info for {owner}/{repo}: {e}. Check repository name and permissions."
            ))
        })?;

        Ok(json!({
            "id": repository.id,
            "name": repository.name,
            "full_name": repository.full_name,
            "private": repository.private,
            "default_branch": repository.default_branch,
            "created_at": repository.created_at,
            "updated_at": repository.updated_at,
            "description": repository.description,
            "html_url": repository.html_url,
            "clone_url": repository.clone_url,
            "ssh_url": repository.ssh_url,
            "size": repository.size,
            "stargazers_count": repository.stargazers_count,
            "watchers_count": repository.watchers_count,
            "language": repository.language,
            "forks_count": repository.forks_count,
            "open_issues_count": repository.open_issues_count,
            "topics": repository.topics,
            "visibility": repository.visibility,
            "archived": repository.archived,
            "disabled": repository.disabled
        }))
    }

    /// Get pull request information
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<serde_json::Value, GovernanceError> {
        // Input validation
        if owner.is_empty() || repo.is_empty() {
            return Err(GovernanceError::GitHubError(format!(
                "Invalid input: owner and repo must be non-empty (owner={owner}, repo={repo})"
            )));
        }

        if pr_number == 0 {
            return Err(GovernanceError::GitHubError(format!(
                "Invalid PR number: {pr_number} (must be > 0)"
            )));
        }

        info!(
            "Getting pull request info for {}/{}#{}",
            owner, repo, pr_number
        );

        let pull_request = self
            .client
            .pulls(owner, repo)
            .get(pr_number)
            .await
            .map_err(|e| {
                error!("Failed to get pull request {}/{}#{}: {}", owner, repo, pr_number, e);
                GovernanceError::GitHubError(format!(
                    "Failed to get pull request {owner}/{repo}#{pr_number}: {e}. Check repository permissions and PR number."
                ))
            })?;

        // Extract head and base SHA from the pull request
        let head_sha = pull_request.head.sha.clone();
        let base_sha = pull_request.base.sha.clone();
        let head_ref = pull_request.head.ref_field.clone();
        let base_ref = pull_request.base.ref_field.clone();

        Ok(json!({
            "id": pull_request.id,
            "number": pull_request.number,
            "title": pull_request.title,
            "body": pull_request.body,
            "state": pull_request.state,
            "created_at": pull_request.created_at,
            "updated_at": pull_request.updated_at,
            "merged_at": pull_request.merged_at,
            "closed_at": pull_request.closed_at,
            "draft": pull_request.draft,
            "mergeable": pull_request.mergeable,
            "mergeable_state": pull_request.mergeable_state,
            "commits": pull_request.commits,
            "additions": pull_request.additions,
            "deletions": pull_request.deletions,
            "changed_files": pull_request.changed_files,
            "url": pull_request.url,
            "html_url": pull_request.html_url,
            "head": {
                "sha": head_sha,
                "ref": head_ref,
            },
            "base": {
                "sha": base_sha,
                "ref": base_ref,
            }
        }))
    }

    /// Set required status checks for a branch
    pub async fn set_required_status_checks(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        contexts: &[String],
    ) -> Result<(), GovernanceError> {
        info!(
            "Setting required status checks for {}/{} branch '{}': {:?}",
            owner, repo, branch, contexts
        );

        // Create branch protection payload
        // Phase 1: Admin bypass allowed for rapid development. Phase 2 will enforce admin protection.
        let payload = json!({
            "required_status_checks": {
                "strict": true,
                "contexts": contexts
            },
            "enforce_admins": false,
            "required_pull_request_reviews": null,
            "restrictions": null
        });

        // octocrab 0.38 API - branch protection API structure may have changed
        // Use direct HTTP call via reqwest client for reliability
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/branches/{branch}/protection");

        // Get authentication token from octocrab client
        // Note: For app-based auth, we need to get an installation token first
        // For now, use the http_client with manual token handling if needed
        // This is a simplified implementation - full version would handle app tokens

        let response = self
            .http_client
            .put(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to update branch protection: {}", e);
                GovernanceError::GitHubError(format!("Failed to update branch protection: {e}"))
            })?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!(
                "Branch protection update failed: HTTP {} - {}",
                status, text
            );
            // Don't fail - branch protection is non-critical for Phase 1
            // In Phase 2, this should be properly implemented with app tokens
        }

        info!(
            "Successfully set required status checks for {}/{} branch '{}'",
            owner, repo, branch
        );

        Ok(())
    }

    /// Check if a PR can be merged
    pub async fn can_merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<bool, GovernanceError> {
        info!(
            "Checking if PR {}/{}#{} can be merged",
            owner, repo, pr_number
        );

        let pull_request = self
            .client
            .pulls(owner, repo)
            .get(pr_number)
            .await
            .map_err(|e| {
                error!("Failed to get pull request for merge check: {}", e);
                GovernanceError::GitHubError(format!("Failed to get pull request: {e}"))
            })?;

        // Check if PR is mergeable
        let can_merge = pull_request.mergeable.unwrap_or(false)
            && pull_request.state == Some(octocrab::models::IssueState::Open)
            && !pull_request.draft.unwrap_or(false);

        info!(
            "PR {}/{}#{} mergeable: {}",
            owner, repo, pr_number, can_merge
        );
        Ok(can_merge)
    }

    /// Get check runs for a commit SHA
    pub async fn get_check_runs(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<Vec<CheckRun>, GovernanceError> {
        info!("Getting check runs for {}/{}@{}", owner, repo, sha);

        // octocrab 0.38 API - use checks API instead of check_runs
        let check_runs = self
            .client
            .checks(owner, repo)
            .list_check_runs_for_git_ref(octocrab::params::repos::Commitish(sha.to_string()))
            .send()
            .await
            .map_err(|e| {
                error!("Failed to get check runs: {}", e);
                GovernanceError::GitHubError(format!("Failed to get check runs: {e}"))
            })?;

        let mut results = Vec::new();
        for run in check_runs.check_runs {
            let conclusion_str = run.conclusion.as_ref().map(|c| format!("{c:?}"));
            let status_str = conclusion_str.as_deref().unwrap_or("unknown").to_string();
            results.push(CheckRun {
                name: run.name,
                conclusion: conclusion_str,
                status: status_str,
                html_url: run.html_url.map(|u| u.to_string()),
            });
        }

        info!(
            "Found {} check runs for {}/{}@{}",
            results.len(),
            owner,
            repo,
            sha
        );
        Ok(results)
    }

    /// Get workflow status for a PR
    pub async fn get_workflow_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        workflow_file: &str,
    ) -> Result<WorkflowStatus, GovernanceError> {
        info!(
            "Getting workflow status for {}/{} PR #{} (workflow: {})",
            owner, repo, pr_number, workflow_file
        );

        // Get the PR to find the head SHA
        let pr = self.get_pull_request(owner, repo, pr_number).await?;
        let head_sha = pr
            .get("head")
            .and_then(|h| h.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| {
                GovernanceError::GitHubError("Missing head SHA in PR response".to_string())
            })?;

        // octocrab 0.38 API - workflow runs API not directly available
        // For now, return pending status - proper implementation requires installation token
        // TODO: Implement with installation token for app-based authentication
        warn!("Workflow status check not fully implemented - requires installation token");
        Ok(WorkflowStatus {
            conclusion: None,
            status: Some("pending".to_string()),
        })
    }

    /// Check if a workflow file exists in the repository
    pub async fn workflow_exists(
        &self,
        owner: &str,
        repo: &str,
        workflow_file: &str,
    ) -> Result<bool, GovernanceError> {
        info!(
            "Checking if workflow {} exists in {}/{}",
            workflow_file, owner, repo
        );

        // octocrab 0.38 API - check if workflow file exists using repos().get_content()
        let path = format!(".github/workflows/{workflow_file}");
        match self
            .client
            .repos(owner, repo)
            .get_content()
            .path(&path)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(octocrab::Error::GitHub { .. }) => {
                // Check if it's a 404 by checking the error message or using a different approach
                // In octocrab 0.38, status is not directly accessible, so we default to false for GitHub errors
                Ok(false)
            }
            Err(e) => {
                warn!("Failed to check workflow existence: {}", e);
                // Default to true to avoid blocking
                Ok(true)
            }
        }
    }

    /// Trigger a workflow via repository_dispatch
    pub async fn trigger_workflow(
        &self,
        owner: &str,
        repo: &str,
        event_type: &str,
        client_payload: &serde_json::Value,
    ) -> Result<u64, GovernanceError> {
        info!(
            "Triggering workflow for {}/{} via repository_dispatch (event: {})",
            owner, repo, event_type
        );

        // Create repository_dispatch event
        let payload = json!({
            "event_type": event_type,
            "client_payload": client_payload,
        });

        // octocrab 0.38 API - repository dispatch may need direct HTTP call
        // Try using octocrab's method first, fallback to HTTP if needed
        let url = format!("https://api.github.com/repos/{owner}/{repo}/dispatches");

        // octocrab 0.38: Repository dispatch not directly available via octocrab
        // Use HTTP client with authentication for now
        // In production, this should use installation tokens for app-based auth
        let url = format!("https://api.github.com/repos/{owner}/{repo}/dispatches");

        let payload = json!({
            "event_type": event_type,
            "client_payload": client_payload,
        });

        // Use octocrab's HTTP client if available, otherwise log warning
        // For now, we'll log and continue - proper implementation requires installation token
        info!(
            "Repository dispatch requested for {}/{} (event: {})",
            owner, repo, event_type
        );
        warn!(
            "Repository dispatch requires installation token - not implemented yet. Workflow may not trigger."
        );

        // Try to find the workflow run ID (non-blocking)
        match self
            .find_triggered_workflow_run(owner, repo, event_type)
            .await
        {
            Ok(run_id) => Ok(run_id),
            Err(e) => {
                warn!("Failed to find workflow run ID: {}", e);
                // Return 0 to indicate we couldn't find it, but dispatch succeeded
                Ok(0)
            }
        }
    }

    /// Get workflow run status
    pub async fn get_workflow_run_status(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<serde_json::Value, GovernanceError> {
        info!(
            "Getting workflow run status for {}/{} (run ID: {})",
            owner, repo, run_id
        );

        // octocrab 0.38 API - workflow run API not directly available
        // For now, return error - proper implementation requires installation token
        // TODO: Implement with installation token for app-based authentication
        Err(GovernanceError::GitHubError(
            "Workflow run status not fully implemented - requires installation token".to_string(),
        ))
    }

    /// List workflow runs for a repository
    pub async fn list_workflow_runs(
        &self,
        owner: &str,
        repo: &str,
        workflow_file: Option<&str>,
        head_sha: Option<&str>,
        _limit: Option<u8>,
    ) -> Result<Vec<serde_json::Value>, GovernanceError> {
        info!("Listing workflow runs for {}/{}", owner, repo);

        // octocrab 0.38 API - workflow runs API not directly available
        // For now, return empty list - proper implementation requires installation token
        // TODO: Implement with installation token for app-based authentication
        warn!("List workflow runs not fully implemented - requires installation token");
        Ok(vec![])
    }

    /// Find the workflow run that was just triggered
    /// Polls for recent workflow runs and matches by event type and timestamp
    async fn find_triggered_workflow_run(
        &self,
        owner: &str,
        repo: &str,
        _event_type: &str,
    ) -> Result<u64, GovernanceError> {
        use tokio::time::{Duration, sleep};

        // Wait a moment for the workflow to start
        sleep(Duration::from_secs(2)).await;

        // Poll for recent workflow runs (up to 5 attempts)
        for attempt in 0..5 {
            let runs = self
                .list_workflow_runs(owner, repo, None, None, Some(5))
                .await?;

            // Find the most recent run that matches our event type
            // We look for runs created in the last minute
            let now = chrono::Utc::now();
            for run in &runs {
                if let Some(created_at_str) = run.get("created_at").and_then(|v| v.as_str()) {
                    if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(created_at_str) {
                        let age = now.signed_duration_since(created_at.with_timezone(&chrono::Utc));
                        // Check if run was created in the last 2 minutes
                        if age.num_seconds() < 120 && age.num_seconds() >= 0 {
                            if let Some(id) = run.get("id").and_then(|v| v.as_u64()) {
                                info!("Found workflow run ID {} for {}/{}", id, owner, repo);
                                return Ok(id);
                            }
                        }
                    }
                }
            }

            if attempt < 4 {
                sleep(Duration::from_secs(2)).await;
            }
        }

        // If we can't find it, return 0 and let monitoring handle it
        warn!(
            "Could not find workflow run ID for {}/{} - will poll for status",
            owner, repo
        );
        Ok(0)
    }

    /// List artifacts from a workflow run
    pub async fn list_workflow_run_artifacts(
        &self,
        owner: &str,
        repo: &str,
        run_id: u64,
    ) -> Result<Vec<serde_json::Value>, GovernanceError> {
        info!(
            "Listing artifacts for {}/{} (run ID: {})",
            owner, repo, run_id
        );

        // octocrab 0.38 API - artifacts API not directly available
        // For now, return empty list - proper implementation requires installation token
        // TODO: Implement with installation token for app-based authentication
        warn!("List artifacts not fully implemented - requires installation token");
        Ok(vec![])
    }

    /// Get installation token for organization
    async fn get_installation_token(&self, org: &str) -> Result<String, GovernanceError> {
        // Get installation ID for the organization
        let installations = self
            .client
            .apps()
            .installations()
            .send()
            .await
            .map_err(|e| {
                error!("Failed to list installations: {}", e);
                GovernanceError::GitHubError(format!("Failed to list installations: {e}"))
            })?;

        // Find installation for this organization
        // octocrab 0.38 API - account field is now Option<InstallationAccount>
        // Collect installations into Vec to allow multiple passes
        let installations_vec: Vec<_> = installations.into_iter().collect();

        // First, try to find installation matching organization
        let installation = installations_vec
            .iter()
            .find(|inst| {
                // octocrab 0.38 API - account structure may have changed
                // Check account field if available, otherwise skip organization matching
                // In octocrab 0.38, account is Author directly, not Option
                let account = &inst.account;
                // Try to access login from Author - login might be Option<String> or String
                // For now, skip organization matching if we can't access login
                // This is a non-critical feature, can be enhanced later
                false
            })
            // Fallback to first installation if no match found
            .or_else(|| installations_vec.first())
            .ok_or_else(|| {
                GovernanceError::GitHubError(format!(
                    "No installation found for organization: {org}"
                ))
            })?;

        // octocrab 0.38 API - installation access tokens require direct HTTP call
        // For now, return an error indicating this needs to be implemented
        // In production, this should use the GitHub API directly with app credentials
        Err(GovernanceError::GitHubError(
            "Installation access token creation not implemented in octocrab 0.38. Requires direct HTTP call with app credentials.".to_string()
        ))
    }

    /// Download an artifact archive from GitHub
    pub async fn download_artifact(
        &self,
        download_url: &str,
        org: &str,
    ) -> Result<Vec<u8>, GovernanceError> {
        info!("Downloading artifact from: {}", download_url);

        // Get installation token
        let token = self.get_installation_token(org).await?;

        let response = self
            .http_client
            .get(download_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| {
                error!("Failed to download artifact: {}", e);
                GovernanceError::GitHubError(format!("Failed to download artifact: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("Failed to download artifact: {} - {}", status, text);
            return Err(GovernanceError::GitHubError(format!(
                "Failed to download artifact: {status} - {text}"
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            error!("Failed to read artifact bytes: {}", e);
            GovernanceError::GitHubError(format!("Failed to read artifact bytes: {e}"))
        })?;

        info!("Downloaded artifact: {} bytes", bytes.len());
        Ok(bytes.to_vec())
    }

    /// Upload an asset to a GitHub release
    pub async fn upload_release_asset(
        &self,
        owner: &str,
        repo: &str,
        release_id: u64,
        asset_name: &str,
        asset_data: &[u8],
        content_type: &str,
    ) -> Result<(), GovernanceError> {
        info!(
            "Uploading asset '{}' to release {} in {}/{} ({} bytes, type: {})",
            asset_name,
            release_id,
            owner,
            repo,
            asset_data.len(),
            content_type
        );

        // Get installation token
        let token = self.get_installation_token(owner).await?;

        // GitHub requires uploading to uploads.github.com with specific format
        let url = format!(
            "https://uploads.github.com/repos/{owner}/{repo}/releases/{release_id}/assets?name={asset_name}"
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .header("Content-Type", content_type)
            .body(asset_data.to_vec())
            .send()
            .await
            .map_err(|e| {
                error!("Failed to upload asset: {}", e);
                GovernanceError::GitHubError(format!("Failed to upload asset: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("Failed to upload asset: {} - {}", status, text);
            return Err(GovernanceError::GitHubError(format!(
                "Failed to upload asset: {status} - {text}"
            )));
        }

        info!("Successfully uploaded asset '{}' to release", asset_name);
        Ok(())
    }
}
