use crate::error::GovernanceError;
use crate::github::client::GitHubClient;
use crate::enforcement::decision_log::DecisionLogger;
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
    pub fn should_block_merge(
        review_period_met: bool,
        signatures_met: bool,
        economic_veto_active: bool,
        tier: u32,
        emergency_mode: bool,
    ) -> Result<bool, GovernanceError> {
        // In emergency mode, only signature threshold matters
        if emergency_mode {
            Ok(!signatures_met)
        } else {
            // Normal mode: check all requirements
            let basic_requirements_met = review_period_met && signatures_met;

            // For Tier 3+ PRs, also check economic node veto
            if tier >= 3 && economic_veto_active {
                Ok(true) // Block merge due to economic node veto
            } else {
                Ok(!basic_requirements_met)
            }
        }
    }

    /// Get detailed reason for merge blocking
    pub fn get_block_reason(
        review_period_met: bool,
        signatures_met: bool,
        economic_veto_active: bool,
        tier: u32,
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

            if tier >= 3 && economic_veto_active {
                reasons
                    .push("Economic node veto active (30%+ hashpower or 40%+ economic activity)");
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
            format!("❌ Merge blocked: {}", reason)
        } else {
            "✅ Governance requirements met - merge allowed".to_string()
        };

        // Add dry-run prefix if in dry-run mode
        let final_description = if self.decision_logger.dry_run_mode {
            format!("[DRY-RUN] {}", description)
        } else {
            description
        };

        // Log the decision
        self.decision_logger.log_merge_decision(
            sha.parse().unwrap_or(0),
            should_block,
            reason,
        );

        if let Some(client) = &self.github_client {
            client
                .post_status_check(
                    owner,
                    repo,
                    sha,
                    state,
                    &final_description,
                    "governance/merge-check",
                )
                .await?;

            info!(
                "Posted merge status for {}/{}@{}: {} - {}",
                owner, repo, sha, state, final_description
            );
        } else {
            warn!("No GitHub client available, cannot post merge status");
        }

        Ok(())
    }

    /// Update merge status when requirements change
    pub async fn update_merge_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        review_period_met: bool,
        signatures_met: bool,
        economic_veto_active: bool,
        tier: u32,
        emergency_mode: bool,
    ) -> Result<(), GovernanceError> {
        let should_block = Self::should_block_merge(
            review_period_met,
            signatures_met,
            economic_veto_active,
            tier,
            emergency_mode,
        )?;

        let reason = Self::get_block_reason(
            review_period_met,
            signatures_met,
            economic_veto_active,
            tier,
            emergency_mode,
        );

        self.post_merge_status(owner, repo, sha, should_block, &reason)
            .await
    }

    /// Check if PR can be merged (GitHub API integration)
    pub async fn check_mergeability(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<bool, GovernanceError> {
        if let Some(client) = &self.github_client {
            client.can_merge_pull_request(owner, repo, pr_number).await
        } else {
            warn!("No GitHub client available, cannot check mergeability");
            Ok(false)
        }
    }

    /// Set required status checks for a repository branch
    pub async fn set_required_checks(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<(), GovernanceError> {
        if let Some(client) = &self.github_client {
            let contexts = vec![
                "governance/merge-check".to_string(),
                "governance/signatures".to_string(),
                "governance/review-period".to_string(),
            ];

            client
                .set_required_status_checks(owner, repo, branch, &contexts)
                .await?;
            info!(
                "Set required status checks for {}/{} branch '{}'",
                owner, repo, branch
            );
        } else {
            warn!("No GitHub client available, cannot set required checks");
        }

        Ok(())
    }

    /// Generate comprehensive merge status message
    pub fn generate_merge_status_message(
        review_period_met: bool,
        signatures_met: bool,
        economic_veto_active: bool,
        tier: u32,
        tier_name: &str,
        emergency_mode: bool,
        review_period_days: i64,
        elapsed_days: i64,
        current_signatures: usize,
        required_signatures: usize,
    ) -> String {
        let mut message = format!("🏛️ Governance Status for Tier {}: {}\n\n", tier, tier_name);

        // Review period status
        if review_period_met {
            message.push_str("✅ Review Period: Met\n");
        } else {
            message.push_str(&format!(
                "❌ Review Period: {} days elapsed, {} days required\n",
                elapsed_days, review_period_days
            ));
        }

        // Signature status
        if signatures_met {
            message.push_str("✅ Signatures: Complete\n");
        } else {
            message.push_str(&format!(
                "❌ Signatures: {}/{} required\n",
                current_signatures, required_signatures
            ));
        }

        // Economic node veto status (Tier 3+)
        if tier >= 3 {
            if economic_veto_active {
                message.push_str("⚠️ Economic Node Veto: Active\n");
            } else {
                message.push_str("✅ Economic Node Veto: Not Active\n");
            }
        }

        // Emergency mode indicator
        if emergency_mode {
            message.push_str("\n🚨 Emergency Mode Active\n");
        }

        // Overall status
        let should_block = Self::should_block_merge(
            review_period_met,
            signatures_met,
            economic_veto_active,
            tier,
            emergency_mode,
        )
        .unwrap_or(true);

        if should_block {
            message.push_str("\n❌ Merge Blocked: Governance requirements not met");
        } else {
            message.push_str("\n✅ Merge Allowed: All governance requirements met");
        }

        message
    }
}
