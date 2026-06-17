//! Protection mechanisms
//!
//! Implements policy protections:
//! - Whistleblower protection (retaliation = immediate removal)
//! - False report consequences
//! - Privacy for reporters

use crate::governance_review::models::{FalseReport, Retaliation};
use chrono::Utc;
use sqlx::{Row, SqlitePool};

pub struct ProtectionManager {
    pool: SqlitePool,
}

impl ProtectionManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Report retaliation against a reporter
    /// Policy: Retaliation is immediate grounds for removal
    pub async fn report_retaliation(
        &self,
        original_case_id: i32,
        reporter_maintainer_id: i32,
        retaliator_maintainer_id: i32,
        retaliation_type: &str,
        description: &str,
    ) -> Result<Retaliation, sqlx::Error> {
        let retaliation_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_retaliation
            (original_case_id, reporter_maintainer_id, retaliator_maintainer_id,
             retaliation_type, description, status)
            VALUES (?, ?, ?, ?, ?, 'open')
            RETURNING id
            "#,
        )
        .bind(original_case_id)
        .bind(reporter_maintainer_id)
        .bind(retaliator_maintainer_id)
        .bind(retaliation_type)
        .bind(description)
        .fetch_one(&self.pool)
        .await?;

        self.get_retaliation_by_id(retaliation_id).await
    }

    /// Get retaliation by ID
    pub async fn get_retaliation_by_id(
        &self,
        retaliation_id: i32,
    ) -> Result<Retaliation, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, original_case_id, reporter_maintainer_id, retaliator_maintainer_id,
                retaliation_type, description, reported_at, status, confirmed_at
            FROM governance_review_retaliation
            WHERE id = ?
            "#,
        )
        .bind(retaliation_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Retaliation {
            id: row.get(0),
            original_case_id: row.get(1),
            reporter_maintainer_id: row.get(2),
            retaliator_maintainer_id: row.get(3),
            retaliation_type: row.get(4),
            description: row.get(5),
            reported_at: row.get(6),
            status: row.get(7),
            confirmed_at: row.get(8),
        })
    }

    /// Confirm retaliation (triggers immediate removal process)
    /// Policy: Retaliation = immediate removal (emergency removal by 4-of-5 emergency keyholders)
    pub async fn confirm_retaliation(
        &self,
        retaliation_id: i32,
        emergency_keyholder_approvals: Vec<i32>, // Emergency keyholder IDs who confirmed
    ) -> Result<(), sqlx::Error> {
        // Policy: Must have at least 4 emergency keyholder approvals for immediate removal
        if emergency_keyholder_approvals.len() < 4 {
            return Err(sqlx::Error::RowNotFound);
        }

        // Get retaliation details
        let retaliation = self.get_retaliation_by_id(retaliation_id).await?;

        // Start transaction for atomic retaliation confirmation + removal
        let mut tx = self.pool.begin().await?;

        // Update retaliation status
        sqlx::query(
            r#"
            UPDATE governance_review_retaliation
            SET status = 'confirmed', confirmed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(retaliation_id)
        .execute(&mut *tx)
        .await?;

        // Auto-trigger emergency removal of retaliator
        // Deactivate maintainer key
        sqlx::query(
            r#"
            UPDATE maintainers
            SET active = false, last_updated = ?
            WHERE id = ?
            "#,
        )
        .bind(Utc::now())
        .bind(retaliation.retaliator_maintainer_id)
        .execute(&mut *tx)
        .await?;

        // Create emergency removal case for tracking
        sqlx::query(
            r#"
            INSERT INTO governance_review_cases
            (case_number, subject_maintainer_id, reporter_maintainer_id,
             case_type, severity, status, description, evidence, on_platform,
             created_at, resolved_at, resolution_reason)
            VALUES (?, ?, ?, 'retaliation', 'gross_misconduct', 'removed', ?, '{}', true, ?, ?, ?)
            "#,
        )
        .bind(format!(
            "GR-RETALIATION-{}",
            Utc::now().format("%Y%m%d-%H%M%S")
        ))
        .bind(retaliation.retaliator_maintainer_id)
        .bind(retaliation.reporter_maintainer_id)
        .bind(format!(
            "Retaliation confirmed: {}",
            retaliation.description
        ))
        .bind(Utc::now())
        .bind(Utc::now())
        .bind("Immediate removal due to confirmed retaliation")
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(())
    }

    /// Report a false report
    /// Policy: False reports are grounds for warning or removal
    pub async fn report_false_report(
        &self,
        original_case_id: i32,
        false_reporter_maintainer_id: i32,
        false_report_evidence: &str,
    ) -> Result<FalseReport, sqlx::Error> {
        let false_report_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_false_reports
            (original_case_id, false_reporter_maintainer_id, false_report_evidence, sanction_applied)
            VALUES (?, ?, ?, 'none')
            RETURNING id
            "#,
        )
        .bind(original_case_id)
        .bind(false_reporter_maintainer_id)
        .bind(false_report_evidence)
        .fetch_one(&self.pool)
        .await?;

        self.get_false_report_by_id(false_report_id).await
    }

    /// Get false report by ID
    pub async fn get_false_report_by_id(
        &self,
        false_report_id: i32,
    ) -> Result<FalseReport, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, original_case_id, false_reporter_maintainer_id,
                confirmed_false_at, false_report_evidence, sanction_applied, sanction_case_id
            FROM governance_review_false_reports
            WHERE id = ?
            "#,
        )
        .bind(false_report_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(FalseReport {
            id: row.get(0),
            original_case_id: row.get(1),
            false_reporter_maintainer_id: row.get(2),
            confirmed_false_at: row.get(3),
            false_report_evidence: row.get(4),
            sanction_applied: row.get(5),
            sanction_case_id: row.get(6),
        })
    }
}
