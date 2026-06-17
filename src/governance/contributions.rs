//! Contribution Tracking Service
//!
//! Tracks governance contributions (zaps) for transparency/reporting only.
//! Note: Contributions no longer affect governance weight (maintainer-only governance).

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// Contribution tracking service
pub struct ContributionTracker {
    pool: SqlitePool,
}

impl ContributionTracker {
    /// Create a new contribution tracker
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // Merge mining removed - it's now a module with its own revenue model
    // Merge mining revenue goes to module developer, not governance

    /// Record a zap contribution (called from zap tracker)
    pub async fn record_zap_contribution(
        &self,
        contributor_id: &str, // Sender pubkey
        amount_btc: f64,
        timestamp: DateTime<Utc>,
        is_proposal_zap: bool,
    ) -> Result<()> {
        // Record in unified contributions table
        let contribution_type = if is_proposal_zap {
            "zap:proposal"
        } else {
            "zap:general"
        };
        sqlx::query(
            r#"
            INSERT INTO unified_contributions
            (contributor_id, contributor_type, contribution_type, amount_btc, timestamp, contribution_age_days, period_type, verified)
            VALUES (?, ?, ?, ?, ?, 0, ?, ?)
            "#,
        )
        .bind(contributor_id)
        .bind("zap_user")
        .bind(contribution_type)
        .bind(amount_btc)
        .bind(timestamp)
        .bind("cumulative")
        .bind(true)  // Verified (Nostr event)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get total contributions for a contributor in a time period
    pub async fn get_contributor_total(
        &self,
        contributor_id: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<ContributorTotal> {
        let rows = sqlx::query_as::<_, (String, Option<f64>)>(
            r#"
            SELECT 
                contribution_type,
                SUM(amount_btc) as total_btc
            FROM unified_contributions
            WHERE contributor_id = ?
              AND timestamp >= ?
              AND timestamp <= ?
            GROUP BY contribution_type
            "#,
        )
        .bind(contributor_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await?;

        let mut zaps_btc = 0.0;

        for (contribution_type, total_btc) in rows {
            let total = total_btc.unwrap_or(0.0);
            if contribution_type.starts_with("zap:") {
                zaps_btc += total;
            }
        }

        Ok(ContributorTotal {
            zaps_btc,
            total_btc: zaps_btc,
        })
    }

    /// Update contribution age for cooling-off period calculation
    pub async fn update_contribution_ages(&self) -> Result<()> {
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

        Ok(())
    }
}

/// Contributor total contributions (for reporting/transparency only)
#[derive(Debug, Clone)]
pub struct ContributorTotal {
    pub zaps_btc: f64,
    pub total_btc: f64,
}
