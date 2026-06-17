//! Governance review case management
//!
//! Implements core policy principles:
//! - On-platform only (off-platform activity disregarded)
//! - Time limits (180 days for resolution)
//! - Response periods (30 days for subject)

use crate::governance_review::models::{GovernanceReviewCase, policy};
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};

pub struct GovernanceReviewCaseManager {
    pool: SqlitePool,
}

impl GovernanceReviewCaseManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new governance review case
    ///
    /// Policy: Only on-platform activity considered
    pub async fn create_case(
        &self,
        subject_maintainer_id: i32,
        reporter_maintainer_id: i32,
        case_type: &str,
        severity: &str,
        description: &str,
        evidence: serde_json::Value,
        on_platform: bool,
    ) -> Result<GovernanceReviewCase, sqlx::Error> {
        // Policy: Off-platform activity disregarded
        if !on_platform {
            return Err(sqlx::Error::RowNotFound); // Or custom error type
        }

        // Calculate deadlines (policy: 30 days response, 180 days resolution)
        let now = Utc::now();
        let response_deadline = now + Duration::days(policy::RESPONSE_DEADLINE_DAYS);
        let resolution_deadline = now + Duration::days(policy::RESOLUTION_DEADLINE_DAYS);

        let evidence_json =
            serde_json::to_string(&evidence).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        // Start transaction for atomic case + time limits creation
        let mut tx = self.pool.begin().await?;

        // Generate case number with retry logic for collision handling
        let case_number = Self::generate_case_number_with_retry(&mut tx, now).await?;

        let case_id: i32 = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO governance_review_cases (
                case_number, subject_maintainer_id, reporter_maintainer_id,
                case_type, severity, status, description, evidence, on_platform,
                response_deadline, resolution_deadline
            )
            VALUES (?, ?, ?, ?, ?, 'open', ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(&case_number)
        .bind(subject_maintainer_id)
        .bind(reporter_maintainer_id)
        .bind(case_type)
        .bind(severity)
        .bind(description)
        .bind(&evidence_json)
        .bind(on_platform)
        .bind(response_deadline)
        .bind(resolution_deadline)
        .fetch_one(&mut *tx)
        .await?;

        // Create time limit tracking within same transaction
        // Response deadline
        sqlx::query(
            r#"
            INSERT INTO governance_review_time_limits (case_id, limit_type, deadline)
            VALUES (?, 'response', ?)
            "#,
        )
        .bind(case_id)
        .bind(response_deadline)
        .execute(&mut *tx)
        .await?;

        // Resolution deadline
        sqlx::query(
            r#"
            INSERT INTO governance_review_time_limits (case_id, limit_type, deadline)
            VALUES (?, 'resolution', ?)
            "#,
        )
        .bind(case_id)
        .bind(resolution_deadline)
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        // Get the created case
        self.get_case_by_id(case_id).await
    }

    /// Generate case number with retry logic to handle collisions
    async fn generate_case_number_with_retry(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        now: DateTime<Utc>,
    ) -> Result<String, sqlx::Error> {
        let base_format = now.format("%Y-%m%d");
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 10;

        loop {
            // Generate case number: GR-YYYY-MMDD-NNNN
            // Use timestamp + attempts to reduce collision chance
            let suffix = (now.timestamp() % 10000) as u32 + attempts;
            let case_number = format!("GR-{base_format}-{suffix:04}");

            // Check if case number already exists
            let exists: Option<i32> =
                sqlx::query_scalar("SELECT id FROM governance_review_cases WHERE case_number = ?")
                    .bind(&case_number)
                    .fetch_optional(&mut **tx)
                    .await?;

            if exists.is_none() {
                return Ok(case_number);
            }

            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                // Fallback to UUID-based case number if too many collisions
                use uuid::Uuid;
                let uuid_str = Uuid::new_v4().to_string();
                let uuid_part = uuid_str.replace("-", "")[..8].to_string();
                return Ok(format!("GR-{base_format}-{uuid_part}"));
            }
        }
    }

    /// Get case by ID
    pub async fn get_case_by_id(&self, case_id: i32) -> Result<GovernanceReviewCase, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, case_number, subject_maintainer_id, reporter_maintainer_id,
                case_type, severity, status, description, evidence, on_platform,
                created_at, response_deadline, resolution_deadline,
                resolved_at, resolution_reason, github_issue_number
            FROM governance_review_cases
            WHERE id = ?
            "#,
        )
        .bind(case_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(GovernanceReviewCase {
            id: row.get(0),
            case_number: row.get(1),
            subject_maintainer_id: row.get(2),
            reporter_maintainer_id: row.get(3),
            case_type: row.get(4),
            severity: row.get(5),
            status: row.get(6),
            description: row.get(7),
            evidence: serde_json::from_str(row.get::<String, _>(8).as_str()).unwrap_or_default(),
            on_platform: row.get(9),
            created_at: row.get(10),
            response_deadline: row.get(11),
            resolution_deadline: row.get(12),
            resolved_at: row.get(13),
            resolution_reason: row.get(14),
            github_issue_number: row.get::<Option<i64>, _>(15).map(|v| v as u64),
        })
    }

    /// Get cases by maintainer (subject or reporter)
    pub async fn get_cases_by_maintainer(
        &self,
        maintainer_id: i32,
        as_subject: bool,
    ) -> Result<Vec<GovernanceReviewCase>, sqlx::Error> {
        let rows = if as_subject {
            sqlx::query(
                r#"
                SELECT 
                    id, case_number, subject_maintainer_id, reporter_maintainer_id,
                    case_type, severity, status, description, evidence, on_platform,
                    created_at, response_deadline, resolution_deadline,
                    resolved_at, resolution_reason, github_issue_number
                FROM governance_review_cases 
                WHERE subject_maintainer_id = ? 
                ORDER BY created_at DESC
                "#,
            )
            .bind(maintainer_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT 
                    id, case_number, subject_maintainer_id, reporter_maintainer_id,
                    case_type, severity, status, description, evidence, on_platform,
                    created_at, response_deadline, resolution_deadline,
                    resolved_at, resolution_reason, github_issue_number
                FROM governance_review_cases 
                WHERE reporter_maintainer_id = ? 
                ORDER BY created_at DESC
                "#,
            )
            .bind(maintainer_id)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter()
            .map(|row| {
                Ok(GovernanceReviewCase {
                    id: row.get(0),
                    case_number: row.get(1),
                    subject_maintainer_id: row.get(2),
                    reporter_maintainer_id: row.get(3),
                    case_type: row.get(4),
                    severity: row.get(5),
                    status: row.get(6),
                    description: row.get(7),
                    evidence: serde_json::from_str(row.get::<String, _>(8).as_str())
                        .unwrap_or_default(),
                    on_platform: row.get(9),
                    created_at: row.get(10),
                    response_deadline: row.get(11),
                    resolution_deadline: row.get(12),
                    resolved_at: row.get(13),
                    resolution_reason: row.get(14),
                    github_issue_number: row.get::<Option<i64>, _>(15).map(|v| v as u64),
                })
            })
            .collect()
    }

    /// Check if case is expired (policy: 180 days)
    pub async fn check_expired_cases(&self) -> Result<Vec<i32>, sqlx::Error> {
        let expired = sqlx::query(
            r#"
            SELECT id FROM governance_review_cases
            WHERE status NOT IN ('resolved', 'dismissed', 'removed', 'expired')
            AND resolution_deadline < ?
            "#,
        )
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await?;

        // Mark as expired
        for row in &expired {
            let case_id: i32 = row.get(0);
            sqlx::query("UPDATE governance_review_cases SET status = 'expired' WHERE id = ?")
                .bind(case_id)
                .execute(&self.pool)
                .await?;
        }

        Ok(expired.iter().map(|row| row.get::<i32, _>(0)).collect())
    }

    /// Update case status
    pub async fn update_status(
        &self,
        case_id: i32,
        status: &str,
        resolution_reason: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let resolved_at = if status == "resolved" || status == "removed" || status == "dismissed" {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE governance_review_cases
            SET status = ?, resolution_reason = ?, resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(resolution_reason)
        .bind(resolved_at)
        .bind(case_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
