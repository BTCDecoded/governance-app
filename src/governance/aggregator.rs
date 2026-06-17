//! Contribution Aggregator (Reporting/Transparency Only)
//!
//! Aggregates contributions for reporting/transparency purposes only.
//! NOTE: Governance is maintainer-only multisig - contributions do NOT affect governance.
//! This aggregator is kept for public reporting/dashboards.

use crate::governance::{ContributionTracker, WeightCalculator};
use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

/// Contribution aggregator for monthly aggregation
pub struct ContributionAggregator {
    pool: SqlitePool,
    contribution_tracker: ContributionTracker,
    weight_calculator: WeightCalculator,
}

impl ContributionAggregator {
    /// Create a new contribution aggregator
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: pool.clone(),
            contribution_tracker: ContributionTracker::new(pool.clone()),
            weight_calculator: WeightCalculator::new(pool),
        }
    }

    /// Aggregate cumulative zap contributions (all-time) - for reporting only
    /// NOTE: Zaps do NOT affect governance (maintainer-only multisig)
    /// Returns total BTC zapped (cumulative) for transparency/reporting
    pub async fn aggregate_zaps_cumulative(&self, contributor_id: &str) -> Result<f64> {
        let total: Option<f64> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(SUM(amount_btc), 0.0) as total
            FROM unified_contributions
            WHERE contributor_id = ?
              AND contribution_type LIKE 'zap:%'
            "#,
        )
        .bind(contributor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(total.unwrap_or(0.0))
    }

    /// Update all participation weights (for reporting only)
    /// NOTE: Governance is maintainer-only - weights are 0.0 and don't affect governance
    /// This is kept for reporting/transparency purposes
    pub async fn update_all_weights(&self) -> Result<()> {
        info!("Starting participation weight update (for reporting only)");

        // Update contribution ages first (for reporting)
        self.contribution_tracker.update_contribution_ages().await?;

        // Update all participation weights (all will be 0.0 since governance is maintainer-only)
        self.weight_calculator
            .update_participation_weights()
            .await?;

        info!(
            "Completed participation weight update (all weights are 0.0 - maintainer-only governance)"
        );
        Ok(())
    }

    /// Get aggregated contributions for a contributor (zaps only)
    pub async fn get_contributor_aggregates(
        &self,
        contributor_id: &str,
    ) -> Result<ContributorAggregates> {
        let zaps = self.aggregate_zaps_cumulative(contributor_id).await?;

        // Get participation weight (always 0.0 for maintainer-only governance)
        let participation_weight = self
            .weight_calculator
            .get_participation_weight(contributor_id)
            .await?
            .unwrap_or(0.0);

        Ok(ContributorAggregates {
            cumulative_zaps_btc: zaps,
            total_contribution_btc: zaps,
            participation_weight,
        })
    }
}

/// Aggregated contributions for a contributor (for reporting/transparency only)
/// NOTE: Governance is maintainer-only - these values do NOT affect governance decisions
#[derive(Debug, Clone)]
pub struct ContributorAggregates {
    pub cumulative_zaps_btc: f64,
    pub total_contribution_btc: f64,
    pub participation_weight: f64, // Always 0.0 (maintainer-only governance)
}
