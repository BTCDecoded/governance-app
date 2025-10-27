//! Veto Signal Management
//!
//! Handles collection, verification, and threshold calculation for economic node vetoes

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use tracing::info;

use super::types::*;
use crate::crypto::signatures::SignatureManager;
use crate::error::GovernanceError;

pub struct VetoManager {
    pool: SqlitePool,
    signature_manager: SignatureManager,
}

impl VetoManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            signature_manager: SignatureManager::new(),
        }
    }

    /// Collect a veto signal from an economic node
    pub async fn collect_veto_signal(
        &self,
        pr_id: i32,
        node_id: i32,
        signal_type: SignalType,
        signature: &str,
        rationale: &str,
    ) -> Result<i32, GovernanceError> {
        // Get node information
        let node = self.get_node_by_id(node_id).await?;
        if node.status != NodeStatus::Active {
            return Err(GovernanceError::CryptoError(
                "Node is not active".to_string(),
            ));
        }

        // Verify signature
        let message = format!("PR #{} veto signal from {}", pr_id, node.entity_name);
        let verified = self.signature_manager.verify_governance_signature(
            &message,
            signature,
            &node.public_key,
        )?;

        if !verified {
            return Err(GovernanceError::CryptoError(
                "Invalid signature".to_string(),
            ));
        }

        // Check if node already submitted a signal for this PR
        let existing = sqlx::query("SELECT id FROM veto_signals WHERE pr_id = ? AND node_id = ?")
            .bind(pr_id)
            .bind(node_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to check existing signal: {}", e))
            })?;

        if existing.is_some() {
            return Err(GovernanceError::CryptoError(
                "Node already submitted signal for this PR".to_string(),
            ));
        }

        // Insert veto signal
        let result = sqlx::query(
            r#"
            INSERT INTO veto_signals 
            (pr_id, node_id, signal_type, weight, signature, rationale, verified)
            VALUES (?, ?, ?, ?, ?, ?, TRUE)
            "#,
        )
        .bind(pr_id)
        .bind(node_id)
        .bind(signal_type.as_str())
        .bind(node.weight)
        .bind(signature)
        .bind(rationale)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to insert veto signal: {}", e))
        })?;

        let signal_id = result.last_insert_rowid() as i32;
        info!(
            "Collected {} signal from node {} for PR {}",
            signal_type.as_str(),
            node.entity_name,
            pr_id
        );

        Ok(signal_id)
    }

    /// Check if veto threshold is met for a PR
    pub async fn check_veto_threshold(&self, pr_id: i32) -> Result<VetoThreshold, GovernanceError> {
        // Get all veto signals for this PR
        let signals = sqlx::query(
            r#"
            SELECT vs.signal_type, vs.weight, en.node_type
            FROM veto_signals vs
            JOIN economic_nodes en ON vs.node_id = en.id
            WHERE vs.pr_id = ? AND vs.verified = TRUE
            "#,
        )
        .bind(pr_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to fetch veto signals: {}", e))
        })?;

        let mut mining_veto_weight = 0.0;
        let mut economic_veto_weight = 0.0;
        let mut total_mining_weight = 0.0;
        let mut total_economic_weight = 0.0;

        // Calculate weights by node type
        for signal in signals {
            let node_type =
                NodeType::from_str(&signal.get::<String, _>("node_type")).ok_or_else(|| {
                    GovernanceError::CryptoError(format!(
                        "Invalid node type: {}",
                        signal.get::<String, _>("node_type")
                    ))
                })?;

            let signal_type = SignalType::from_str(&signal.get::<String, _>("signal_type"))
                .ok_or_else(|| {
                    GovernanceError::CryptoError(format!(
                        "Invalid signal type: {}",
                        signal.get::<String, _>("signal_type")
                    ))
                })?;

            let weight = signal.get::<f64, _>("weight");

            match node_type {
                NodeType::MiningPool => {
                    total_mining_weight += weight;
                    if signal_type == SignalType::Veto {
                        mining_veto_weight += weight;
                    }
                }
                _ => {
                    total_economic_weight += weight;
                    if signal_type == SignalType::Veto {
                        economic_veto_weight += weight;
                    }
                }
            }
        }

        // Calculate percentages
        let mining_veto_percent = if total_mining_weight > 0.0 {
            (mining_veto_weight / total_mining_weight) * 100.0
        } else {
            0.0
        };

        let economic_veto_percent = if total_economic_weight > 0.0 {
            (economic_veto_weight / total_economic_weight) * 100.0
        } else {
            0.0
        };

        // Check thresholds (30% mining or 40% economic)
        let threshold_met = mining_veto_percent >= 30.0 || economic_veto_percent >= 40.0;
        let veto_active = threshold_met;

        Ok(VetoThreshold {
            mining_veto_percent,
            economic_veto_percent,
            threshold_met,
            veto_active,
        })
    }

    /// Get all veto signals for a PR
    pub async fn get_pr_veto_signals(
        &self,
        pr_id: i32,
    ) -> Result<Vec<VetoSignal>, GovernanceError> {
        let rows = sqlx::query(
            r#"
            SELECT vs.id, vs.pr_id, vs.node_id, vs.signal_type, vs.weight, 
                   vs.signature, vs.rationale, vs.timestamp, vs.verified,
                   en.entity_name
            FROM veto_signals vs
            JOIN economic_nodes en ON vs.node_id = en.id
            WHERE vs.pr_id = ?
            ORDER BY vs.timestamp DESC
            "#,
        )
        .bind(pr_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to fetch veto signals: {}", e))
        })?;

        let mut signals = Vec::new();
        for row in rows {
            let signal_type = SignalType::from_str(&row.get::<String, _>("signal_type"))
                .ok_or_else(|| {
                    GovernanceError::CryptoError(format!(
                        "Invalid signal type: {}",
                        row.get::<String, _>("signal_type")
                    ))
                })?;

            signals.push(VetoSignal {
                id: Some(row.get::<i32, _>("id")),
                pr_id: row.get::<i32, _>("pr_id"),
                node_id: row.get::<i32, _>("node_id"),
                signal_type,
                weight: row.get::<f64, _>("weight"),
                signature: row.get::<String, _>("signature"),
                rationale: row.get::<String, _>("rationale"),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
                    .map_err(|e| GovernanceError::CryptoError(format!("Invalid timestamp: {}", e)))?
                    .with_timezone(&Utc),
                verified: row.get::<i32, _>("verified") != 0,
            });
        }

        Ok(signals)
    }

    /// Get economic node by ID
    async fn get_node_by_id(&self, node_id: i32) -> Result<EconomicNode, GovernanceError> {
        let row = sqlx::query(
            r#"
            SELECT id, node_type, entity_name, public_key, qualification_data, 
                   weight, status, registered_at, verified_at, last_verified_at, 
                   created_by, notes
            FROM economic_nodes 
            WHERE id = ?
            "#,
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to fetch node: {}", e)))?
        .ok_or_else(|| GovernanceError::CryptoError("Node not found".to_string()))?;

        let node_type =
            NodeType::from_str(&row.get::<String, _>("node_type")).ok_or_else(|| {
                GovernanceError::CryptoError(format!(
                    "Invalid node type: {}",
                    row.get::<String, _>("node_type")
                ))
            })?;

        let status = NodeStatus::from_str(&row.get::<String, _>("status")).ok_or_else(|| {
            GovernanceError::CryptoError(format!(
                "Invalid status: {}",
                row.get::<String, _>("status")
            ))
        })?;

        Ok(EconomicNode {
            id: Some(row.get::<i32, _>("id")),
            node_type,
            entity_name: row.get::<String, _>("entity_name"),
            public_key: row.get::<String, _>("public_key"),
            qualification_data: serde_json::from_str(&row.get::<String, _>("qualification_data"))?,
            weight: row.get::<f64, _>("weight"),
            status,
            registered_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("registered_at"))
                .map_err(|e| GovernanceError::CryptoError(format!("Invalid timestamp: {}", e)))?
                .with_timezone(&Utc),
            verified_at: row.get::<Option<String>, _>("verified_at").map(|t| {
                DateTime::parse_from_rfc3339(&t)
                    .unwrap()
                    .with_timezone(&Utc)
            }),
            last_verified_at: row.get::<Option<String>, _>("last_verified_at").map(|t| {
                DateTime::parse_from_rfc3339(&t)
                    .unwrap()
                    .with_timezone(&Utc)
            }),
            created_by: row.get::<Option<String>, _>("created_by"),
            notes: row.get::<String, _>("notes"),
        })
    }

    /// Get veto statistics for a PR
    pub async fn get_veto_statistics(
        &self,
        pr_id: i32,
    ) -> Result<serde_json::Value, GovernanceError> {
        let threshold = self.check_veto_threshold(pr_id).await?;
        let signals = self.get_pr_veto_signals(pr_id).await?;

        let mining_signals = 0;
        let economic_signals = 0;
        let mut veto_count = 0;
        let mut support_count = 0;
        let mut abstain_count = 0;

        for signal in &signals {
            match signal.signal_type {
                SignalType::Veto => veto_count += 1,
                SignalType::Support => support_count += 1,
                SignalType::Abstain => abstain_count += 1,
            }
        }

        Ok(serde_json::json!({
            "threshold": {
                "mining_veto_percent": threshold.mining_veto_percent,
                "economic_veto_percent": threshold.economic_veto_percent,
                "threshold_met": threshold.threshold_met,
                "veto_active": threshold.veto_active
            },
            "signals": {
                "total": signals.len(),
                "veto": veto_count,
                "support": support_count,
                "abstain": abstain_count,
                "mining_signals": mining_signals,
                "economic_signals": economic_signals
            }
        }))
    }
}
