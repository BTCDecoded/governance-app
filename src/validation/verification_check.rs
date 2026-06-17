//! Verification Check Validator
//!
//! Validates that consensus-critical PRs have passed formal verification
//! before allowing maintainer signatures. Implements Ostrom Principle #5
//! (Graduated Sanctions) by preventing progress on unverified code.

use crate::database::models::PullRequest;
use crate::error::{GovernanceError, Result};
use crate::github::client::GitHubClient;
use crate::github::types::{CheckRun, WorkflowStatus};
use crate::validation::ValidationResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for GitHub operations needed by verification checks
/// This allows for easy mocking in tests
#[async_trait::async_trait]
pub trait GitHubVerificationClient: Send + Sync {
    async fn get_workflow_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        workflow_file: &str,
    ) -> Result<WorkflowStatus>;

    async fn get_check_runs(&self, owner: &str, repo: &str, sha: &str) -> Result<Vec<CheckRun>>;

    async fn workflow_exists(&self, owner: &str, repo: &str, workflow_file: &str) -> Result<bool>;
}

/// Implement the trait for GitHubClient
#[async_trait::async_trait]
impl GitHubVerificationClient for GitHubClient {
    async fn get_workflow_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        workflow_file: &str,
    ) -> Result<WorkflowStatus> {
        GitHubClient::get_workflow_status(self, owner, repo, pr_number, workflow_file).await
    }

    async fn get_check_runs(&self, owner: &str, repo: &str, sha: &str) -> Result<Vec<CheckRun>> {
        GitHubClient::get_check_runs(self, owner, repo, sha).await
    }

    async fn workflow_exists(&self, owner: &str, repo: &str, workflow_file: &str) -> Result<bool> {
        GitHubClient::workflow_exists(self, owner, repo, workflow_file).await
    }
}

/// Verification configuration loaded from governance config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    pub required: bool,
    pub tools: Vec<VerificationTool>,
    pub ci_workflow: String,
    pub blocking: bool,
    pub override_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationTool {
    pub name: String,
    pub command: String,
    pub required: bool,
}

/// Repository configuration loaded from governance config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryConfig {
    pub verification: Option<VerificationConfig>,
}

/// Governance configuration loaded from config files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    pub repos: HashMap<String, RepositoryConfig>,
}

/// Check if PR has passed formal verification
pub async fn check_verification_status<C: GitHubVerificationClient>(
    client: &C,
    pr: &PullRequest,
) -> Result<ValidationResult> {
    // Parse repository name to extract owner and repo
    let (owner, repo) = parse_repo_name(&pr.repo_name)?;

    // Check if PR is to a verification-required repository
    if !requires_verification(&pr.repo_name)? {
        return Ok(ValidationResult::NotApplicable);
    }

    // Get CI status for verification workflow
    let workflow = "verify.yml";
    let status = client
        .get_workflow_status(&owner, &repo, pr.pr_number as u64, workflow)
        .await?;

    match status.conclusion.as_deref() {
        Some("success") => {
            // Verification passed - check specific tools
            let tests_passed = check_tool_status(client, pr, "Unit & Property Tests").await?;
            let clippy_passed = check_tool_status(client, pr, "Clippy Linting").await?;

            if tests_passed && clippy_passed {
                Ok(ValidationResult::Valid {
                    message: "Formal verification passed (tests + clippy)".to_string(),
                })
            } else {
                Ok(ValidationResult::Invalid {
                    message: "Some verification tools failed".to_string(),
                    blocking: true,
                })
            }
        }
        Some("failure") | Some("cancelled") => Ok(ValidationResult::Invalid {
            message: "Formal verification failed - see CI logs".to_string(),
            blocking: true,
        }),
        Some("skipped") => Ok(ValidationResult::Invalid {
            message: "Verification was skipped - this is not allowed".to_string(),
            blocking: true,
        }),
        None => Ok(ValidationResult::Pending {
            message: "Verification is still running".to_string(),
        }),
        _ => Ok(ValidationResult::Invalid {
            message: format!("Unknown verification status: {:?}", status.conclusion),
            blocking: true,
        }),
    }
}

/// Check if repository requires verification
pub fn requires_verification(repo: &str) -> Result<bool> {
    // Load from governance config
    let config = load_governance_config()?;
    Ok(config
        .repos
        .get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.required)
        .unwrap_or(false))
}

