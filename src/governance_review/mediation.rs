//! Mediation process implementation
//!
//! Implements policy mediation process:
//! - 30-day mediation period
//! - Optional neutral mediator
//! - Conflict resolution before escalation

use crate::governance_review::models::{Mediation, policy};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};

pub struct MediationManager {
    pool: SqlitePool,
}

impl MediationManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Start mediation for a case
    /// Policy: 30-day mediation period
    pub async fn start_mediation(
        &self,
        case_id: i32,
        mediator_maintainer_id: Option<i32>, // Optional neutral maintainer
    ) -> Result<Mediation, sqlx::Error> {
        // Policy: 30-day mediation period
        let mediation_deadline = Utc::now() + Duration::days(policy::MEDIATION_PERIOD_DAYS);

        let mediation_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_mediation
            (case_id, mediator_maintainer_id, mediation_deadline, status)
            VALUES (?, ?, ?, 'active')
            RETURNING id
            "#,
        )
        .bind(case_id)
        .bind(mediator_maintainer_id)
        .bind(mediation_deadline)
        .fetch_one(&self.pool)
        .await?;

        // Update case status
        sqlx::query("UPDATE governance_review_cases SET status = 'mediation' WHERE id = ?")
            .bind(case_id)
            .execute(&self.pool)
            .await?;

        self.get_mediation_by_id(mediation_id).await
    }

    /// Get mediation by ID
    pub async fn get_mediation_by_id(&self, mediation_id: i32) -> Result<Mediation, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, case_id, mediator_maintainer_id, mediation_started_at,
                mediation_deadline, status, resolution_notes, resolved_at
            FROM governance_review_mediation
            WHERE id = ?
            "#,
        )
        .bind(mediation_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Mediation {
            id: row.get(0),
            case_id: row.get(1),
            mediator_maintainer_id: row.get(2),
            mediation_started_at: row.get(3),
            mediation_deadline: row.get(4),
            status: row.get(5),
            resolution_notes: row.get(6),
            resolved_at: row.get(7),
        })
    }

    /// Resolve mediation (successful)
    pub async fn resolve_mediation(
        &self,
        mediation_id: i32,
        resolution_notes: &str,
    ) -> Result<(), sqlx::Error> {
        let mediation = self.get_mediation_by_id(mediation_id).await?;

        sqlx::query(
            r#"
            UPDATE governance_review_mediation
            SET status = 'resolved', resolution_notes = ?, resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(resolution_notes)
        .bind(Utc::now())
        .bind(mediation_id)
        .execute(&self.pool)
        .await?;

        // Update case status to resolved
        sqlx::query(
            r#"
            UPDATE governance_review_cases
            SET status = 'resolved', resolution_reason = ?, resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(format!("Mediation successful: {resolution_notes}"))
        .bind(Utc::now())
        .bind(mediation.case_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Fail mediation (unsuccessful, proceed to sanctions)
    pub async fn fail_mediation(
        &self,
        mediation_id: i32,
        resolution_notes: &str,
    ) -> Result<(), sqlx::Error> {
        let mediation = self.get_mediation_by_id(mediation_id).await?;

        sqlx::query(
            r#"
            UPDATE governance_review_mediation
            SET status = 'failed', resolution_notes = ?, resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(resolution_notes)
        .bind(Utc::now())
        .bind(mediation_id)
        .execute(&self.pool)
        .await?;

        // Update case status back to under_review (proceed to sanctions)
        sqlx::query(
            r#"
            UPDATE governance_review_cases
            SET status = 'under_review'
            WHERE id = ?
            "#,
        )
        .bind(mediation.case_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check for expired mediations
    pub async fn check_expired_mediations(&self) -> Result<Vec<i32>, sqlx::Error> {
        let expired = sqlx::query(
            r#"
            SELECT id FROM governance_review_mediation
            WHERE status = 'active' AND mediation_deadline < ?
            "#,
        )
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await?;

        // Mark as failed (expired = failed)
        for row in &expired {
            let mediation_id: i32 = row.get(0);
            self.fail_mediation(mediation_id, "Mediation period expired without resolution")
                .await?;
        }

        Ok(expired.iter().map(|row| row.get::<i32, _>(0)).collect())
    }
}
