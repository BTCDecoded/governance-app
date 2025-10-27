//! Verification Check Validator
//! 
//! Validates that consensus-critical PRs have passed formal verification
//! before allowing maintainer signatures. Implements Ostrom Principle #5
//! (Graduated Sanctions) by preventing progress on unverified code.

use crate::error::Result;
use crate::github::client::GitHubClient;
use crate::database::models::PullRequest;
use crate::validation::ValidationResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
pub async fn check_verification_status(
    client: &GitHubClient,
    pr: &PullRequest,
) -> Result<ValidationResult> {
    // Check if PR is to a verification-required repository
    if !requires_verification(&pr.repository)? {
        return Ok(ValidationResult::NotApplicable);
    }
    
    // Get CI status for verification workflow
    let workflow = "verify.yml";
    let status = client.get_workflow_status(&pr.repository, pr.number, workflow).await?;
    
    match status.conclusion {
        Some("success") => {
            // Verification passed - check specific tools
            let kani_passed = check_tool_status(client, pr, "Kani Model Checking").await?;
            let proptest_passed = check_tool_status(client, pr, "Unit & Property Tests").await?;
            
            if kani_passed && proptest_passed {
                Ok(ValidationResult::Valid {
                    message: "Formal verification passed (Kani + Proptest)".to_string(),
                })
            } else {
                Ok(ValidationResult::Invalid {
                    message: "Some verification tools failed".to_string(),
                    blocking: true,
                })
            }
        },
        Some("failure") | Some("cancelled") => {
            Ok(ValidationResult::Invalid {
                message: "Formal verification failed - see CI logs".to_string(),
                blocking: true,
            })
        },
        Some("skipped") => {
            Ok(ValidationResult::Invalid {
                message: "Verification was skipped - this is not allowed".to_string(),
                blocking: true,
            })
        },
        None => {
            Ok(ValidationResult::Pending {
                message: "Verification is still running".to_string(),
            })
        },
        _ => {
            Ok(ValidationResult::Invalid {
                message: format!("Unknown verification status: {:?}", status.conclusion),
                blocking: true,
            })
        }
    }
}

/// Check if repository requires verification
fn requires_verification(repo: &str) -> Result<bool> {
    // Load from governance config
    let config = load_governance_config()?;
    Ok(config.repos.get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.required)
        .unwrap_or(false))
}

