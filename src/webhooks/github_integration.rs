//! GitHub Integration for Status Checks and Merge Blocking
//!
//! Handles posting status checks and updating merge status based on governance requirements

use serde_json::Value;
use tracing::info;

use crate::database::Database;
use crate::enforcement::merge_block::MergeBlocker;
use crate::enforcement::status_checks::StatusCheckGenerator;
use crate::enforcement::decision_log::DecisionLogger;
use crate::error::GovernanceError;
use crate::github::client::GitHubClient;
use crate::validation::review_period::ReviewPeriodValidator;
use crate::validation::threshold::ThresholdValidator;
use crate::validation::tier_classification;
// use crate::economic_nodes::veto::VetoManager;

pub struct GitHubIntegration {
    github_client: GitHubClient,
    database: Database,
    merge_blocker: MergeBlocker,
    decision_logger: DecisionLogger,
}

impl GitHubIntegration {
    pub fn new(github_client: GitHubClient, database: Database, decision_logger: DecisionLogger) -> Self {
        let merge_blocker = MergeBlocker::new(Some(github_client.clone()), decision_logger.clone());
        Self {
            github_client,
            database,
            merge_blocker,
            decision_logger,
        }
    }

    /// Handle pull request opened event
    pub async fn handle_pr_opened(&self, payload: &Value) -> Result<(), GovernanceError> {
        let repo_name = self.extract_repo_name(payload)?;
        let pr_number = self.extract_pr_number(payload)?;
        let head_sha = self.extract_head_sha(payload)?;
        let (owner, repo) = self.parse_repo_name(&repo_name)?;

        info!(
            "Handling PR opened event for {}/{}#{}",
            owner, repo, pr_number
        );

        // Classify PR tier
        let tier = tier_classification::classify_pr_tier(payload).await;
        let tier_name = self.get_tier_name(tier);

        // Post initial status check
        self.post_initial_status_check(&owner, &repo, &head_sha, tier, &tier_name)
            .await?;

        // Set up required status checks for the branch
        self.merge_blocker
            .set_required_checks(&owner, &repo, "main")
            .await?;

        Ok(())
    }

    /// Handle pull request comment event (signature collection)
    pub async fn handle_pr_comment(&self, payload: &Value) -> Result<(), GovernanceError> {
        let repo_name = self.extract_repo_name(payload)?;
        let pr_number = self.extract_pr_number(payload)?;
        let head_sha = self.extract_head_sha(payload)?;
        let (owner, repo) = self.parse_repo_name(&repo_name)?;

        info!(
            "Handling PR comment event for {}/{}#{}",
            owner, repo, pr_number
        );

        // Update status checks based on current state
        self.update_pr_status_checks(&owner, &repo, &head_sha, pr_number as u64, payload)
            .await?;

        Ok(())
    }

    /// Handle pull request updated event
    pub async fn handle_pr_updated(&self, payload: &Value) -> Result<(), GovernanceError> {
        let repo_name = self.extract_repo_name(payload)?;
        let pr_number = self.extract_pr_number(payload)?;
        let head_sha = self.extract_head_sha(payload)?;
        let (owner, repo) = self.parse_repo_name(&repo_name)?;

        info!(
            "Handling PR updated event for {}/{}#{}",
            owner, repo, pr_number
        );

        // Update all status checks
        self.update_pr_status_checks(&owner, &repo, &head_sha, pr_number as u64, payload)
            .await?;

        Ok(())
    }

    /// Post initial status check when PR is opened
    async fn post_initial_status_check(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        tier: u32,
        tier_name: &str,
    ) -> Result<(), GovernanceError> {
        let status_message = format!(
            "🔍 Governance: Analyzing PR\n\
            Tier {}: {}\n\
            Review period and signature requirements will be checked...",
            tier, tier_name
        );

        self.github_client
            .post_status_check(
                owner,
                repo,
                sha,
                "pending",
                &status_message,
                "governance/analysis",
            )
            .await?;

        Ok(())
    }

