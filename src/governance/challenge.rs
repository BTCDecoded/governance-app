//! Challenge Mechanism for Governance Decisions
//!
//! Implements cryptographically signed challenges for PRs, governance decisions,
//! and maintainer actions. Enables spontaneous discovery of problems without
//! prescriptive enforcement.

use crate::crypto::signatures::SignatureManager;
use crate::error::GovernanceError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub target_type: ChallengeTarget,
    pub target_id: String,  // PR number or decision ID
    pub challenger: String, // GitHub username
    pub reason: String,
    pub signature: String, // Cryptographic signature
    pub status: ChallengeStatus,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution: Option<String>,
    pub resolver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChallengeTarget {
    PullRequest,
    GovernanceDecision,
    MaintainerAction,
    InsufficientReview,
}

impl ChallengeTarget {
    pub fn from_str(s: &str) -> Result<Self, GovernanceError> {
        match s.to_lowercase().as_str() {
            "pull_request" | "pr" => Ok(ChallengeTarget::PullRequest),
            "governance_decision" | "decision" => Ok(ChallengeTarget::GovernanceDecision),
            "maintainer_action" | "action" => Ok(ChallengeTarget::MaintainerAction),
            "insufficient_review" | "review" => Ok(ChallengeTarget::InsufficientReview),
            _ => Err(GovernanceError::ValidationError(format!(
                "Invalid challenge target: {s}"
            ))),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ChallengeTarget::PullRequest => "pull_request".to_string(),
            ChallengeTarget::GovernanceDecision => "governance_decision".to_string(),
            ChallengeTarget::MaintainerAction => "maintainer_action".to_string(),
            ChallengeTarget::InsufficientReview => "insufficient_review".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChallengeStatus {
    Pending,
    UnderReview,
    Resolved,
    Rejected,
}

impl ChallengeStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => ChallengeStatus::Pending,
            "under_review" => ChallengeStatus::UnderReview,
            "resolved" => ChallengeStatus::Resolved,
            "rejected" => ChallengeStatus::Rejected,
            _ => ChallengeStatus::Pending,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ChallengeStatus::Pending => "pending".to_string(),
            ChallengeStatus::UnderReview => "under_review".to_string(),
            ChallengeStatus::Resolved => "resolved".to_string(),
            ChallengeStatus::Rejected => "rejected".to_string(),
        }
    }
}

pub struct ChallengeManager {
    pool: SqlitePool,
}

