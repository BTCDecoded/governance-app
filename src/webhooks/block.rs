//! Block webhook handler
//!
//! Receives block notifications from blvm-node (fee forwarding removed)

use axum::{extract::State, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::AppConfig;
use crate::database::Database;

/// Block notification payload
/// Block should be provided as JSON object that can be deserialized to blvm_protocol::Block
#[derive(Debug, Deserialize)]
pub struct BlockNotification {
    pub block_hash: String,
    pub block_height: i32,
    pub block: Value, // Block data as JSON - will be converted to blvm_protocol::Block
    pub contributor_id: Option<String>, // Optional: node/miner identifier
}

/// Block notification response
#[derive(Debug, Serialize)]
pub struct BlockNotificationResponse {
    pub success: bool,
    pub message: String,
    pub contributions_found: usize,
}

/// Handle block notification webhook.
/// Block notifications are acknowledged but not processed (fee forwarding was removed).
pub async fn handle_block_notification(
    State((_config, _database)): State<(AppConfig, Database)>,
    Json(_payload): Json<BlockNotification>,
) -> Json<BlockNotificationResponse> {
    Json(BlockNotificationResponse {
        success: true,
        message: "Block notification acknowledged".to_string(),
        contributions_found: 0,
    })
}