    /// Update all status checks for a PR
    async fn update_pr_status_checks(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        pr_number: u64,
        payload: &Value,
    ) -> Result<(), GovernanceError> {
        // Get PR information from database
        let pr_info = self
            .database
            .get_pull_request(owner, pr_number as i32)
            .await?;

        if let Some(pr) = pr_info {
            let layer = pr.layer;
            let tier = tier_classification::classify_pr_tier(payload).await;
            let tier_name = self.get_tier_name(tier);

        // Get combined requirements (Layer + Tier)
        let (sigs_req, sigs_total, review_days) = 
            ThresholdValidator::get_combined_requirements(layer, tier);
        let _source = ThresholdValidator::get_requirement_source(layer, tier);

        // Check review period
        let review_period_met = self.check_review_period(&pr, review_days).await?;
        let review_period_status = self.generate_review_period_status(&pr, review_days).await?;

        // Check signatures
        let (signatures_met, signature_status) = self.check_signatures(&pr, sigs_req, sigs_total).await?;

        // Check economic node veto (Tier 3+)
        let (economic_veto_active, economic_veto_status) = if tier >= 3 {
            self.check_economic_veto(pr.id).await?
        } else {
            (false, String::new())
        };

            // Post individual status checks
            self.post_review_period_status(owner, repo, sha, &review_period_status)
                .await?;
            self.post_signature_status(owner, repo, sha, &signature_status)
                .await?;

            if tier >= 3 {
                self.post_economic_veto_status(owner, repo, sha, &economic_veto_status)
                    .await?;
            }

            // Post combined status
            self.post_combined_status(
                owner,
                repo,
                sha,
                layer,
                tier,
                &tier_name,
                review_period_met,
                signatures_met,
                economic_veto_active,
                &review_period_status,
                &signature_status,
                &economic_veto_status,
            )
            .await?;

            // Update merge blocking status
            self.merge_blocker
                .update_merge_status(
                    owner,
                    repo,
                    sha,
                    review_period_met,
                    signatures_met,
                    economic_veto_active,
                    tier,
                    false, // emergency_mode
                )
                .await?;
        }

        Ok(())
    }

        /// Check review period requirements
        async fn check_review_period(
            &self,
            pr: &crate::database::models::PullRequest,
            required_days: i64,
        ) -> Result<bool, GovernanceError> {
            let opened_at = pr.opened_at;
            Ok(ReviewPeriodValidator::validate_review_period(opened_at, required_days, false).is_ok())
        }

        /// Generate review period status message
        async fn generate_review_period_status(
            &self,
            pr: &crate::database::models::PullRequest,
            required_days: i64,
        ) -> Result<String, GovernanceError> {
            let opened_at = pr.opened_at;
            Ok(StatusCheckGenerator::generate_review_period_status(
                opened_at,
                required_days,
                false,
            ))
        }

        /// Check signature requirements
        async fn check_signatures(
            &self,
            _pr: &crate::database::models::PullRequest,
            required: usize,
            total: usize,
        ) -> Result<(bool, String), GovernanceError> {
            // TODO: Get actual signature count from database
            let current_signatures = 0; // Placeholder
            let signers = vec![]; // Placeholder
            let pending = vec![]; // Placeholder

            let signatures_met = current_signatures >= required;
            let status = StatusCheckGenerator::generate_signature_status(
                current_signatures,
                required,
                total,
                &signers,
                &pending,
            );

            Ok((signatures_met, status))
        }

    /// Check economic node veto status
    async fn check_economic_veto(&self, _pr_id: i32) -> Result<(bool, String), GovernanceError> {
        // let veto_manager = VetoManager::new(self.database.pool().clone());
        // let threshold = veto_manager.check_veto_threshold(pr_id).await?;
        // let statistics = veto_manager.get_veto_statistics(pr_id).await?;

        // For now, return mock data
        let veto_active = false;
        let status = "Economic node veto: No veto signals received".to_string();

        Ok((veto_active, status))
    }

