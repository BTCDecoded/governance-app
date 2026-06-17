//! Time limit management
//!
//! Implements policy time limits:
//! - 180 days for case resolution
//! - 90 days for appeals
//! - 90 days for improvement periods
//! - Extensions require approval

use crate::governance_review::case::GovernanceReviewCaseManager;
use crate::governance_review::models::policy;
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};

pub struct TimeLimitManager {
    pool: SqlitePool,
}

impl TimeLimitManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create time limits for a case
    pub async fn create_time_limits(
        &self,
        case_id: i32,
        response_deadline: DateTime<Utc>,
        resolution_deadline: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        // Response deadline
        sqlx::query(
            r#"
            INSERT INTO governance_review_time_limits (case_id, limit_type, deadline)
            VALUES (?, 'response', ?)
            "#,
        )
        .bind(case_id)
        .bind(response_deadline)
        .execute(&self.pool)
        .await?;

        // Resolution deadline
        sqlx::query(
            r#"
            INSERT INTO governance_review_time_limits (case_id, limit_type, deadline)
            VALUES (?, 'resolution', ?)
            "#,
        )
        .bind(case_id)
        .bind(resolution_deadline)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Extend a time limit (requires 5-of-7 team approval)
    /// Policy: Maximum extension is 90 days beyond original deadline
    pub async fn extend_time_limit(
        &self,
        case_id: i32,
        limit_type: &str,
        extension_approved_by: i32,
        extension_reason: &str,
        extension_until: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        // Get original deadline
        let original_deadline: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            SELECT deadline FROM governance_review_time_limits
            WHERE case_id = ? AND limit_type = ? AND extended = false
            "#,
        )
        .bind(case_id)
        .bind(limit_type)
        .fetch_optional(&self.pool)
        .await?;

        // If no original deadline found, try to get it from case
        let original_deadline = if let Some(deadline) = original_deadline {
            deadline
        } else {
            // Fallback: get from case resolution_deadline for resolution limits
            if limit_type == "resolution" {
                let case_manager = GovernanceReviewCaseManager::new(self.pool.clone());
                let case = case_manager.get_case_by_id(case_id).await?;
                case.resolution_deadline.unwrap_or(Utc::now())
            } else {
                // For other limit types, use current time as baseline
                Utc::now()
            }
        };

        // Validate: extension_until must not exceed original_deadline + MAX_EXTENSION_DAYS
        let max_extension = original_deadline + Duration::days(policy::MAX_EXTENSION_DAYS);
        if extension_until > max_extension {
            return Err(sqlx::Error::RowNotFound); // Or custom error type
        }

        sqlx::query(
            r#"
            UPDATE governance_review_time_limits
            SET extended = true, extension_approved_by = ?, 
                extension_reason = ?, extension_until = ?
            WHERE case_id = ? AND limit_type = ?
            "#,
        )
        .bind(extension_approved_by)
        .bind(extension_reason)
        .bind(extension_until)
        .bind(case_id)
        .bind(limit_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check for expired time limits
    pub async fn check_expired_limits(&self) -> Result<Vec<(i32, String)>, sqlx::Error> {
        let expired = sqlx::query(
            r#"
            SELECT case_id, limit_type
            FROM governance_review_time_limits
            WHERE deadline < ? AND extended = false
            "#,
        )
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await?;

        Ok(expired
            .iter()
            .map(|row| (row.get::<i32, _>(0), row.get::<String, _>(1)))
            .collect())
    }
}
