//! Background job to check for expired governance review cases, time limits, appeals, and mediations
//!
//! This binary is designed to be run by GitHub Actions workflows on a schedule

use blvm_commons::github::client::GitHubClient;
use blvm_commons::governance_review::{
    AppealManager, DeadlineNotificationManager, GovernanceReviewCaseManager,
    GovernanceReviewGitHubIntegration, MediationManager, TimeLimitManager, get_database_url,
    get_github_token, get_governance_repo,
};
use sqlx::SqlitePool;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let check_cases = args.contains(&"--check-cases".to_string());
    let check_time_limits = args.contains(&"--check-time-limits".to_string());
    let check_appeals = args.contains(&"--check-appeals".to_string());
    let check_mediations = args.contains(&"--check-mediations".to_string());
    let notify_deadlines = args.contains(&"--notify-deadlines".to_string());

    // If no specific check specified, run all
    let run_all = !check_cases
        && !check_time_limits
        && !check_appeals
        && !check_mediations
        && !notify_deadlines;

    let database_url = get_database_url();
    let pool = SqlitePool::connect(&database_url).await?;

    // Setup GitHub integration (optional)
    let github_integration =
        if let (Some(token), Some((owner, name))) = (get_github_token(), get_governance_repo()) {
            match GitHubClient::from_token(&token) {
                Ok(client) => {
                    info!("GitHub integration available (token found)");
                    Some(GovernanceReviewGitHubIntegration::new(client, owner, name))
                }
                Err(e) => {
                    error!("Failed to create GitHub client: {}", e);
                    None
                }
            }
        } else {
            info!("GitHub integration not available (no token or repo config)");
            None
        };

    if run_all || check_cases {
        info!("Checking for expired cases...");
        let case_manager = GovernanceReviewCaseManager::new(pool.clone());
        let expired = case_manager.check_expired_cases().await?;
        if !expired.is_empty() {
            info!("Found {} expired cases", expired.len());
            for case_id in expired {
                info!("Expired case: {}", case_id);
            }
        }
    }

    if run_all || check_time_limits {
        info!("Checking for expired time limits...");
        let time_limit_manager = TimeLimitManager::new(pool.clone());
        let expired = time_limit_manager.check_expired_limits().await?;
        if !expired.is_empty() {
            info!("Found {} expired time limits", expired.len());
            for (case_id, limit_type) in expired {
                info!("Expired time limit: case {} - {}", case_id, limit_type);
            }
        }
    }

    if run_all || check_appeals {
        info!("Checking for expired appeals...");
        let appeal_manager = AppealManager::new(pool.clone());
        let expired = appeal_manager.check_expired_appeals().await?;
        if !expired.is_empty() {
            info!("Found {} expired appeals", expired.len());
            for appeal_id in expired {
                info!("Expired appeal: {}", appeal_id);
            }
        }
    }

    if run_all || check_mediations {
        info!("Checking for expired mediations...");
        let mediation_manager = MediationManager::new(pool.clone());
        let expired = mediation_manager.check_expired_mediations().await?;
        if !expired.is_empty() {
            info!("Found {} expired mediations", expired.len());
            for mediation_id in expired {
                info!("Expired mediation: {}", mediation_id);
            }
        }
    }

    if notify_deadlines {
        info!("Checking for approaching deadlines...");
        let deadline_manager = DeadlineNotificationManager::new(pool.clone(), github_integration);
        match deadline_manager.check_and_notify().await {
            Ok(result) => {
                info!(
                    "Deadline notifications: {} cases, {} appeals, {} mediations",
                    result.cases_notified, result.appeals_notified, result.mediations_notified
                );
            }
            Err(e) => {
                error!("Failed to check deadlines: {}", e);
            }
        }
    }

    info!("Background job completed successfully");
    Ok(())
}
