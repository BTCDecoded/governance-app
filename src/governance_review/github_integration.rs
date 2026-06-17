//! GitHub integration for governance review
//!
//! Creates GitHub issues for cases, links to PRs/comments, generates warning files

use crate::error::GovernanceError;
use crate::github::client::GitHubClient;
use crate::governance_review::models::{GovernanceReviewCase, GovernanceReviewWarning};
use chrono::Utc;
use tracing::info;

pub struct GovernanceReviewGitHubIntegration {
    pub github_client: GitHubClient,
    governance_repo_owner: String,
    governance_repo_name: String,
}

impl GovernanceReviewGitHubIntegration {
    pub fn new(
        github_client: GitHubClient,
        governance_repo_owner: String,
        governance_repo_name: String,
    ) -> Self {
        Self {
            github_client,
            governance_repo_owner,
            governance_repo_name,
        }
    }

    /// Create a GitHub issue for a governance review case
    pub async fn create_case_issue(
        &self,
        case: &GovernanceReviewCase,
        subject_username: &str,
        reporter_username: &str,
    ) -> Result<u64, GovernanceError> {
        let title = format!(
            "Governance Review: {} ({})",
            case.case_number, subject_username
        );

        let body = format!(
            r#"# Governance Review Case: {}

**Case Number:** {}
**Subject:** @{}
**Reporter:** @{}
**Type:** {}
**Severity:** {}
**Status:** {}

## Description

{}

## Evidence

```json
{}
```

## Timeline

- **Created:** {}
- **Response Deadline:** {}
- **Resolution Deadline:** {}

## Policy

This case is subject to the [Governance Review Policy](https://github.com/{}/{}/blob/main/governance/config/maintainers/GOVERNANCE_REVIEW_POLICY.md).

**On-Platform Activity Only:** This case only considers on-platform activity. Off-platform activity is explicitly disregarded.

## Next Steps

1. Subject has 30 days to respond
2. Case must be resolved within 180 days
3. Mediation available for non-security issues

---
*This issue was automatically created by the Governance Review System*
"#,
            case.case_number,
            case.case_number,
            subject_username,
            reporter_username,
            case.case_type,
            case.severity,
            case.status,
            case.description,
            serde_json::to_string_pretty(&case.evidence).unwrap_or_default(),
            case.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
            case.response_deadline
                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            case.resolution_deadline
                .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            self.governance_repo_owner,
            self.governance_repo_name,
        );

        // Create issue using octocrab
        // Note: This requires installation token - for now, return placeholder
        // In production, get installation token from GitHubClient
        // TODO: Add installation token support to GitHubClient
        Err(GovernanceError::GitHubError(
            "Issue creation requires installation token - use GitHub Actions workflow instead"
                .to_string(),
        ))
    }

    /// Create a public warning file in governance/warnings/
    pub async fn create_warning_file(
        &self,
        warning: &GovernanceReviewWarning,
        maintainer_username: &str,
        case: &GovernanceReviewCase,
    ) -> Result<String, GovernanceError> {
        let filename = format!(
            "governance/warnings/{}-{}.md",
            warning.issued_at.format("%Y-%m-%d"),
            maintainer_username.to_lowercase()
        );

        let content = format!(
            r#"# Governance Review Warning: {}

**Date:** {}
**Maintainer:** @{}
**Case:** {}
**Warning Level:** {} ({})
**Approved by:** {} maintainers

## Reason

{}

## Case Details

- **Type:** {}
- **Severity:** {}
- **Case Number:** {}

## Improvement Period

{}
"#,
            maintainer_username,
            warning.issued_at.format("%Y-%m-%d"),
            maintainer_username,
            case.case_number,
            warning.warning_level,
            warning.warning_type,
            warning.issued_by_team_approval,
            case.description,
            case.case_type,
            case.severity,
            case.case_number,
            warning
                .improvement_deadline
                .map(|d| format!("**Deadline:** {}", d.format("%Y-%m-%d")))
                .unwrap_or_else(|| "No improvement deadline set".to_string()),
        );

        // Create file in governance repo using file operations
        // Note: This requires a token-based client, not app-based
        // For now, we'll use the file_operations module which needs a token
        // In production, this should use the app's installation token
        // For now, return the filename - actual file creation can be done via workflow
        // TODO: Implement file creation with app installation token

        Ok(filename)
    }

    /// Link a case to a PR or comment
    pub async fn link_case_to_pr(
        &self,
        case_number: &str,
        repo_owner: &str,
        repo_name: &str,
        pr_number: u64,
    ) -> Result<(), GovernanceError> {
        // Add comment to PR linking the case
        let comment = format!(
            r#"This PR is related to governance review case: **{}**

See: https://github.com/{}/{}/issues?q=is:issue+{}"#,
            case_number, self.governance_repo_owner, self.governance_repo_name, case_number
        );

        // Create PR comment
        // Note: This requires installation token
        // For now, log the comment - actual creation should be done via workflow
        info!(
            "PR comment ready for PR #{} - create via workflow",
            pr_number
        );
        Ok(())
    }

    /// Update case issue with status change
    pub async fn update_case_issue(
        &self,
        issue_number: u64,
        case: &GovernanceReviewCase,
    ) -> Result<(), GovernanceError> {
        // Add comment to issue with status update
        let comment = format!(
            r#"## Status Update

**New Status:** {}
**Updated:** {}

{}

---
*Status updated by Governance Review System*"#,
            case.status,
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            case.resolution_reason
                .as_ref()
                .map(|r| format!("**Resolution:** {r}"))
                .unwrap_or_default()
        );

        // Create issue comment
        // Note: This requires installation token
        // For now, log the comment - actual creation should be done via workflow
        info!(
            "Issue comment ready for issue #{} - create via workflow",
            issue_number
        );
        Ok(())
    }

    /// Post a comment to a GitHub issue
    pub async fn post_issue_comment(
        &self,
        issue_number: u64,
        body: &str,
    ) -> Result<(), GovernanceError> {
        self.github_client
            .post_issue_comment(
                &self.governance_repo_owner,
                &self.governance_repo_name,
                issue_number,
                body,
            )
            .await
    }
}