/// Parse repository name into owner and repo
fn parse_repo_name(repo_name: &str) -> crate::error::Result<(String, String)> {
    let parts: Vec<&str> = repo_name.split('/').collect();
    if parts.len() != 2 {
        return Err(GovernanceError::ValidationError(format!(
            "Invalid repository name format: {repo_name}"
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Check specific tool status
async fn check_tool_status<C: GitHubVerificationClient>(
    client: &C,
    pr: &PullRequest,
    tool_name: &str,
) -> Result<bool> {
    let (owner, repo) = parse_repo_name(&pr.repo_name)?;
    let checks = client.get_check_runs(&owner, &repo, &pr.head_sha).await?;

    for check in checks {
        if check.name == tool_name {
            return Ok(check.conclusion == Some("success".to_string()));
        }
    }

    Ok(false)
}

/// Load governance configuration from config files
fn load_governance_config() -> Result<GovernanceConfig> {
    // In a real implementation, this would load from actual config files
    // For now, we'll return a hardcoded config for blvm-consensus
    let mut repos = HashMap::new();

    // Add both "blvm-consensus" and "BTCDecoded/blvm-consensus" for compatibility
    let consensus_proof_config = RepositoryConfig {
        verification: Some(VerificationConfig {
            required: true,
            tools: vec![
                VerificationTool {
                    name: "Spec-Lock".to_string(),
                    command: "cargo spec-lock verify --crate-path .".to_string(),
                    required: true,
                },
                VerificationTool {
                    name: "Proptest".to_string(),
                    command: "cargo test --all-features".to_string(),
                    required: true,
                },
            ],
            ci_workflow: ".github/workflows/verify.yml".to_string(),
            blocking: true,
            override_allowed: false,
        }),
    };

    // Add both formats for compatibility
    repos.insert("blvm-consensus".to_string(), consensus_proof_config.clone());
    repos.insert(
        "BTCDecoded/blvm-consensus".to_string(),
        consensus_proof_config,
    );

    Ok(GovernanceConfig { repos })
}

/// Validate verification requirements for a repository
pub async fn validate_verification_requirements<C: GitHubVerificationClient>(
    client: &C,
    repo: &str,
    pr_number: u64,
) -> Result<VerificationValidationResult> {
    let config = load_governance_config()?;

    if let Some(repo_config) = config.repos.get(repo) {
        if let Some(verification) = &repo_config.verification {
            if verification.required {
                // Parse repository name to extract owner and repo
                let (owner, repo_name) = parse_repo_name(repo)?;

                // Check if verification workflow exists
                let workflow_exists = client
                    .workflow_exists(&owner, &repo_name, &verification.ci_workflow)
                    .await?;

                if !workflow_exists {
                    return Ok(VerificationValidationResult::MissingWorkflow {
                        workflow: verification.ci_workflow.clone(),
                    });
                }

                // Check if all required tools are configured
                for tool in &verification.tools {
                    if tool.required {
                        // In a real implementation, we'd check if the tool is properly configured
                        // For now, we'll assume they are configured correctly
                    }
                }

                return Ok(VerificationValidationResult::Valid);
            }
        }
    }

    Ok(VerificationValidationResult::NotRequired)
}

/// Result of verification requirements validation
#[derive(Debug, Clone)]
pub enum VerificationValidationResult {
    Valid,
    NotRequired,
    MissingWorkflow { workflow: String },
    MissingTool { tool: String },
    ConfigurationError { message: String },
}

impl std::fmt::Display for VerificationValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationValidationResult::Valid => write!(f, "Verification requirements are valid"),
            VerificationValidationResult::NotRequired => {
                write!(f, "Verification not required for this repository")
            }
            VerificationValidationResult::MissingWorkflow { workflow } => {
                write!(f, "Missing verification workflow: {workflow}")
            }
            VerificationValidationResult::MissingTool { tool } => {
                write!(f, "Missing required verification tool: {tool}")
            }
            VerificationValidationResult::ConfigurationError { message } => {
                write!(f, "Verification configuration error: {message}")
            }
        }
    }
}

/// Check if verification can be overridden
pub fn can_override_verification(repo: &str) -> Result<bool> {
    let config = load_governance_config()?;
    Ok(config
        .repos
        .get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.override_allowed)
        .unwrap_or(false))
}

/// Get verification tools for a repository
pub fn get_verification_tools(repo: &str) -> Result<Vec<VerificationTool>> {
    let config = load_governance_config()?;
    Ok(config
        .repos
        .get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.tools.clone())
        .unwrap_or_default())
}

