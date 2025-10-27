//! Governance Adoption Tracking
//!
//! Tracks adoption metrics for different governance rulesets

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use tracing::info;

use super::types::*;
use crate::error::GovernanceError;

#[derive(Clone)]
pub struct AdoptionTracker {
    pool: SqlitePool,
}

impl AdoptionTracker {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Track adoption of a ruleset by a node
    pub async fn track_adoption(
        &self,
        ruleset_id: &str,
        node_id: &str,
        node_type: &str,
        weight: f64,
        decision_reason: &str,
        signature: &str,
    ) -> Result<(), GovernanceError> {
        // Record the adoption decision
        sqlx::query(
            r#"
            INSERT INTO fork_decisions 
            (ruleset_id, node_id, node_type, weight, decision_reason, signature, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(ruleset_id)
        .bind(node_id)
        .bind(node_type)
        .bind(weight)
        .bind(decision_reason)
        .bind(signature)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to track adoption: {}", e)))?;

        // Log adoption event
        self.log_fork_event(
            ForkEventType::RulesetAdopted,
            ruleset_id,
            node_id,
            &serde_json::json!({
                "node_type": node_type,
                "weight": weight,
                "decision_reason": decision_reason
            }),
        )
        .await?;

        info!(
            "Tracked adoption of ruleset {} by node {} (weight: {})",
            ruleset_id, node_id, weight
        );
        Ok(())
    }

    /// Calculate adoption metrics for a specific ruleset
    pub async fn calculate_adoption_metrics(
        &self,
        ruleset_id: &str,
    ) -> Result<AdoptionMetrics, GovernanceError> {
        // Get all adoption decisions for this ruleset
        let decisions = sqlx::query(
            r#"
            SELECT node_type, weight, timestamp
            FROM fork_decisions 
            WHERE ruleset_id = ?
            ORDER BY timestamp DESC
            "#,
        )
        .bind(ruleset_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to fetch adoption decisions: {}", e))
        })?;

        let mut node_count = 0;
        let mut hashpower_weight = 0.0;
        let mut economic_weight = 0.0;
        let mut total_weight = 0.0;

        for decision in decisions {
            let node_type = decision.get::<String, _>("node_type");
            let weight = decision.get::<f64, _>("weight");

            node_count += 1;
            total_weight += weight;

            match node_type.as_str() {
                "mining_pool" => hashpower_weight += weight,
                _ => economic_weight += weight,
            }
        }

        // Calculate percentages (assuming total network metrics are known)
        // For now, use placeholder calculations
        let hashpower_percentage = if total_weight > 0.0 {
            (hashpower_weight / total_weight) * 100.0
        } else {
            0.0
        };

        let economic_activity_percentage = if total_weight > 0.0 {
            (economic_weight / total_weight) * 100.0
        } else {
            0.0
        };

        Ok(AdoptionMetrics {
            ruleset_id: ruleset_id.to_string(),
            node_count: node_count as u32,
            hashpower_percentage,
            economic_activity_percentage,
            total_weight,
            last_updated: Utc::now(),
        })
    }

    /// Get comprehensive adoption statistics
    pub async fn get_adoption_statistics(&self) -> Result<AdoptionStatistics, GovernanceError> {
        // Get all unique rulesets
        let rulesets = sqlx::query("SELECT DISTINCT ruleset_id FROM fork_decisions")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to fetch rulesets: {}", e))
            })?;

        let mut adoption_metrics = Vec::new();
        let mut total_nodes = 0;
        let mut total_hashpower = 0.0;
        let mut total_economic_activity = 0.0;

        for ruleset in rulesets {
            let ruleset_id = ruleset.get::<String, _>("ruleset_id");
            let metrics = self.calculate_adoption_metrics(&ruleset_id).await?;

            total_nodes += metrics.node_count;
            total_hashpower += metrics.hashpower_percentage;
            total_economic_activity += metrics.economic_activity_percentage;

            adoption_metrics.push(metrics);
        }

        // Find winning ruleset (highest adoption)
        let winning_ruleset = adoption_metrics
            .iter()
            .max_by(|a, b| {
                a.total_weight
                    .partial_cmp(&b.total_weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| m.ruleset_id.clone());

        // Calculate overall adoption percentage
        let adoption_percentage = if total_nodes > 0 {
            (adoption_metrics.len() as f64 / total_nodes as f64) * 100.0
        } else {
            0.0
        };

        Ok(AdoptionStatistics {
            total_nodes,
            total_hashpower,
            total_economic_activity,
            rulesets: adoption_metrics,
            winning_ruleset,
            adoption_percentage,
            last_updated: Utc::now(),
        })
    }

    /// Check if adoption threshold is met for a ruleset
    pub async fn check_adoption_threshold(
        &self,
        ruleset_id: &str,
        thresholds: &ForkThresholds,
    ) -> Result<bool, GovernanceError> {
        let metrics = self.calculate_adoption_metrics(ruleset_id).await?;

        let threshold_met = metrics.node_count >= thresholds.minimum_node_count
            && metrics.hashpower_percentage >= thresholds.minimum_hashpower_percentage
            && metrics.economic_activity_percentage
                >= thresholds.minimum_economic_activity_percentage;

        if threshold_met {
            // Log threshold met event
            self.log_fork_event(
                ForkEventType::AdoptionThresholdMet,
                ruleset_id,
                "system",
                &serde_json::json!({
                    "node_count": metrics.node_count,
                    "hashpower_percentage": metrics.hashpower_percentage,
                    "economic_activity_percentage": metrics.economic_activity_percentage,
                    "thresholds": thresholds
                }),
            )
            .await?;
        }

        Ok(threshold_met)
    }

    /// Get adoption history for a ruleset
    pub async fn get_adoption_history(
        &self,
        ruleset_id: &str,
        days: u32,
    ) -> Result<Vec<AdoptionMetrics>, GovernanceError> {
        let _cutoff_date = Utc::now() - chrono::Duration::try_days(days as i64).unwrap_or_default();

        // This would require a more complex query to get historical data
        // For now, return current metrics
        let current_metrics = self.calculate_adoption_metrics(ruleset_id).await?;
        Ok(vec![current_metrics])
    }

    /// Log a fork event
    async fn log_fork_event(
        &self,
        event_type: ForkEventType,
        ruleset_id: &str,
        node_id: &str,
        details: &serde_json::Value,
    ) -> Result<(), GovernanceError> {
        let event_id = format!("{}_{}_{}", event_type.as_str(), ruleset_id, node_id);

        sqlx::query(
            r#"
            INSERT INTO fork_events 
            (event_id, event_type, ruleset_id, node_id, details, timestamp)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&event_id)
        .bind(event_type.as_str())
        .bind(ruleset_id)
        .bind(node_id)
        .bind(serde_json::to_string(details)?)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to log fork event: {}", e)))?;

        Ok(())
    }
}
