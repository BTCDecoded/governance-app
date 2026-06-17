//! Graduated sanctions implementation
//!
//! Implements policy graduated sanctions:
//! - Level 1: Private warning (4-of-7 team)
//! - Level 2: Public warning (5-of-7 team, 90-day improvement)
//! - Level 3: Removal (6-of-7 team + 4-of-7 teams)

use crate::governance_review::models::{GovernanceReviewWarning, policy};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};

pub struct SanctionManager {
    pool: SqlitePool,
}

impl SanctionManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Issue a private warning (Level 1)
    /// Policy: Requires 4-of-7 team approval
    pub async fn issue_private_warning(
        &self,
        case_id: i32,
        maintainer_id: i32,
        approvals: Vec<i32>, // Maintainer IDs who approved
    ) -> Result<GovernanceReviewWarning, sqlx::Error> {
        // Policy: Must have at least 4 approvals
        if approvals.len() < policy::PRIVATE_WARNING_THRESHOLD as usize {
            return Err(sqlx::Error::RowNotFound); // Or custom error
        }

        // Start transaction for atomic approval + warning creation
        let mut tx = self.pool.begin().await?;

        // Record approvals
        for approver_id in &approvals {
            sqlx::query(
                r#"
            INSERT INTO governance_review_sanction_approvals 
            (case_id, maintainer_id, sanction_type)
            VALUES (?, ?, 'private_warning')
            "#,
            )
            .bind(case_id)
            .bind(approver_id)
            .execute(&mut *tx)
            .await?;
        }

        // Create warning
        let approval_count = approvals.len() as i32;
        let warning_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_warnings
            (case_id, maintainer_id, warning_level, warning_type, issued_by_team_approval)
            VALUES (?, ?, 1, 'private_warning', ?)
            RETURNING id
            "#,
        )
        .bind(case_id)
        .bind(maintainer_id)
        .bind(approval_count)
        .fetch_one(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        self.get_warning_by_id(warning_id).await
    }

    /// Issue a public warning (Level 2)
    /// Policy: Requires 5-of-7 team approval, 90-day improvement period
    pub async fn issue_public_warning(
        &self,
        case_id: i32,
        maintainer_id: i32,
        approvals: Vec<i32>, // Maintainer IDs who approved
        warning_file_path: String,
    ) -> Result<GovernanceReviewWarning, sqlx::Error> {
        // Policy: Must have at least 5 approvals
        if approvals.len() < policy::PUBLIC_WARNING_THRESHOLD as usize {
            return Err(sqlx::Error::RowNotFound);
        }

        // Start transaction for atomic approval + warning creation
        let mut tx = self.pool.begin().await?;

        // Record approvals
        for approver_id in &approvals {
            sqlx::query(
                r#"
            INSERT INTO governance_review_sanction_approvals 
            (case_id, maintainer_id, sanction_type)
            VALUES (?, ?, 'public_warning')
            "#,
            )
            .bind(case_id)
            .bind(approver_id)
            .execute(&mut *tx)
            .await?;
        }

        // Policy: 90-day improvement period
        let improvement_deadline = Utc::now() + Duration::days(policy::IMPROVEMENT_PERIOD_DAYS);

        // Create warning
        let approval_count = approvals.len() as i32;
        let warning_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_warnings
            (case_id, maintainer_id, warning_level, warning_type, 
             issued_by_team_approval, improvement_deadline, warning_file_path)
            VALUES (?, ?, 2, 'public_warning', ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(case_id)
        .bind(maintainer_id)
        .bind(approval_count)
        .bind(improvement_deadline)
        .bind(&warning_file_path)
        .fetch_one(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        self.get_warning_by_id(warning_id).await
    }

    /// Get warning by ID
    pub async fn get_warning_by_id(
        &self,
        warning_id: i32,
    ) -> Result<GovernanceReviewWarning, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, case_id, maintainer_id, warning_level, warning_type,
                issued_by_team_approval, issued_at, improvement_deadline,
                improvement_extended, improvement_extended_until,
                resolved, resolved_at, warning_file_path
            FROM governance_review_warnings
            WHERE id = ?
            "#,
        )
        .bind(warning_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(GovernanceReviewWarning {
            id: row.get(0),
            case_id: row.get(1),
            maintainer_id: row.get(2),
            warning_level: row.get(3),
            warning_type: row.get(4),
            issued_by_team_approval: row.get(5),
            issued_at: row.get(6),
            improvement_deadline: row.get(7),
            improvement_extended: row.get(8),
            improvement_extended_until: row.get(9),
            resolved: row.get(10),
            resolved_at: row.get(11),
            warning_file_path: row.get(12),
        })
    }

    /// Check if maintainer has unresolved warnings
    pub async fn has_unresolved_warnings(&self, maintainer_id: i32) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM governance_review_warnings
            WHERE maintainer_id = ? AND resolved = false
            "#,
        )
        .bind(maintainer_id)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get(0);

        Ok(count > 0)
    }
}
