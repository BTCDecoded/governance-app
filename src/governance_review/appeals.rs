//! Appeal process implementation
//!
//! Implements policy appeal process:
//! - 60-day appeal deadline
//! - 5-of-7 teams required to overturn
//! - New evidence can be submitted

use crate::governance_review::case::GovernanceReviewCaseManager;
use crate::governance_review::models::{Appeal, policy};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};

pub struct AppealManager {
    pool: SqlitePool,
}

impl AppealManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Submit an appeal
    /// Policy: 60-day deadline from submission, but must not exceed case resolution deadline
    pub async fn submit_appeal(
        &self,
        case_id: i32,
        maintainer_id: i32,
        appeal_reason: &str,
        new_evidence: serde_json::Value,
    ) -> Result<Appeal, sqlx::Error> {
        // Get case to check resolution deadline
        let case_manager = GovernanceReviewCaseManager::new(self.pool.clone());
        let case = case_manager.get_case_by_id(case_id).await?;

        // Policy: 60-day appeal deadline, but must not exceed case resolution deadline
        let max_appeal_deadline = case
            .resolution_deadline
            .unwrap_or(Utc::now() + Duration::days(policy::APPEAL_DEADLINE_DAYS));
        let appeal_deadline = std::cmp::min(
            Utc::now() + Duration::days(policy::APPEAL_DEADLINE_DAYS),
            max_appeal_deadline,
        );

        let appeal_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_appeals
            (case_id, maintainer_id, appeal_reason, new_evidence, appeal_deadline, status)
            VALUES (?, ?, ?, ?, ?, 'pending')
            RETURNING id
            "#,
        )
        .bind(case_id)
        .bind(maintainer_id)
        .bind(appeal_reason)
        .bind(serde_json::to_string(&new_evidence).map_err(|e| sqlx::Error::Decode(Box::new(e)))?)
        .bind(appeal_deadline)
        .fetch_one(&self.pool)
        .await?;

        self.get_appeal_by_id(appeal_id).await
    }

    /// Get appeal by ID
    pub async fn get_appeal_by_id(&self, appeal_id: i32) -> Result<Appeal, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, case_id, maintainer_id, appeal_reason, new_evidence,
                submitted_at, appeal_deadline, status, reviewed_at,
                review_decision, teams_approval_count
            FROM governance_review_appeals
            WHERE id = ?
            "#,
        )
        .bind(appeal_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Appeal {
            id: row.get(0),
            case_id: row.get(1),
            maintainer_id: row.get(2),
            appeal_reason: row.get(3),
            new_evidence: serde_json::from_str(row.get::<String, _>(4).as_str())
                .unwrap_or_default(),
            submitted_at: row.get(5),
            appeal_deadline: row.get(6),
            status: row.get(7),
            reviewed_at: row.get(8),
            review_decision: row.get(9),
            teams_approval_count: row.get(10),
        })
    }

    /// Review an appeal (overturn or deny)
    /// Policy: Requires 5-of-7 teams to overturn
    pub async fn review_appeal(
        &self,
        appeal_id: i32,
        teams_approval_count: i32, // Number of teams that approved overturning
        decision: &str,            // 'granted' or 'denied'
        review_decision: &str,     // Explanation
    ) -> Result<(), sqlx::Error> {
        // Policy: Must have at least 5 teams to overturn
        if decision == "granted" && teams_approval_count < policy::APPEAL_OVERTURN_THRESHOLD {
            return Err(sqlx::Error::RowNotFound);
        }

        sqlx::query(
            r#"
            UPDATE governance_review_appeals
            SET status = ?, reviewed_at = ?, review_decision = ?, teams_approval_count = ?
            WHERE id = ?
            "#,
        )
        .bind(decision)
        .bind(Utc::now())
        .bind(review_decision)
        .bind(teams_approval_count)
        .bind(appeal_id)
        .execute(&self.pool)
        .await?;

        // If appeal granted, reactivate maintainer and update case
        if decision == "granted" {
            let appeal = self.get_appeal_by_id(appeal_id).await?;

            // Reactivate maintainer
            sqlx::query("UPDATE maintainers SET active = true, last_updated = ? WHERE id = ?")
                .bind(Utc::now())
                .bind(appeal.maintainer_id)
                .execute(&self.pool)
                .await?;

            // Update case status
            sqlx::query(
                r#"
                UPDATE governance_review_cases
                SET status = 'resolved', resolution_reason = ?
                WHERE id = ?
                "#,
            )
            .bind(format!("Appeal granted: {review_decision}"))
            .bind(appeal.case_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Check for expired appeals
    pub async fn check_expired_appeals(&self) -> Result<Vec<i32>, sqlx::Error> {
        let expired = sqlx::query(
            r#"
            SELECT id FROM governance_review_appeals
            WHERE status = 'pending' AND appeal_deadline < ?
            "#,
        )
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await?;

        // Mark as expired
        for row in &expired {
            let appeal_id: i32 = row.get(0);
            sqlx::query("UPDATE governance_review_appeals SET status = 'expired' WHERE id = ?")
                .bind(appeal_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(expired.iter().map(|row| row.get::<i32, _>(0)).collect())
    }
}