        /// Post review period status check
        async fn post_review_period_status(
            &self,
            owner: &str,
            repo: &str,
            sha: &str,
            status: &str,
        ) -> Result<(), GovernanceError> {
            let state = if status.contains("✅") {
                "success"
            } else {
                "pending"
            };

            // Log the status check
            self.decision_logger.log_status_check(
                sha.parse().unwrap_or(0),
                "governance/review-period",
                state,
                status,
            );

            self.github_client
                .post_status_check(owner, repo, sha, state, status, "governance/review-period")
                .await
        }

    /// Post signature status check
    async fn post_signature_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        status: &str,
    ) -> Result<(), GovernanceError> {
        let state = if status.contains("✅") {
            "success"
        } else {
            "pending"
        };

        // Log the status check
        self.decision_logger.log_status_check(
            sha.parse().unwrap_or(0),
            "governance/signatures",
            state,
            status,
        );

        self.github_client
            .post_status_check(owner, repo, sha, state, status, "governance/signatures")
            .await
    }

    /// Post economic veto status check
    async fn post_economic_veto_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        status: &str,
    ) -> Result<(), GovernanceError> {
        let state = if status.contains("✅") {
            "success"
        } else {
            "failure"
        };

        self.github_client
            .post_status_check(owner, repo, sha, state, status, "governance/economic-veto")
            .await
    }

    /// Post combined status check
    async fn post_combined_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        _layer: i32,
        tier: u32,
        tier_name: &str,
        review_period_met: bool,
        signatures_met: bool,
        economic_veto_active: bool,
        review_period_status: &str,
        signature_status: &str,
        economic_veto_status: &str,
    ) -> Result<(), GovernanceError> {
        let status = StatusCheckGenerator::generate_detailed_status(
            tier,
            tier_name,
            review_period_met,
            signatures_met,
            economic_veto_active,
            review_period_status,
            signature_status,
            economic_veto_status,
            Some("https://github.com/BTCDecoded/governance"),
        );

        let state = if review_period_met && signatures_met && !economic_veto_active {
            "success"
        } else {
            "failure"
        };

        self.github_client
            .post_status_check(owner, repo, sha, state, &status, "governance/combined")
            .await
    }

    /// Extract repository name from payload
    fn extract_repo_name(&self, payload: &Value) -> Result<String, GovernanceError> {
        payload
            .get("repository")
            .and_then(|r| r.get("full_name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GovernanceError::WebhookError("Missing repository name".to_string()))
    }

    /// Extract PR number from payload
    fn extract_pr_number(&self, payload: &Value) -> Result<i32, GovernanceError> {
        payload
            .get("pull_request")
            .and_then(|pr| pr.get("number"))
            .and_then(|n| n.as_i64())
            .map(|n| n as i32)
            .ok_or_else(|| GovernanceError::WebhookError("Missing PR number".to_string()))
    }

    /// Extract head SHA from payload
    fn extract_head_sha(&self, payload: &Value) -> Result<String, GovernanceError> {
        payload
            .get("pull_request")
            .and_then(|pr| pr.get("head"))
            .and_then(|h| h.get("sha"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| GovernanceError::WebhookError("Missing head SHA".to_string()))
    }

    /// Parse repository name into owner and repo
    fn parse_repo_name(&self, repo_name: &str) -> Result<(String, String), GovernanceError> {
        let parts: Vec<&str> = repo_name.split('/').collect();
        if parts.len() != 2 {
            return Err(GovernanceError::WebhookError(
                "Invalid repository name format".to_string(),
            ));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    /// Get tier name from tier number
    fn get_tier_name(&self, tier: u32) -> &'static str {
        match tier {
            1 => "Routine Maintenance",
            2 => "Feature Changes",
            3 => "Consensus-Adjacent",
            4 => "Emergency Actions",
            5 => "Governance Changes",
            _ => "Unknown",
        }
    }
}
