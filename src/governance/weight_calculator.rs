//! Weight Calculator (Reporting/Transparency Only)
//!
//! NOTE: Governance is maintainer-only multisig. This calculator is kept for
//! reporting/transparency purposes only - it does NOT affect governance decisions.
//! All weight calculations return 0.0 since contributions no longer affect governance.

use anyhow::Result;
use sqlx::SqlitePool;
use tracing::{debug, info};

/// Weight calculator (for reporting/transparency only)
/// All weights are 0.0 since governance is maintainer-only
pub struct WeightCalculator {
    pool: SqlitePool,
}

impl WeightCalculator {
    /// Create a new weight calculator
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Calculate ongoing participation weight (for reporting only)
    /// Note: Governance is maintainer-only, always returns 0.0
    pub fn calculate_participation_weight(&self) -> f64 {
        // Governance is maintainer-only - no contribution-based weight
        0.0
    }

    /// Apply weight cap to prevent whale dominance (reporting only; governance is maintainer-only)
    pub fn apply_weight_cap(&self, calculated_weight: f64, _total_system_weight: f64) -> f64 {
        calculated_weight // No cap applied; maintainer-only governance
    }

    /// Check if contribution is eligible for voting (cooling-off period; reporting only)
    pub fn check_cooling_off(
        &self,
        _contribution_amount_btc: f64,
        _contribution_age_days: u32,
    ) -> bool {
        true // No cooling period; maintainer-only governance
    }

    /// Calculate zap "weight" for reporting purposes only (not used in governance)
    /// Governance is maintainer-only - zaps are tracked for transparency only
    pub fn calculate_zap_vote_weight(&self, zap_amount_btc: f64) -> f64 {
        // Return 0 - zaps don't affect governance
        0.0
    }

    /// Get proposal vote weight (for reporting only)
    /// Note: Governance is maintainer-only, always returns 0.0
    pub fn get_proposal_vote_weight(&self) -> f64 {
        // Governance is maintainer-only - zaps don't affect governance
        0.0
    }

    /// Calculate and update participation weights for all contributors
    pub async fn update_participation_weights(&self) -> Result<()> {
        // First, update contribution ages (for cooling-off calculation)
        sqlx::query(
            r#"
            UPDATE unified_contributions
            SET contribution_age_days = CAST(
                (julianday('now') - julianday(timestamp)) AS INTEGER
            )
            WHERE contribution_age_days != CAST(
                (julianday('now') - julianday(timestamp)) AS INTEGER
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Get all unique contributors
        #[derive(sqlx::FromRow)]
        struct ContributorRow {
            contributor_id: String,
            contributor_type: String,
        }

        let contributors = sqlx::query_as::<_, ContributorRow>(
            r#"
            SELECT DISTINCT contributor_id, contributor_type
            FROM unified_contributions
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let contributor_count = contributors.len();

        // First pass: calculate all base weights and store contribution data
        // NOTE: Governance is maintainer-only - all weights are 0.0 (for reporting only)
        struct ContributorData {
            contributor_id: String,
            contributor_type: String,
            total_contribution_btc: f64, // Always 0.0 (maintainer-only governance)
            base_weight: f64,            // Always 0.0 (maintainer-only governance)
        }

        let mut contributor_data = Vec::new();

        for contributor in contributors {
            let contributor_id = contributor.contributor_id.clone();

            // Governance is maintainer-only - no contribution-based weight
            // All contributions are tracked for reporting/transparency only
            let total_contribution_btc = 0.0; // No contribution-based weight
            let base_weight = 0.0; // Maintainer-only governance

            contributor_data.push(ContributorData {
                contributor_id,
                contributor_type: contributor.contributor_type,
                total_contribution_btc,
                base_weight,
            });
        }

        // All weights are 0.0 (maintainer-only governance)
        let final_total = 0.0;
        let capped_weights: Vec<(String, f64)> = contributor_data
            .iter()
            .map(|data| (data.contributor_id.clone(), 0.0))
            .collect();

        // Update all weights in database
        for (idx, data) in contributor_data.iter().enumerate() {
            let capped_weight = capped_weights[idx].1;
            sqlx::query(
                r#"
                INSERT INTO participation_weights
                (contributor_id, contributor_type, total_contribution_btc, base_weight, capped_weight, total_system_weight, last_updated)
                VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(contributor_id) DO UPDATE SET
                    contributor_type = excluded.contributor_type,
                    total_contribution_btc = excluded.total_contribution_btc,
                    base_weight = excluded.base_weight,
                    capped_weight = excluded.capped_weight,
                    total_system_weight = excluded.total_system_weight,
                    last_updated = CURRENT_TIMESTAMP
                "#,
            )
            .bind(&data.contributor_id)
            .bind(&data.contributor_type)
            .bind(data.total_contribution_btc)
            .bind(data.base_weight) // Actual base weight
            .bind(capped_weight) // Capped weight
            .bind(final_total)
            .execute(&self.pool)
            .await?;

            debug!(
                "Updated participation weight for {}: base={:.2}, capped={:.2} (contributions: {:.8} BTC)",
                data.contributor_id, data.base_weight, capped_weight, data.total_contribution_btc
            );
        }

        info!(
            "Updated participation weights for {} contributors",
            contributor_count
        );
        Ok(())
    }

    /// Calculate total system weight (sum of all capped weights)
    pub async fn calculate_total_system_weight(&self) -> Result<f64> {
        let total: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT SUM(capped_weight) as total
            FROM participation_weights
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(total.unwrap_or(0.0))
    }

    /// Get participation weight for a contributor
    pub async fn get_participation_weight(&self, contributor_id: &str) -> Result<Option<f64>> {
        let weight: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT capped_weight
            FROM participation_weights
            WHERE contributor_id = ?
            "#,
        )
        .bind(contributor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(weight)
    }
}
