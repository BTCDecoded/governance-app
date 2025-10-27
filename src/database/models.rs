use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: i32,
    pub repo_name: String,
    pub pr_number: i32,
    pub opened_at: DateTime<Utc>,
    pub layer: i32,
    pub head_sha: String,
    pub signatures: Vec<Signature>,
    pub governance_status: String,
    pub linked_prs: Vec<i32>,
    pub emergency_mode: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub signer: String,
    pub signature: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Maintainer {
    pub id: i32,
    pub github_username: String,
    pub public_key: String,
    pub layer: i32,
    pub active: bool,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyKeyholder {
    pub id: i32,
    pub github_username: String,
    pub public_key: String,
    pub active: bool,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEvent {
    pub id: i32,
    pub event_type: String,
    pub repo_name: Option<String>,
    pub pr_number: Option<i32>,
    pub maintainer: Option<String>,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}
