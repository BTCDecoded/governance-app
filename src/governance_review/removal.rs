//! Maintainer removal process
//!
//! Implements policy removal process:
//! - Level 3: Removal (6-of-7 team + 4-of-7 teams approval)
//! - Deactivates maintainer key
//! - Handles emergency removal

use crate::governance_review::models::policy;
use chrono::Utc;
use sqlx::{Row, SqlitePool};

pub struct RemovalManager {
    pool: SqlitePool,
}

impl RemovalManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Remove a maintainer (Level 3 sanction)
    /// Policy: Requires 6-of-7 team + 4-of-7 teams approval
    pub async fn remove_maintainer(
        &self,
        case_id: i32,
        maintainer_id: i32,
        team_approvals: Vec<i32>, // Maintainer IDs from the team who approved
        teams_approval_count: i32, // Number of teams that approved (4-of-7)
    ) -> Result<(), sqlx::Error> {
        // Policy: Must have at least 6 approvals from team
        if team_approvals.len() < policy::REMOVAL_TEAM_THRESHOLD as usize {
            return Err(sqlx::Error::RowNotFound); // Or custom error
        }

        // Policy: Must have at least 4 teams approval
        if teams_approval_count < policy::REMOVAL_TEAMS_THRESHOLD {
            return Err(sqlx::Error::RowNotFound);
        }

        // Start transaction for atomic removal process
        let mut tx = self.pool.begin().await?;

        // Record sanction approvals
        for approver_id in &team_approvals {
            sqlx::query(
                r#"
                INSERT INTO governance_review_sanction_approvals 
                (case_id, maintainer_id, sanction_type)
                VALUES (?, ?, 'removal')
                "#,
            )
            .bind(case_id)
            .bind(approver_id)
            .execute(&mut *tx)
            .await?;
        }

        // Update case status
        sqlx::query(
            r#"
            UPDATE governance_review_cases
            SET status = 'removed', resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(case_id)
        .execute(&mut *tx)
        .await?;

        // Deactivate maintainer key
        sqlx::query(
            r#"
            UPDATE maintainers
            SET active = false, last_updated = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(maintainer_id)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(())
    }

    /// Deactivate a maintainer's key
    /// This prevents them from signing PRs
    pub async fn deactivate_maintainer(&self, maintainer_id: i32) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE maintainers
            SET active = false, last_updated = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(maintainer_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Emergency removal (by emergency keyholders)
    /// Policy: 4-of-5 emergency keyholders can immediately deactivate
    pub async fn emergency_remove(
        &self,
        maintainer_id: i32,
        emergency_keyholder_approvals: Vec<i32>, // Emergency keyholder IDs who approved
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        // Policy: Must have at least 4 emergency keyholder approvals
        if emergency_keyholder_approvals.len() < 4 {
            return Err(sqlx::Error::RowNotFound);
        }

        // Immediately deactivate
        self.deactivate_maintainer(maintainer_id).await?;

        // Create emergency removal case for tracking
        // (This would be handled by the case manager, but we log it here)
        sqlx::query(
            r#"
            INSERT INTO governance_review_cases
            (case_number, subject_maintainer_id, reporter_maintainer_id,
             case_type, severity, status, description, evidence, on_platform,
             created_at, resolved_at, resolution_reason)
            VALUES (?, ?, ?, 'security_violation', 'gross_misconduct', 'removed', ?, '{}', true, ?, ?, ?)
            "#,
        )
        .bind(format!("GR-EMERGENCY-{}", Utc::now().format("%Y%m%d-%H%M%S")))
        .bind(maintainer_id)
        .bind(emergency_keyholder_approvals[0]) // First approver as reporter
        .bind(reason)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind("Emergency removal by emergency keyholders")
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if maintainer is active
    pub async fn is_maintainer_active(&self, maintainer_id: i32) -> Result<bool, sqlx::Error> {
        let row = sqlx::query("SELECT active FROM maintainers WHERE id = ?")
            .bind(maintainer_id)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(r.get::<bool, _>(0)),
            None => Ok(false),
        }
    }
}