/// Check specific tool status
async fn check_tool_status(
    client: &GitHubClient,
    pr: &PullRequest,
    tool_name: &str,
) -> Result<bool> {
    let checks = client.get_check_runs(&pr.repository, &pr.head_sha).await?;
    
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
    // For now, we'll return a hardcoded config for consensus-proof
    let mut repos = HashMap::new();
    
    let consensus_proof_config = RepositoryConfig {
        verification: Some(VerificationConfig {
            required: true,
            tools: vec![
                VerificationTool {
                    name: "Kani".to_string(),
                    command: "cargo kani --features verify".to_string(),
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
    
    repos.insert("consensus-proof".to_string(), consensus_proof_config);
    
    Ok(GovernanceConfig { repos })
}

/// Validate verification requirements for a repository
pub async fn validate_verification_requirements(
    client: &GitHubClient,
    repo: &str,
    pr_number: u64,
) -> Result<VerificationValidationResult> {
    let config = load_governance_config()?;
    
    if let Some(repo_config) = config.repos.get(repo) {
        if let Some(verification) = &repo_config.verification {
            if verification.required {
                // Check if verification workflow exists
                let workflow_exists = client.workflow_exists(repo, &verification.ci_workflow).await?;
                
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
            VerificationValidationResult::NotRequired => write!(f, "Verification not required for this repository"),
            VerificationValidationResult::MissingWorkflow { workflow } => {
                write!(f, "Missing verification workflow: {}", workflow)
            },
            VerificationValidationResult::MissingTool { tool } => {
                write!(f, "Missing required verification tool: {}", tool)
            },
            VerificationValidationResult::ConfigurationError { message } => {
                write!(f, "Verification configuration error: {}", message)
            },
        }
    }
}

/// Check if verification can be overridden
pub fn can_override_verification(repo: &str) -> Result<bool> {
    let config = load_governance_config()?;
    Ok(config.repos.get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.override_allowed)
        .unwrap_or(false))
}

/// Get verification tools for a repository
pub fn get_verification_tools(repo: &str) -> Result<Vec<VerificationTool>> {
    let config = load_governance_config()?;
    Ok(config.repos.get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.tools.clone())
        .unwrap_or_default())
}

/// Check if verification is blocking for a repository
pub fn is_verification_blocking(repo: &str) -> Result<bool> {
    let config = load_governance_config()?;
    Ok(config.repos.get(repo)
        .and_then(|r| r.verification.as_ref())
        .map(|v| v.blocking)
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::types::{WorkflowStatus, CheckRun};
    
    // Mock GitHub client for testing
    struct MockGitHubClient {
        workflow_status: WorkflowStatus,
        check_runs: Vec<CheckRun>,
    }
    
    impl MockGitHubClient {
        fn new(workflow_status: WorkflowStatus, check_runs: Vec<CheckRun>) -> Self {
            Self {
                workflow_status,
                check_runs,
            }
        }
    }
    
    #[async_trait::async_trait]
    impl GitHubClient for MockGitHubClient {
        async fn get_workflow_status(&self, _repo: &str, _pr_number: u64, _workflow: &str) -> Result<WorkflowStatus> {
            Ok(self.workflow_status.clone())
        }
        
        async fn get_check_runs(&self, _repo: &str, _sha: &str) -> Result<Vec<CheckRun>> {
            Ok(self.check_runs.clone())
        }
        
        async fn workflow_exists(&self, _repo: &str, _workflow: &str) -> Result<bool> {
            Ok(true)
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
                    name: "Kani Model Checking".to_string(),
                    conclusion: Some("success".to_string()),
                    status: "completed".to_string(),
                },
                CheckRun {
                    name: "Unit & Property Tests".to_string(),
                    conclusion: Some("success".to_string()),
                    status: "completed".to_string(),
                },
            ],
        );
        
        let pr = PullRequest {
            repository: "consensus-proof".to_string(),
            number: 123,
            head_sha: "abc123".to_string(),
            base_sha: "def456".to_string(),
            title: "Test PR".to_string(),
            body: "Test body".to_string(),
            author: "test-author".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = check_verification_status(&client, &pr).await.unwrap();
        
        match result {
            ValidationResult::Valid { message } => {
                assert!(message.contains("Formal verification passed"));
            },
            _ => panic!("Expected Valid result"),
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
            repository: "consensus-proof".to_string(),
            number: 123,
            head_sha: "abc123".to_string(),
            base_sha: "def456".to_string(),
            title: "Test PR".to_string(),
            body: "Test body".to_string(),
            author: "test-author".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = check_verification_status(&client, &pr).await.unwrap();
        
        match result {
            ValidationResult::Invalid { message, blocking } => {
                assert!(message.contains("Formal verification failed"));
                assert!(blocking);
            },
            _ => panic!("Expected Invalid result"),
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
            repository: "consensus-proof".to_string(),
            number: 123,
            head_sha: "abc123".to_string(),
            base_sha: "def456".to_string(),
            title: "Test PR".to_string(),
            body: "Test body".to_string(),
            author: "test-author".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let result = check_verification_status(&client, &pr).await.unwrap();
        
        match result {
            ValidationResult::Pending { message } => {
                assert!(message.contains("Verification is still running"));
            },
            _ => panic!("Expected Pending result"),
        }
    }
    
    #[test]
    fn test_requires_verification() {
        let result = requires_verification("consensus-proof").unwrap();
        assert!(result);
        
        let result = requires_verification("other-repo").unwrap();
        assert!(!result);
    }
    
    #[test]
    fn test_can_override_verification() {
        let result = can_override_verification("consensus-proof").unwrap();
        assert!(!result); // consensus-proof should not allow override
        
        let result = can_override_verification("other-repo").unwrap();
        assert!(!result); // default should be false
    }
    
    #[test]
    fn test_is_verification_blocking() {
        let result = is_verification_blocking("consensus-proof").unwrap();
        assert!(result); // consensus-proof should be blocking
        
        let result = is_verification_blocking("other-repo").unwrap();
        assert!(!result); // default should be false
    }
    
    #[test]
    fn test_get_verification_tools() {
        let tools = get_verification_tools("consensus-proof").unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "Kani");
        assert_eq!(tools[1].name, "Proptest");
        
        let tools = get_verification_tools("other-repo").unwrap();
        assert_eq!(tools.len(), 0);
    }
}