impl ChallengeManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new challenge (cryptographically signed)
    pub async fn create_challenge(
        &self,
        target_type: ChallengeTarget,
        target_id: String,
        challenger: String,
        reason: String,
        signature: String,
    ) -> Result<String, GovernanceError> {
        // Verify signature
        // Message format: "challenge:{target_type}:{target_id}:{reason}"
        let message = format!(
            "challenge:{}:{}:{}",
            target_type.to_string(),
            target_id,
            reason
        );

        // Get challenger's public key (if they're a maintainer)
        // Note: Challenges can be from anyone, but we verify signature if maintainer
        let signature_manager = SignatureManager::new();

        // Try to get maintainer public key, but don't require it
        // (challenges can come from non-maintainers too)
        let signature_valid = if let Ok(Some(maintainer)) =
            sqlx::query_as::<_, crate::database::models::Maintainer>(
                "SELECT id, github_username, public_key, layer, active, last_updated FROM maintainers WHERE github_username = ? AND active = true"
            )
            .bind(&challenger)
            .fetch_optional(&self.pool)
            .await
        {
            signature_manager
                .verify_governance_signature(&message, &signature, &maintainer.public_key)
                .unwrap_or(false)
        } else {
            // Non-maintainer challenge - signature verification optional
            // For now, we accept it (could add alternative verification later)
            true
        };

        if !signature_valid {
            return Err(GovernanceError::SignatureError(
                "Invalid challenge signature".to_string(),
            ));
        }

        // Generate challenge ID: challenge-<timestamp>-<random>
        let timestamp = Utc::now().timestamp();
        let random_suffix = rand::random::<u32>();
        let challenge_id = format!("challenge-{timestamp}-{random_suffix:08x}");
        let target_type_str = target_type.to_string();
        let now = Utc::now();

        sqlx::query!(
            r#"
            INSERT INTO challenges (
                id, target_type, target_id, challenger, reason, 
                signature, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            challenge_id,
            target_type_str,
            target_id,
            challenger,
            reason,
            signature,
            "pending",
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(challenge_id)
    }

    /// Resolve a challenge
    pub async fn resolve_challenge(
        &self,
        challenge_id: &str,
        resolution: String,
        resolver: String,
    ) -> Result<(), GovernanceError> {
        let now = Utc::now();
        sqlx::query!(
            r#"
            UPDATE challenges
            SET status = ?, resolution = ?, resolver = ?, resolved_at = ?
            WHERE id = ?
            "#,
            "resolved",
            resolution,
            resolver,
            now,
            challenge_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Reject a challenge
    pub async fn reject_challenge(
        &self,
        challenge_id: &str,
        reason: String,
        resolver: String,
    ) -> Result<(), GovernanceError> {
        let now = Utc::now();
        sqlx::query!(
            r#"
            UPDATE challenges
            SET status = ?, resolution = ?, resolver = ?, resolved_at = ?
            WHERE id = ?
            "#,
            "rejected",
            reason,
            resolver,
            now,
            challenge_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all challenges for a target
    pub async fn get_challenges_for_target(
        &self,
        target_type: &ChallengeTarget,
        target_id: &str,
    ) -> Result<Vec<Challenge>, GovernanceError> {
        let target_type_str = target_type.to_string();
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, target_type, target_id, challenger, reason, signature,
                status, created_at, resolved_at, resolution, resolver
            FROM challenges
            WHERE target_type = ? AND target_id = ?
            ORDER BY created_at DESC
            "#,
            target_type_str,
            target_id
        )
        .fetch_all(&self.pool)
        .await?;

        let challenges = rows
            .into_iter()
            .map(|row| Challenge {
                id: row.id.unwrap_or_default(),
                target_type: ChallengeTarget::from_str(&row.target_type)
                    .unwrap_or(ChallengeTarget::PullRequest),
                target_id: row.target_id,
                challenger: row.challenger,
                reason: row.reason,
                signature: row.signature,
                status: ChallengeStatus::from_str(&row.status),
                created_at: row.created_at.and_utc(),
                resolved_at: row.resolved_at.map(|dt| dt.and_utc()),
                resolution: row.resolution,
                resolver: row.resolver,
            })
            .collect();

        Ok(challenges)
    }

    /// Get pending challenges that need response (30-day deadline)
    pub async fn get_pending_challenges(&self) -> Result<Vec<Challenge>, GovernanceError> {
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        let cutoff_str = thirty_days_ago.to_rfc3339();
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, target_type, target_id, challenger, reason, signature,
                status, created_at, resolved_at, resolution, resolver
            FROM challenges
            WHERE status IN ('pending', 'under_review')
            AND created_at < ?
            ORDER BY created_at ASC
            "#,
            cutoff_str
        )
        .fetch_all(&self.pool)
        .await?;

        let challenges = rows
            .into_iter()
            .map(|row| Challenge {
                id: row.id.unwrap_or_default(),
                target_type: ChallengeTarget::from_str(&row.target_type)
                    .unwrap_or(ChallengeTarget::PullRequest),
                target_id: row.target_id,
                challenger: row.challenger,
                reason: row.reason,
                signature: row.signature,
                status: ChallengeStatus::from_str(&row.status),
                created_at: row.created_at.and_utc(),
                resolved_at: row.resolved_at.map(|dt| dt.and_utc()),
                resolution: row.resolution,
                resolver: row.resolver,
            })
            .collect();

        Ok(challenges)
    }

    /// Check if review challenge is valid (for insufficient review challenges)
    pub async fn validate_review_challenge(
        &self,
        repo_name: &str,
        pr_number: i32,
    ) -> Result<bool, GovernanceError> {
        // Check if PR has signatures without reviews
        // Signatures are stored as JSON in pull_requests.signatures
        let (signatures_without_review,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM (
                SELECT 1
                FROM pull_requests pr
                CROSS JOIN json_each(pr.signatures) sig
                LEFT JOIN reviews r ON r.repo_name = pr.repo_name 
                    AND r.pr_number = pr.pr_number 
                    AND r.reviewer = json_extract(sig.value, '$.signer')
                WHERE pr.repo_name = ? AND pr.pr_number = ?
                AND r.id IS NULL
            )
            "#,
        )
        .bind(repo_name)
        .bind(pr_number)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        Ok(signatures_without_review > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_target_from_str() {
        assert!(matches!(
            ChallengeTarget::from_str("pull_request").unwrap(),
            ChallengeTarget::PullRequest
        ));
        assert!(matches!(
            ChallengeTarget::from_str("pr").unwrap(),
            ChallengeTarget::PullRequest
        ));
        assert!(matches!(
            ChallengeTarget::from_str("insufficient_review").unwrap(),
            ChallengeTarget::InsufficientReview
        ));
        assert!(ChallengeTarget::from_str("invalid").is_err());
    }

    #[test]
    fn test_challenge_status_from_str() {
        assert!(matches!(
            ChallengeStatus::from_str("pending"),
            ChallengeStatus::Pending
        ));
        assert!(matches!(
            ChallengeStatus::from_str("resolved"),
            ChallengeStatus::Resolved
        ));
    }
}
