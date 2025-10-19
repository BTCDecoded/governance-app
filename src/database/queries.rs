use sqlx::PgPool;
use crate::database::models::*;

pub struct Queries;

impl Queries {
    pub async fn get_pull_request(
        pool: &PgPool,
        repo_name: &str,
        pr_number: i32,
    ) -> Result<Option<PullRequest>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT id, repo_name, pr_number, opened_at, layer, head_sha, 
                   signatures, governance_status, linked_prs, emergency_mode, 
                   created_at, updated_at
            FROM pull_requests 
            WHERE repo_name = $1 AND pr_number = $2
            "#,
            repo_name,
            pr_number
        )
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(PullRequest {
                id: row.id,
                repo_name: row.repo_name,
                pr_number: row.pr_number,
                opened_at: row.opened_at,
                layer: row.layer,
                head_sha: row.head_sha,
                signatures: serde_json::from_value(row.signatures).unwrap_or_default(),
                governance_status: row.governance_status,
                linked_prs: serde_json::from_value(row.linked_prs).unwrap_or_default(),
                emergency_mode: row.emergency_mode,
                created_at: row.created_at,
                updated_at: row.updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_maintainers_for_layer(
        pool: &PgPool,
        layer: i32,
    ) -> Result<Vec<Maintainer>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, github_username, public_key, layer, active, last_updated
            FROM maintainers 
            WHERE layer = $1 AND active = true
            "#,
            layer
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| Maintainer {
                id: row.id,
                github_username: row.github_username,
                public_key: row.public_key,
                layer: row.layer,
                active: row.active,
                last_updated: row.last_updated,
            })
            .collect())
    }

    pub async fn get_emergency_keyholders(
        pool: &PgPool,
    ) -> Result<Vec<EmergencyKeyholder>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, github_username, public_key, active, last_updated
            FROM emergency_keyholders 
            WHERE active = true
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| EmergencyKeyholder {
                id: row.id,
                github_username: row.github_username,
                public_key: row.public_key,
                active: row.active,
                last_updated: row.last_updated,
            })
            .collect())
    }

    pub async fn get_governance_events(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<GovernanceEvent>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, event_type, repo_name, pr_number, maintainer, details, timestamp
            FROM governance_events 
            ORDER BY timestamp DESC 
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| GovernanceEvent {
                id: row.id,
                event_type: row.event_type,
                repo_name: row.repo_name,
                pr_number: row.pr_number,
                maintainer: row.maintainer,
                details: row.details,
                timestamp: row.timestamp,
            })
            .collect())
    }
}




