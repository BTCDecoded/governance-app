use crate::enforcement::decision_log::DecisionLogger;
use crate::error::GovernanceError;
use crate::github::client::GitHubClient;
use tracing::{info, warn};

pub struct MergeBlocker {
    github_client: Option<GitHubClient>,
    decision_logger: DecisionLogger,
}

impl MergeBlocker {
    pub fn new(github_client: Option<GitHubClient>, decision_logger: DecisionLogger) -> Self {
        Self {
            github_client,
            decision_logger,
        }
    }

    /// Determine if merge should be blocked based on governance requirements
    /// Governance is maintainer-only multisig.
    pub fn should_block_merge(
        review_period_met: bool,
        signatures_met: bool,
        emergency_mode: bool,
    ) -> Result<bool, GovernanceError> {
        // In emergency mode, only signature threshold matters
        if emergency_mode {
            Ok(!signatures_met)
        } else {
            // Normal mode: check maintainer signature requirements
            let basic_requirements_met = review_period_met && signatures_met;
            Ok(!basic_requirements_met)
        }
    }

    /// Get detailed reason for merge blocking
    pub fn get_block_reason(
        review_period_met: bool,
        signatures_met: bool,
        emergency_mode: bool,
    ) -> String {
        if emergency_mode {
            if !signatures_met {
                "Emergency mode: Signature threshold not met".to_string()
            } else {
                "Emergency mode: All requirements met".to_string()
            }
        } else {
            let mut reasons = Vec::new();

            if !review_period_met {
                reasons.push("Review period requirement not met");
            }

            if !signatures_met {
                reasons.push("Signature threshold requirement not met");
            }

            if reasons.is_empty() {
                "All governance requirements met".to_string()
            } else {
                format!("Governance requirements not met: {}", reasons.join(", "))
            }
        }
    }

    /// Post status check to GitHub for merge blocking
    pub async fn post_merge_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        should_block: bool,
        reason: &str,
    ) -> Result<(), GovernanceError> {
        let state = if should_block { "failure" } else { "success" };
        let description = if should_block {
            format!("❌ Merge blocked: {reason}")
        } else {
            "✅ Governance requirements met - merge allowed".to_string()
        };

        // Add dry-run prefix if in dry-run mode
        let final_description = if self.decision_logger.dry_run_mode {
            format!("[DRY-RUN] {description}")
        } else {
            description
        };

        if let Some(client) = &self.github_client {
            client
                .post_status_check(
                    owner,
                    repo,
                    sha,
                    state,
                    &final_description,
                    "governance/merge",
                )
                .await?;
            info!(
                "Posted merge status: {} for {}/{}@{}",
                state, owner, repo, sha
            );
        } else {
            warn!("No GitHub client available, skipping status check");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_block_merge_all_requirements_met() {
        let result = MergeBlocker::should_block_merge(
            true,  // review_period_met
            true,  // signatures_met
            false, // emergency_mode
        )
        .unwrap();

        assert!(!result, "Should not block when all requirements met");
    }

    #[test]
    fn test_should_block_merge_review_period_not_met() {
        let result = MergeBlocker::should_block_merge(
            false, // review_period_met
            true,  // signatures_met
            false, // emergency_mode
        )
        .unwrap();

        assert!(result, "Should block when review period not met");
    }

    #[test]
    fn test_should_block_merge_signatures_not_met() {
        let result = MergeBlocker::should_block_merge(
            true,  // review_period_met
            false, // signatures_met
            false, // emergency_mode
        )
        .unwrap();

        assert!(result, "Should block when signatures not met");
    }

    #[test]
    fn test_should_block_merge_emergency_mode_signatures_met() {
        let result = MergeBlocker::should_block_merge(
            false, // review_period_met (ignored in emergency)
            true,  // signatures_met
            true,  // emergency_mode
        )
        .unwrap();

        assert!(
            !result,
            "Should not block in emergency mode when signatures met"
        );
    }

    #[test]
    fn test_should_block_merge_emergency_mode_signatures_not_met() {
        let result = MergeBlocker::should_block_merge(
            true,  // review_period_met (ignored in emergency)
            false, // signatures_met
            true,  // emergency_mode
        )
        .unwrap();

        assert!(
            result,
            "Should block in emergency mode when signatures not met"
        );
    }

    #[test]
    fn test_get_block_reason_all_met() {
        let reason = MergeBlocker::get_block_reason(
            true,  // review_period_met
            true,  // signatures_met
            false, // emergency_mode
        );
        assert_eq!(reason, "All governance requirements met");
    }

    #[test]
    fn test_get_block_reason_review_period() {
        let reason = MergeBlocker::get_block_reason(false, true, false);
        assert!(reason.contains("Review period requirement not met"));
    }

    #[test]
    fn test_get_block_reason_signatures() {
        let reason = MergeBlocker::get_block_reason(true, false, false);
        assert!(reason.contains("Signature threshold requirement not met"));
    }

    #[test]
    fn test_get_block_reason_multiple() {
        let reason = MergeBlocker::get_block_reason(false, false, false);
        assert!(reason.contains("Review period requirement not met"));
        assert!(reason.contains("Signature threshold requirement not met"));
    }

    #[test]
    fn test_get_block_reason_emergency_signatures_met() {
        let reason = MergeBlocker::get_block_reason(false, true, true);
        assert_eq!(reason, "Emergency mode: All requirements met");
    }

    #[test]
    fn test_get_block_reason_emergency_signatures_not_met() {
        let reason = MergeBlocker::get_block_reason(true, false, true);
        assert_eq!(reason, "Emergency mode: Signature threshold not met");
    }

    #[test]
    fn test_merge_blocker_new() {
        let logger = DecisionLogger::new(true, true, None);
        let blocker = MergeBlocker::new(None, logger);
        assert!(blocker.github_client.is_none());
    }
}
