//! Enforcement Decision Logger
//!
//! Logs all governance enforcement decisions for audit and debugging

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

use crate::error::GovernanceError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementDecision {
    pub pr_number: i32,
    pub repo: String,
    pub layer: i32,
    pub tier: u32,
    pub combined_requirements: Requirements,
    pub current_state: CurrentState,
    pub would_allow_merge: bool,
    pub dry_run: bool,
    pub timestamp: DateTime<Utc>,
    pub rationale: String,
    pub enforcement_actions: Vec<EnforcementAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirements {
    pub signatures_required: usize,
    pub signatures_total: usize,
    pub review_period_days: i64,
    pub economic_veto_required: bool,
    pub source: String, // "Layer X", "Tier Y", or "Combined Layer X + Tier Y"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentState {
    pub signatures_current: usize,
    pub signatures_signers: Vec<String>,
    pub signatures_pending: Vec<String>,
    pub review_period_met: bool,
    pub review_period_remaining_days: i64,
    pub economic_veto_active: bool,
    pub economic_veto_percent: f64,
    pub emergency_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementAction {
    pub action_type: String,
    pub status: String,
    pub message: String,
    pub dry_run: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone)]
pub struct DecisionLogger {
    pub dry_run_mode: bool,
    log_enforcement_decisions: bool,
    log_path: Option<String>,
}

impl DecisionLogger {
    pub fn new(dry_run_mode: bool, log_enforcement_decisions: bool, log_path: Option<String>) -> Self {
        Self {
            dry_run_mode,
            log_enforcement_decisions,
            log_path,
        }
    }

    /// Log an enforcement decision
    pub fn log_decision(&self, decision: &EnforcementDecision) -> Result<(), GovernanceError> {
        if !self.log_enforcement_decisions {
            return Ok(());
        }

        // Log to console
        self.log_to_console(decision);

        // Log to file if path provided
        if let Some(path) = &self.log_path {
            self.log_to_file(decision, path)?;
        }

        Ok(())
    }

    /// Log to console with appropriate level
    fn log_to_console(&self, decision: &EnforcementDecision) {
        let action = if decision.would_allow_merge { "ALLOW" } else { "BLOCK" };
        let prefix = if decision.dry_run { "[DRY-RUN]" } else { "[ENFORCEMENT]" };
        
        info!(
            "{} Would {} merge for PR #{} (Layer {}, Tier {}) - {}",
            prefix,
            action,
            decision.pr_number,
            decision.layer,
            decision.tier,
            decision.rationale
        );

        // Log detailed enforcement actions
        for action in &decision.enforcement_actions {
            let action_prefix = if action.dry_run { "[DRY-RUN]" } else { "[ACTION]" };
            debug!(
                "{} {}: {} - {}",
                action_prefix,
                action.action_type,
                action.status,
                action.message
            );
        }
    }

    /// Log to file for audit trail
    fn log_to_file(&self, decision: &EnforcementDecision, path: &str) -> Result<(), GovernanceError> {
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to open log file: {}", e)))?;

        let log_entry = serde_json::to_string_pretty(decision)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to serialize decision: {}", e)))?;

        writeln!(file, "{}", log_entry)
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to write to log file: {}", e)))?;

        Ok(())
    }

    /// Create a new enforcement decision
    pub fn create_decision(
        &self,
        pr_number: i32,
        repo: String,
        layer: i32,
        tier: u32,
        requirements: Requirements,
        current_state: CurrentState,
        would_allow_merge: bool,
        rationale: String,
        enforcement_actions: Vec<EnforcementAction>,
    ) -> EnforcementDecision {
        EnforcementDecision {
            pr_number,
            repo,
            layer,
            tier,
            combined_requirements: requirements,
            current_state,
            would_allow_merge,
            dry_run: self.dry_run_mode,
            timestamp: Utc::now(),
            rationale,
            enforcement_actions,
        }
    }

    /// Create an enforcement action
    pub fn create_action(
        &self,
        action_type: String,
        status: String,
        message: String,
    ) -> EnforcementAction {
        EnforcementAction {
            action_type,
            status,
            message,
            dry_run: self.dry_run_mode,
            timestamp: Utc::now(),
        }
    }

    /// Log a status check update
    pub fn log_status_check(
        &self,
        pr_number: i32,
        context: &str,
        state: &str,
        description: &str,
    ) {
        if self.log_enforcement_decisions {
            let prefix = if self.dry_run_mode { "[DRY-RUN]" } else { "[STATUS]" };
            info!(
                "{} Status check for PR #{}: {} - {} ({})",
                prefix, pr_number, context, state, description
            );
        }
    }

    /// Log a merge blocking decision
    pub fn log_merge_decision(
        &self,
        pr_number: i32,
        blocked: bool,
        reason: &str,
    ) {
        if self.log_enforcement_decisions {
            let action = if blocked { "BLOCKED" } else { "ALLOWED" };
            let prefix = if self.dry_run_mode { "[DRY-RUN]" } else { "[MERGE]" };
            info!(
                "{} Merge {} for PR #{}: {}",
                prefix, action, pr_number, reason
            );
        }
    }

    /// Log signature validation
    pub fn log_signature_validation(
        &self,
        pr_number: i32,
        current: usize,
        required: usize,
        total: usize,
        valid: bool,
    ) {
        if self.log_enforcement_decisions {
            let prefix = if self.dry_run_mode { "[DRY-RUN]" } else { "[SIGNATURES]" };
            let status = if valid { "VALID" } else { "INVALID" };
            info!(
                "{} Signature validation for PR #{}: {} ({}/{}/{})",
                prefix, pr_number, status, current, required, total
            );
        }
    }

    /// Log review period check
    pub fn log_review_period_check(
        &self,
        pr_number: i32,
        met: bool,
        remaining_days: i64,
    ) {
        if self.log_enforcement_decisions {
            let prefix = if self.dry_run_mode { "[DRY-RUN]" } else { "[REVIEW]" };
            let status = if met { "MET" } else { "NOT MET" };
            info!(
                "{} Review period for PR #{}: {} ({} days remaining)",
                prefix, pr_number, status, remaining_days
            );
        }
    }

    /// Log economic veto check
    pub fn log_economic_veto_check(
        &self,
        pr_number: i32,
        active: bool,
        percent: f64,
    ) {
        if self.log_enforcement_decisions {
            let prefix = if self.dry_run_mode { "[DRY-RUN]" } else { "[VETO]" };
            let status = if active { "ACTIVE" } else { "INACTIVE" };
            info!(
                "{} Economic veto for PR #{}: {} ({}%)",
                prefix, pr_number, status, percent
            );
        }
    }
}

impl Default for DecisionLogger {
    fn default() -> Self {
        Self::new(false, true, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_decision() {
        let logger = DecisionLogger::new(true, true, None);
        
        let requirements = Requirements {
            signatures_required: 4,
            signatures_total: 5,
            review_period_days: 30,
            economic_veto_required: false,
            source: "Tier 2".to_string(),
        };

        let current_state = CurrentState {
            signatures_current: 2,
            signatures_signers: vec!["alice".to_string(), "bob".to_string()],
            signatures_pending: vec!["charlie".to_string(), "dave".to_string()],
            review_period_met: false,
            review_period_remaining_days: 15,
            economic_veto_active: false,
            economic_veto_percent: 0.0,
            emergency_mode: false,
        };

        let decision = logger.create_decision(
            123,
            "test/repo".to_string(),
            3,
            2,
            requirements,
            current_state,
            false,
            "Insufficient signatures".to_string(),
            vec![],
        );

        assert_eq!(decision.pr_number, 123);
        assert_eq!(decision.dry_run, true);
        assert_eq!(decision.would_allow_merge, false);
    }

    #[test]
    fn test_create_action() {
        let logger = DecisionLogger::new(true, true, None);
        
        let action = logger.create_action(
            "status_check".to_string(),
            "pending".to_string(),
            "Waiting for signatures".to_string(),
        );

        assert_eq!(action.action_type, "status_check");
        assert_eq!(action.dry_run, true);
    }

    #[test]
    fn test_log_to_file() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir.path().join("test.log");
        
        let logger = DecisionLogger::new(true, true, Some(log_path.to_string_lossy().to_string()));
        
        let requirements = Requirements {
            signatures_required: 4,
            signatures_total: 5,
            review_period_days: 30,
            economic_veto_required: false,
            source: "Tier 2".to_string(),
        };

        let current_state = CurrentState {
            signatures_current: 2,
            signatures_signers: vec!["alice".to_string()],
            signatures_pending: vec!["bob".to_string()],
            review_period_met: false,
            review_period_remaining_days: 15,
            economic_veto_active: false,
            economic_veto_percent: 0.0,
            emergency_mode: false,
        };

        let decision = logger.create_decision(
            123,
            "test/repo".to_string(),
            3,
            2,
            requirements,
            current_state,
            false,
            "Test decision".to_string(),
            vec![],
        );

        let result = logger.log_decision(&decision);
        assert!(result.is_ok());
        
        // Verify file was created and contains the decision
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("test/repo"));
        assert!(content.contains("123"));
    }
}