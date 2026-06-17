//! Models for governance review tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReviewCase {
    pub id: i32,
    pub case_number: String,
    pub subject_maintainer_id: i32,
    pub reporter_maintainer_id: i32,
    pub case_type: String, // 'abuse', 'harassment', 'malicious_code', etc.
    pub severity: String,  // 'minor', 'moderate', 'serious', 'gross_misconduct'
    pub status: String,    // 'open', 'under_review', 'mediation', etc.
    pub description: String,
    pub evidence: serde_json::Value,
    pub on_platform: bool, // Policy: only on-platform considered
    pub created_at: DateTime<Utc>,
    pub response_deadline: Option<DateTime<Utc>>,
    pub resolution_deadline: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_reason: Option<String>,
    pub github_issue_number: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReviewResponse {
    pub id: i32,
    pub case_id: i32,
    pub maintainer_id: i32,
    pub response_text: String,
    pub counter_evidence: serde_json::Value,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReviewWarning {
    pub id: i32,
    pub case_id: i32,
    pub maintainer_id: i32,
    pub warning_level: i32,           // 1 = private, 2 = public
    pub warning_type: String,         // 'private_warning', 'public_warning'
    pub issued_by_team_approval: i32, // Number of maintainers who approved
    pub issued_at: DateTime<Utc>,
    pub improvement_deadline: Option<DateTime<Utc>>,
    pub improvement_extended: bool,
    pub improvement_extended_until: Option<DateTime<Utc>>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
    pub warning_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanctionApproval {
    pub id: i32,
    pub case_id: i32,
    pub maintainer_id: i32,
    pub sanction_type: String,
    pub approved_at: DateTime<Utc>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mediation {
    pub id: i32,
    pub case_id: i32,
    pub mediator_maintainer_id: Option<i32>,
    pub mediation_started_at: DateTime<Utc>,
    pub mediation_deadline: Option<DateTime<Utc>>,
    pub status: String,
    pub resolution_notes: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appeal {
    pub id: i32,
    pub case_id: i32,
    pub maintainer_id: i32,
    pub appeal_reason: String,
    pub new_evidence: serde_json::Value,
    pub submitted_at: DateTime<Utc>,
    pub appeal_deadline: Option<DateTime<Utc>>,
    pub status: String,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_decision: Option<String>,
    pub teams_approval_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retaliation {
    pub id: i32,
    pub original_case_id: i32,
    pub reporter_maintainer_id: i32,
    pub retaliator_maintainer_id: i32,
    pub retaliation_type: String,
    pub description: String,
    pub reported_at: DateTime<Utc>,
    pub status: String,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FalseReport {
    pub id: i32,
    pub original_case_id: i32,
    pub false_reporter_maintainer_id: i32,
    pub confirmed_false_at: DateTime<Utc>,
    pub false_report_evidence: String,
    pub sanction_applied: String,
    pub sanction_case_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLimit {
    pub id: i32,
    pub case_id: i32,
    pub limit_type: String, // 'response', 'resolution', 'appeal', 'improvement'
    pub deadline: DateTime<Utc>,
    pub extended: bool,
    pub extension_approved_by: Option<i32>,
    pub extension_reason: Option<String>,
    pub extension_until: Option<DateTime<Utc>>,
}

// Policy constants
pub mod policy {

    // Time limits (from policy)
    pub const RESPONSE_DEADLINE_DAYS: i64 = 30;
    pub const RESOLUTION_DEADLINE_DAYS: i64 = 180;
    pub const APPEAL_DEADLINE_DAYS: i64 = 60;
    pub const IMPROVEMENT_PERIOD_DAYS: i64 = 90;
    pub const MEDIATION_PERIOD_DAYS: i64 = 30;
    pub const IMPROVEMENT_EXTENSION_DAYS: i64 = 30;
    pub const MAX_EXTENSION_DAYS: i64 = 90; // Maximum extension beyond original deadline

    // Thresholds (from policy)
    pub const PRIVATE_WARNING_THRESHOLD: i32 = 4; // 4-of-7 team
    pub const PUBLIC_WARNING_THRESHOLD: i32 = 5; // 5-of-7 team
    pub const REMOVAL_TEAM_THRESHOLD: i32 = 6; // 6-of-7 team
    pub const REMOVAL_TEAMS_THRESHOLD: i32 = 4; // 4-of-7 teams
    pub const APPEAL_OVERTURN_THRESHOLD: i32 = 5; // 5-of-7 teams

    // Case types (from policy)
    pub const CASE_TYPES: &[&str] = &[
        "abuse",
        "harassment",
        "malicious_code",
        "collusion",
        "conflict_of_interest",
        "technical_errors",
        "security_violation",
        "false_report",
        "retaliation",
    ];

    // Severity levels (from policy)
    pub const SEVERITY_LEVELS: &[&str] = &["minor", "moderate", "serious", "gross_misconduct"];

    // Status values
    pub const STATUS_OPEN: &str = "open";
    pub const STATUS_UNDER_REVIEW: &str = "under_review";
    pub const STATUS_MEDIATION: &str = "mediation";
    pub const STATUS_WARNING_ISSUED: &str = "warning_issued";
    pub const STATUS_REMOVAL_PENDING: &str = "removal_pending";
    pub const STATUS_REMOVED: &str = "removed";
    pub const STATUS_RESOLVED: &str = "resolved";
    pub const STATUS_DISMISSED: &str = "dismissed";
    pub const STATUS_EXPIRED: &str = "expired";
}
