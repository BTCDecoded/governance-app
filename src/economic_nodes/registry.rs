//! Economic Node Registry
//!
//! Handles registration, qualification verification, and weight calculation

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use tracing::{info, warn};

use super::types::*;
use crate::error::GovernanceError;

pub struct EconomicNodeRegistry {
    pool: SqlitePool,
}

impl EconomicNodeRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Register a new economic node with qualification proof
    pub async fn register_economic_node(
        &self,
        node_type: NodeType,
        entity_name: &str,
        public_key: &str,
        qualification_data: &QualificationProof,
        created_by: Option<&str>,
    ) -> Result<i32, GovernanceError> {
        // Verify qualification meets thresholds
        let verified = self
            .verify_qualification(node_type.clone(), qualification_data)
            .await?;
        if !verified {
            return Err(GovernanceError::CryptoError(
                "Node does not meet qualification thresholds".to_string(),
            ));
        }

        // Calculate initial weight
        let weight = self
            .calculate_weight(node_type.clone(), qualification_data)
            .await?;

        // Insert into database
        let result = sqlx::query(
            r#"
            INSERT INTO economic_nodes 
            (node_type, entity_name, public_key, qualification_data, weight, status, created_by)
            VALUES (?, ?, ?, ?, ?, 'pending', ?)
            "#,
        )
        .bind(node_type.as_str())
        .bind(entity_name)
        .bind(public_key)
        .bind(serde_json::to_string(qualification_data)?)
        .bind(weight)
        .bind(created_by)
        .execute(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to register node: {}", e)))?;

        let node_id = result.last_insert_rowid() as i32;
        info!("Registered economic node {} (ID: {})", entity_name, node_id);
        Ok(node_id)
    }

    /// Verify that a node meets qualification thresholds
    pub async fn verify_qualification(
        &self,
        node_type: NodeType,
        qualification_data: &QualificationProof,
    ) -> Result<bool, GovernanceError> {
        let thresholds = node_type.qualification_thresholds();

        // Check hashpower threshold (mining pools)
        if let Some(min_hashpower) = thresholds.minimum_hashpower_percent {
            if let Some(hashpower_proof) = &qualification_data.hashpower_proof {
                if hashpower_proof.percentage < min_hashpower {
                    warn!(
                        "Hashpower {}% below threshold {}%",
                        hashpower_proof.percentage, min_hashpower
                    );
                    return Ok(false);
                }
            } else {
                warn!("Hashpower proof required for mining pools");
                return Ok(false);
            }
        }

        // Check holdings threshold
        if let Some(min_holdings) = thresholds.minimum_holdings_btc {
            if let Some(holdings_proof) = &qualification_data.holdings_proof {
                if holdings_proof.total_btc < min_holdings as f64 {
                    warn!(
                        "Holdings {} BTC below threshold {} BTC",
                        holdings_proof.total_btc, min_holdings
                    );
                    return Ok(false);
                }
            } else {
                warn!("Holdings proof required for this node type");
                return Ok(false);
            }
        }

        // Check volume threshold
        if let Some(min_volume) = thresholds.minimum_volume_usd {
            if let Some(volume_proof) = &qualification_data.volume_proof {
                let volume = if node_type == NodeType::Exchange {
                    volume_proof.daily_volume_usd
                } else {
                    volume_proof.monthly_volume_usd
                };

                if volume < min_volume as f64 {
                    warn!("Volume ${} below threshold ${}", volume, min_volume);
                    return Ok(false);
                }
            } else {
                warn!("Volume proof required for this node type");
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Calculate weight for an economic node
    pub async fn calculate_weight(
        &self,
        node_type: NodeType,
        qualification_data: &QualificationProof,
    ) -> Result<f64, GovernanceError> {
        match node_type {
            NodeType::MiningPool => {
                // Weight = hashpower percentage
                if let Some(hashpower_proof) = &qualification_data.hashpower_proof {
                    Ok(hashpower_proof.percentage / 100.0)
                } else {
                    Err(GovernanceError::CryptoError(
                        "Hashpower proof required for mining pools".to_string(),
                    ))
                }
            }
            NodeType::Exchange => {
                // Weight = 70% holdings + 30% volume (trust-discounted)
                let holdings_weight =
                    if let Some(holdings_proof) = &qualification_data.holdings_proof {
                        // Normalize to 0-1 scale (10K BTC = 1.0)
                        (holdings_proof.total_btc / 10_000.0).min(1.0) * 0.7
                    } else {
                        0.0
                    };

                let volume_weight = if let Some(volume_proof) = &qualification_data.volume_proof {
                    // Normalize to 0-1 scale ($100M daily = 1.0)
                    (volume_proof.daily_volume_usd / 100_000_000.0).min(1.0) * 0.3
                } else {
                    0.0
                };

                Ok(holdings_weight + volume_weight)
            }
            NodeType::Custodian => {
                // Weight = holdings percentage
                if let Some(holdings_proof) = &qualification_data.holdings_proof {
                    // Normalize to 0-1 scale (10K BTC = 1.0)
                    Ok((holdings_proof.total_btc / 10_000.0).min(1.0))
                } else {
                    Err(GovernanceError::CryptoError(
                        "Holdings proof required for custodians".to_string(),
                    ))
                }
            }
            NodeType::PaymentProcessor => {
                // Weight = transaction volume
                if let Some(volume_proof) = &qualification_data.volume_proof {
                    // Normalize to 0-1 scale ($50M monthly = 1.0)
                    Ok((volume_proof.monthly_volume_usd / 50_000_000.0).min(1.0))
                } else {
                    Err(GovernanceError::CryptoError(
                        "Volume proof required for payment processors".to_string(),
                    ))
                }
            }
            NodeType::MajorHolder => {
                // Weight = holdings percentage
                if let Some(holdings_proof) = &qualification_data.holdings_proof {
                    // Normalize to 0-1 scale (5K BTC = 1.0)
                    Ok((holdings_proof.total_btc / 5_000.0).min(1.0))
                } else {
                    Err(GovernanceError::CryptoError(
                        "Holdings proof required for major holders".to_string(),
                    ))
                }
            }
        }
    }

    /// Get all active economic nodes
    pub async fn get_active_nodes(&self) -> Result<Vec<EconomicNode>, GovernanceError> {
        let rows = sqlx::query(
            r#"
            SELECT id, node_type, entity_name, public_key, qualification_data, 
                   weight, status, registered_at, verified_at, last_verified_at, 
                   created_by, notes
            FROM economic_nodes 
            WHERE status = 'active'
            ORDER BY weight DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to fetch nodes: {}", e)))?;

        let mut nodes = Vec::new();
        for row in rows {
            let node_type =
                NodeType::from_str(&row.get::<String, _>("node_type")).ok_or_else(|| {
                    GovernanceError::CryptoError(format!(
                        "Invalid node type: {}",
                        row.get::<String, _>("node_type")
                    ))
                })?;

            let status =
                NodeStatus::from_str(&row.get::<String, _>("status")).ok_or_else(|| {
                    GovernanceError::CryptoError(format!(
                        "Invalid status: {}",
                        row.get::<String, _>("status")
                    ))
                })?;

            nodes.push(EconomicNode {
                id: Some(row.get::<i32, _>("id")),
                node_type,
                entity_name: row.get::<String, _>("entity_name"),
                public_key: row.get::<String, _>("public_key"),
                qualification_data: serde_json::from_str(
                    &row.get::<String, _>("qualification_data"),
                )?,
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
            });
        }

        Ok(nodes)
    }

    /// Update node status
    pub async fn update_node_status(
        &self,
        node_id: i32,
        status: NodeStatus,
    ) -> Result<(), GovernanceError> {
        sqlx::query("UPDATE economic_nodes SET status = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(node_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to update status: {}", e))
            })?;

        info!("Updated node {} status to {}", node_id, status.as_str());
        Ok(())
    }

    /// Recalculate weights for all nodes (for periodic updates)
    pub async fn recalculate_all_weights(&self) -> Result<(), GovernanceError> {
        let nodes = self.get_active_nodes().await?;

        for node in nodes {
            if let Some(node_id) = node.id {
                // Recalculate weight based on current qualification data
                let qualification_data: QualificationProof =
                    serde_json::from_value(node.qualification_data.clone())?;

                let new_weight = self
                    .calculate_weight(node.node_type.clone(), &qualification_data)
                    .await?;

                sqlx::query("UPDATE economic_nodes SET weight = ? WHERE id = ?")
                    .bind(new_weight)
                    .bind(node_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| {
                        GovernanceError::DatabaseError(format!("Failed to update weight: {}", e))
                    })?;
            }
        }

        info!("Recalculated weights for all active nodes");
        Ok(())
    }
}
