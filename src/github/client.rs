use octocrab::Octocrab;
use serde_json::json;
use tracing::{error, info};

use crate::error::GovernanceError;

#[derive(Clone)]
pub struct GitHubClient {
    client: Octocrab,
    app_id: u64,
}

impl GitHubClient {
    pub fn new(app_id: u64, private_key_path: &str) -> Result<Self, GovernanceError> {
        let key = std::fs::read_to_string(private_key_path).map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read private key: {}", e))
        })?;

        let client = Octocrab::builder()
            .app(
                app_id.into(),
                jsonwebtoken::EncodingKey::from_rsa_pem(key.as_bytes()).map_err(|e| {
                    GovernanceError::GitHubError(format!("Failed to parse private key: {}", e))
                })?,
            )
            .build()
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to create GitHub client: {}", e))
            })?;

        Ok(Self { client, app_id })
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
        info!(
            "Posting status check for {}/{}@{}: {} - {} ({})",
            owner, repo, sha, state, description, context
        );

        // Convert state to GitHub API format
        let github_state = match state {
            "success" => "success",
            "failure" => "failure",
            "pending" => "pending",
            "error" => "error",
            _ => "error",
        };

        // Create status check payload
        let payload = json!({
            "state": github_state,
            "description": description,
            "context": context,
            "target_url": format!("https://github.com/{}/{}/actions", owner, repo)
        });

        // Post status check via GitHub API
        self.client
            .repos(owner, repo)
            .create_status(sha)
            .body(&payload)
            .send()
            .await
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to post status check: {}", e))
            })?;

        info!(
            "Successfully posted status check: {}/{}@{} - {}: {} ({})",
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

        // For now, just log the status check update - full implementation will be added later
        info!(
            "Status check would be updated: {} - {} ({})",
            state, description, check_run_id
        );

        // TODO: Implement actual GitHub API call when octocrab issues are resolved
        Ok(())
    }

    /// Get repository information
    pub async fn get_repository_info(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<serde_json::Value, GovernanceError> {
        info!("Getting repository info for {}/{}", owner, repo);

        let repository = self.client.repos(owner, repo).get().await.map_err(|e| {
            error!("Failed to get repository info: {}", e);
            GovernanceError::GitHubError(format!("Failed to get repository info: {}", e))
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
                error!("Failed to get pull request info: {}", e);
                GovernanceError::GitHubError(format!("Failed to get pull request info: {}", e))
            })?;

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
            "html_url": pull_request.html_url
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
        let payload = json!({
            "required_status_checks": {
                "strict": true,
                "contexts": contexts
            },
            "enforce_admins": false,
            "required_pull_request_reviews": null,
            "restrictions": null
        });

        // Update branch protection via GitHub API
        self.client
            .repos(owner, repo)
            .branches(branch)
            .protection()
            .put(&payload)
            .await
            .map_err(|e| {
                GovernanceError::GitHubError(format!("Failed to set required status checks: {}", e))
            })?;

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
                GovernanceError::GitHubError(format!("Failed to get pull request: {}", e))
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
}
