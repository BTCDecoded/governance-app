//! Node Registry API endpoints

use axum::{
    Router,
    extract::State,
    response::Json,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::database::Database;
use crate::node_registry::{NodeRegistry, NodeType};

/// Register node request
#[derive(Debug, Deserialize)]
pub struct RegisterNodeRequest {
    pub node_id: String,
    pub node_name: String,
    pub node_type: String,
    pub bitcoin_addresses: Vec<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Node registration response
#[derive(Debug, Serialize)]
pub struct RegisterNodeResponse {
    pub success: bool,
    pub message: String,
}

/// Get node response
#[derive(Debug, Serialize)]
pub struct GetNodeResponse {
    pub node: Option<crate::node_registry::NodeRegistration>,
}

/// List nodes response
#[derive(Debug, Serialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<crate::node_registry::NodeRegistration>,
}

/// Register a new node
pub async fn register_node(
    State((_, database)): State<(crate::config::AppConfig, Database)>,
    Json(request): Json<RegisterNodeRequest>,
) -> Json<RegisterNodeResponse> {
    let pool = match database.get_sqlite_pool() {
        Some(pool) => pool,
        None => {
            return Json(RegisterNodeResponse {
                success: false,
                message: "Database pool not available".to_string(),
            });
        }
    };

    let registry = NodeRegistry::new(pool.clone());
    let node_type = NodeType::from_str(&request.node_type);

    match registry
        .register_node(
            &request.node_id,
            &request.node_name,
            node_type,
            request.bitcoin_addresses,
            request.metadata,
        )
        .await
    {
        Ok(_) => {
            info!("Node registered: {}", request.node_id);
            Json(RegisterNodeResponse {
                success: true,
                message: format!("Node {} registered successfully", request.node_id),
            })
        }
        Err(e) => {
            warn!("Failed to register node {}: {}", request.node_id, e);
            Json(RegisterNodeResponse {
                success: false,
                message: format!("Failed to register node: {e}"),
            })
        }
    }
}

/// Get node by ID
pub async fn get_node(
    State((_, database)): State<(crate::config::AppConfig, Database)>,
    axum::extract::Path(node_id): axum::extract::Path<String>,
) -> Json<GetNodeResponse> {
    let pool = match database.get_sqlite_pool() {
        Some(pool) => pool,
        None => {
            return Json(GetNodeResponse { node: None });
        }
    };

    let registry = NodeRegistry::new(pool.clone());
    let node = registry.get_node(&node_id).await.ok().flatten();

    Json(GetNodeResponse { node })
}

/// List all active nodes
pub async fn list_nodes(
    State((_, database)): State<(crate::config::AppConfig, Database)>,
) -> Json<ListNodesResponse> {
    let pool = match database.get_sqlite_pool() {
        Some(pool) => pool,
        None => {
            return Json(ListNodesResponse { nodes: Vec::new() });
        }
    };

    let registry = NodeRegistry::new(pool.clone());
    let nodes = registry.get_active_nodes().await.unwrap_or_default();

    Json(ListNodesResponse { nodes })
}

/// Create router for node registry API
pub fn create_router() -> Router<(crate::config::AppConfig, Database)> {
    Router::new()
        .route("/nodes/register", post(register_node))
        .route("/nodes/:node_id", get(get_node))
        .route("/nodes", get(list_nodes))
}