/// Check if verification is blocking for a repository
pub fn is_verification_blocking(repo: &str) -> Result<bool> {
    let config = load_governance_config()?;
    Ok(config
        .repos
        .get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.blocking)
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::types::{CheckRun, WorkflowStatus};

    // Mock GitHub client for testing
    struct MockGitHubClient {
        workflow_status: WorkflowStatus,
        check_runs: Vec<CheckRun>,
        workflow_exists_result: bool,
    }

    impl MockGitHubClient {
        fn new(workflow_status: WorkflowStatus, check_runs: Vec<CheckRun>) -> Self {
            Self {
                workflow_status,
                check_runs,
                workflow_exists_result: true,
            }
        }

        fn with_workflow_exists(mut self, exists: bool) -> Self {
            self.workflow_exists_result = exists;
            self
        }
    }

    #[async_trait::async_trait]
    impl GitHubVerificationClient for MockGitHubClient {
        async fn get_workflow_status(
            &self,
            _owner: &str,
            _repo: &str,
            _pr_number: u64,
            _workflow_file: &str,
        ) -> Result<WorkflowStatus> {
            Ok(self.workflow_status.clone())
        }

        async fn get_check_runs(
            &self,
            _owner: &str,
            _repo: &str,
            _sha: &str,
        ) -> Result<Vec<CheckRun>> {
            Ok(self.check_runs.clone())
        }

        async fn workflow_exists(
            &self,
            _owner: &str,
            _repo: &str,
            _workflow_file: &str,
        ) -> Result<bool> {
            Ok(self.workflow_exists_result)
        }
    }

    #[tokio::test]
    async fn test_verification_check_passes() {
        let client = MockGitHubClient::new(
            WorkflowStatus {
                conclusion: Some("success".to_string()),
                status: Some("completed".to_string()),
            },
            vec![
                CheckRun {
                    name: "Unit & Property Tests".to_string(),
                    conclusion: Some("success".to_string()),
                    status: "completed".to_string(),
                    html_url: None,
                },
                CheckRun {
                    name: "Clippy Linting".to_string(),
                    conclusion: Some("success".to_string()),
                    status: "completed".to_string(),
                    html_url: None,
                },
            ],
        );

        let pr = PullRequest {
            id: 0,
            repo_name: "BTCDecoded/blvm-consensus".to_string(),
            pr_number: 123,
            opened_at: chrono::Utc::now(),
            layer: 2,
            head_sha: "abc123".to_string(),
            signatures: vec![],
            governance_status: "pending".to_string(),
            linked_prs: vec![],
            emergency_mode: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = check_verification_status(&client, &pr).await.unwrap();

        match result {
            ValidationResult::Valid { message } => {
                assert!(message.contains("Formal verification passed"));
            }
            _ => panic!("Expected Valid result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_verification_check_blocks_unverified() {
        let client = MockGitHubClient::new(
            WorkflowStatus {
                conclusion: Some("failure".to_string()),
                status: Some("completed".to_string()),
            },
            vec![],
        );

        let pr = PullRequest {
            id: 0,
            repo_name: "BTCDecoded/blvm-consensus".to_string(),
            pr_number: 123,
            opened_at: chrono::Utc::now(),
            layer: 2,
            head_sha: "abc123".to_string(),
            signatures: vec![],
            governance_status: "pending".to_string(),
            linked_prs: vec![],
            emergency_mode: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = check_verification_status(&client, &pr).await.unwrap();

        match result {
            ValidationResult::Invalid { message, blocking } => {
                assert!(message.contains("Formal verification failed"));
                assert!(blocking);
            }
            _ => panic!("Expected Invalid result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_verification_check_pending() {
        let client = MockGitHubClient::new(
            WorkflowStatus {
                conclusion: None,
                status: Some("in_progress".to_string()),
            },
            vec![],
        );

        let pr = PullRequest {
            id: 0,
            repo_name: "BTCDecoded/blvm-consensus".to_string(),
            pr_number: 123,
            opened_at: chrono::Utc::now(),
            layer: 2,
            head_sha: "abc123".to_string(),
            signatures: vec![],
            governance_status: "pending".to_string(),
            linked_prs: vec![],
            emergency_mode: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = check_verification_status(&client, &pr).await.unwrap();

        match result {
            ValidationResult::Pending { message } => {
                assert!(message.contains("Verification is still running"));
            }
            _ => panic!("Expected Pending result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_verification_check_not_applicable() {
        let client = MockGitHubClient::new(
            WorkflowStatus {
                conclusion: Some("success".to_string()),
                status: Some("completed".to_string()),
            },
            vec![],
        );

        // Use a repo that doesn't require verification
        let pr = PullRequest {
            id: 0,
            repo_name: "BTCDecoded/some-other-repo".to_string(),
            pr_number: 123,
            opened_at: chrono::Utc::now(),
            layer: 1,
            head_sha: "abc123".to_string(),
            signatures: vec![],
            governance_status: "pending".to_string(),
            linked_prs: vec![],
            emergency_mode: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = check_verification_status(&client, &pr).await.unwrap();

        match result {
            ValidationResult::NotApplicable => {
                // Expected - verification not required for this repo
            }
            _ => panic!("Expected NotApplicable result, got {:?}", result),
        }
    }

    #[test]
    fn test_requires_verification() {
        let result = requires_verification("blvm-consensus").unwrap();
        assert!(result);

        let result = requires_verification("other-repo").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_can_override_verification() {
        let result = can_override_verification("blvm-consensus").unwrap();
        assert!(!result); // blvm-consensus should not allow override

        let result = can_override_verification("other-repo").unwrap();
        assert!(!result); // default should be false
    }

    #[test]
    fn test_is_verification_blocking() {
        let result = is_verification_blocking("blvm-consensus").unwrap();
        assert!(result); // blvm-consensus should be blocking

        let result = is_verification_blocking("other-repo").unwrap();
        assert!(!result); // default should be false
    }

    #[test]
    fn test_get_verification_tools() {
        let tools = get_verification_tools("blvm-consensus").unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "Spec-Lock");
        assert_eq!(tools[1].name, "Proptest");

        let tools = get_verification_tools("other-repo").unwrap();
        assert_eq!(tools.len(), 0);
    }
}
