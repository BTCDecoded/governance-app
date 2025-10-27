//! Adoption Dashboard
//!
//! Provides HTTP endpoints for viewing governance adoption metrics

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::Value;

// use crate::error::GovernanceError;
use super::adoption::AdoptionTracker;
use super::types::*;

#[derive(Clone)]
pub struct AdoptionDashboard {
    adoption_tracker: AdoptionTracker,
}

impl AdoptionDashboard {
    pub fn new(adoption_tracker: AdoptionTracker) -> Self {
        Self { adoption_tracker }
    }

    /// Create the dashboard router
    pub fn router(self) -> Router {
        Router::new()
            .route("/adoption-metrics", get(get_adoption_metrics))
            .route("/ruleset/:ruleset_id/metrics", get(get_ruleset_metrics))
            .route("/ruleset/:ruleset_id/history", get(get_ruleset_history))
            .route("/health", get(health_check))
            .with_state(self)
    }
}

/// Get overall adoption metrics
pub async fn get_adoption_metrics(
    State(dashboard): State<AdoptionDashboard>,
) -> Result<Json<Value>, StatusCode> {
    match dashboard.adoption_tracker.get_adoption_statistics().await {
        Ok(stats) => {
            let response = serde_json::json!({
                "status": "success",
                "data": {
                    "total_nodes": stats.total_nodes,
                    "total_hashpower": stats.total_hashpower,
                    "total_economic_activity": stats.total_economic_activity,
                    "winning_ruleset": stats.winning_ruleset,
                    "adoption_percentage": stats.adoption_percentage,
                    "last_updated": stats.last_updated,
                    "rulesets": stats.rulesets
                }
            });
            Ok(Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get adoption metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get metrics for a specific ruleset
pub async fn get_ruleset_metrics(
    axum::extract::Path(ruleset_id): axum::extract::Path<String>,
    State(dashboard): State<AdoptionDashboard>,
) -> Result<Json<Value>, StatusCode> {
    match dashboard
        .adoption_tracker
        .calculate_adoption_metrics(&ruleset_id)
        .await
    {
        Ok(metrics) => {
            let response = serde_json::json!({
                "status": "success",
                "data": {
                    "ruleset_id": metrics.ruleset_id,
                    "node_count": metrics.node_count,
                    "hashpower_percentage": metrics.hashpower_percentage,
                    "economic_activity_percentage": metrics.economic_activity_percentage,
                    "total_weight": metrics.total_weight,
                    "last_updated": metrics.last_updated
                }
            });
            Ok(Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get ruleset metrics for {}: {}", ruleset_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get adoption history for a ruleset
pub async fn get_ruleset_history(
    axum::extract::Path(ruleset_id): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    State(dashboard): State<AdoptionDashboard>,
) -> Result<Json<Value>, StatusCode> {
    let days = params
        .get("days")
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(30);

    match dashboard
        .adoption_tracker
        .get_adoption_history(&ruleset_id, days)
        .await
    {
        Ok(history) => {
            let response = serde_json::json!({
                "status": "success",
                "data": {
                    "ruleset_id": ruleset_id,
                    "days": days,
                    "history": history
                }
            });
            Ok(Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get ruleset history for {}: {}", ruleset_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Health check endpoint
pub async fn health_check() -> Result<Json<Value>, StatusCode> {
    let response = serde_json::json!({
        "status": "healthy",
        "service": "governance-adoption-dashboard",
        "timestamp": chrono::Utc::now()
    });
    Ok(Json(response))
}

/// Create a simple adoption dashboard HTML page
pub fn create_dashboard_html(statistics: &AdoptionStatistics) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Governance Adoption Dashboard</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .header {{ background: #f0f0f0; padding: 20px; border-radius: 5px; margin-bottom: 20px; }}
        .metrics {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; }}
        .metric {{ background: #fff; border: 1px solid #ddd; padding: 15px; border-radius: 5px; }}
        .metric h3 {{ margin: 0 0 10px 0; color: #333; }}
        .metric .value {{ font-size: 24px; font-weight: bold; color: #2c5aa0; }}
        .ruleset {{ background: #f9f9f9; border: 1px solid #ddd; padding: 15px; margin: 10px 0; border-radius: 5px; }}
        .ruleset h4 {{ margin: 0 0 10px 0; }}
        .progress {{ background: #e0e0e0; height: 20px; border-radius: 10px; overflow: hidden; }}
        .progress-bar {{ background: #4CAF50; height: 100%; transition: width 0.3s; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Governance Adoption Dashboard</h1>
        <p>Last updated: {}</p>
    </div>
    
    <div class="metrics">
        <div class="metric">
            <h3>Total Nodes</h3>
            <div class="value">{}</div>
        </div>
        <div class="metric">
            <h3>Total Hashpower</h3>
            <div class="value">{:.1}%</div>
        </div>
        <div class="metric">
            <h3>Economic Activity</h3>
            <div class="value">{:.1}%</div>
        </div>
        <div class="metric">
            <h3>Adoption Rate</h3>
            <div class="value">{:.1}%</div>
        </div>
    </div>
    
    <h2>Ruleset Adoption</h2>
    {}
    
    <script>
        // Auto-refresh every 30 seconds
        setTimeout(() => location.reload(), 30000);
    </script>
</body>
</html>
        "#,
        statistics.last_updated.format("%Y-%m-%d %H:%M:%S UTC"),
        statistics.total_nodes,
        statistics.total_hashpower,
        statistics.total_economic_activity,
        statistics.adoption_percentage,
        statistics
            .rulesets
            .iter()
            .map(|ruleset| {
                format!(
                    r#"
                <div class="ruleset">
                    <h4>Ruleset: {}</h4>
                    <p>Nodes: {} | Hashpower: {:.1}% | Economic: {:.1}%</p>
                    <div class="progress">
                        <div class="progress-bar" style="width: {:.1}%"></div>
                    </div>
                </div>
                "#,
                    ruleset.ruleset_id,
                    ruleset.node_count,
                    ruleset.hashpower_percentage,
                    ruleset.economic_activity_percentage,
                    ruleset.total_weight
                )
            })
            .collect::<Vec<_>>()
            .join("")
    )
}
