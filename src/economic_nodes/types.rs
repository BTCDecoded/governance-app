//! Economic Node Types and Data Structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Types of economic nodes that can participate in governance
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    MiningPool,
    Exchange,
    Custodian,
    PaymentProcessor,
    MajorHolder,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::MiningPool => "mining_pool",
            NodeType::Exchange => "exchange",
            NodeType::Custodian => "custodian",
            NodeType::PaymentProcessor => "payment_processor",
            NodeType::MajorHolder => "major_holder",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mining_pool" => Some(NodeType::MiningPool),
            "exchange" => Some(NodeType::Exchange),
            "custodian" => Some(NodeType::Custodian),
            "payment_processor" => Some(NodeType::PaymentProcessor),
            "major_holder" => Some(NodeType::MajorHolder),
            _ => None,
        }
    }

    /// Get minimum qualification thresholds for this node type
    pub fn qualification_thresholds(&self) -> QualificationThresholds {
        match self {
            NodeType::MiningPool => QualificationThresholds {
                minimum_hashpower_percent: Some(1.0),
                minimum_holdings_btc: None,
                minimum_volume_usd: None,
                minimum_transactions_monthly: None,
            },
            NodeType::Exchange => QualificationThresholds {
                minimum_hashpower_percent: None,
                minimum_holdings_btc: Some(10_000),
                minimum_volume_usd: Some(100_000_000), // $100M daily
                minimum_transactions_monthly: None,
            },
            NodeType::Custodian => QualificationThresholds {
                minimum_hashpower_percent: None,
                minimum_holdings_btc: Some(10_000),
                minimum_volume_usd: None,
                minimum_transactions_monthly: None,
            },
            NodeType::PaymentProcessor => QualificationThresholds {
                minimum_hashpower_percent: None,
                minimum_holdings_btc: None,
                minimum_volume_usd: Some(50_000_000), // $50M monthly
                minimum_transactions_monthly: None,
            },
            NodeType::MajorHolder => QualificationThresholds {
                minimum_hashpower_percent: None,
                minimum_holdings_btc: Some(5_000),
                minimum_volume_usd: None,
                minimum_transactions_monthly: None,
            },
        }
    }
}

/// Qualification thresholds for different node types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationThresholds {
    pub minimum_hashpower_percent: Option<f64>,
    pub minimum_holdings_btc: Option<u64>,
    pub minimum_volume_usd: Option<u64>,
    pub minimum_transactions_monthly: Option<u64>,
}

/// Economic node registration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicNode {
    pub id: Option<i32>,
    pub node_type: NodeType,
    pub entity_name: String,
    pub public_key: String,
    pub qualification_data: serde_json::Value,
    pub weight: f64,
    pub status: NodeStatus,
    pub registered_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
    pub notes: String,
}

/// Node status in the registry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Active,
    Suspended,
    Removed,
}

impl NodeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeStatus::Pending => "pending",
            NodeStatus::Active => "active",
            NodeStatus::Suspended => "suspended",
            NodeStatus::Removed => "removed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(NodeStatus::Pending),
            "active" => Some(NodeStatus::Active),
            "suspended" => Some(NodeStatus::Suspended),
            "removed" => Some(NodeStatus::Removed),
            _ => None,
        }
    }
}

/// Veto signal from an economic node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VetoSignal {
    pub id: Option<i32>,
    pub pr_id: i32,
    pub node_id: i32,
    pub signal_type: SignalType,
    pub weight: f64,
    pub signature: String,
    pub rationale: String,
    pub timestamp: DateTime<Utc>,
    pub verified: bool,
}

/// Type of signal from economic node
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    Veto,
    Support,
    Abstain,
}

impl SignalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SignalType::Veto => "veto",
            SignalType::Support => "support",
            SignalType::Abstain => "abstain",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "veto" => Some(SignalType::Veto),
            "support" => Some(SignalType::Support),
            "abstain" => Some(SignalType::Abstain),
            _ => None,
        }
    }
}

/// Veto threshold calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VetoThreshold {
    pub mining_veto_percent: f64,
    pub economic_veto_percent: f64,
    pub threshold_met: bool,
    pub veto_active: bool,
}

/// Economic node qualification proof data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualificationProof {
    pub node_type: NodeType,
    pub hashpower_proof: Option<HashpowerProof>,
    pub holdings_proof: Option<HoldingsProof>,
    pub volume_proof: Option<VolumeProof>,
    pub contact_info: ContactInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashpowerProof {
    pub blocks_mined: Vec<String>, // Block hashes
    pub time_period_days: u32,
    pub total_network_blocks: u32,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingsProof {
    pub addresses: Vec<String>, // Bitcoin addresses
    pub total_btc: f64,
    pub signature_challenge: String, // Signature proving control
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProof {
    pub daily_volume_usd: f64,
    pub monthly_volume_usd: f64,
    pub data_source: String, // External data provider
    pub verification_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub entity_name: String,
    pub contact_email: String,
    pub website: Option<String>,
    pub github_username: Option<String>,
}
