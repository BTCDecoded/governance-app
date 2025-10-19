pub mod models;
pub mod queries;
pub mod schema;

use sqlx::{PgPool, Row};
use std::collections::HashMap;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Database { pool })
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        // Run initial schema migration
        sqlx::query(include_str!("../migrations/001_initial_schema.sql"))
            .execute(&self.pool)
            .await?;
        
        // Run emergency mode migration
        sqlx::query(include_str!("../migrations/002_emergency_mode.sql"))
            .execute(&self.pool)
            .await?;
        
        // Run audit log migration
        sqlx::query(include_str!("../migrations/003_audit_log.sql"))
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }

    pub async fn create_pull_request(
        &self,
        repo_name: &str,
        pr_number: i32,
        head_sha: &str,
        layer: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO pull_requests (repo_name, pr_number, opened_at, layer, head_sha)
            VALUES ($1, $2, NOW(), $3, $4)
            ON CONFLICT (repo_name, pr_number) DO UPDATE SET
                head_sha = EXCLUDED.head_sha,
                updated_at = NOW()
            "#,
            repo_name,
            pr_number,
            layer,
            head_sha
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn update_review_status(
        &self,
        repo_name: &str,
        pr_number: i32,
        reviewer: &str,
        state: &str,
    ) -> Result<(), sqlx::Error> {
        // This would update review status in the database
        // Implementation depends on specific review tracking requirements
        Ok(())
    }

    pub async fn add_signature(
        &self,
        repo_name: &str,
        pr_number: i32,
        signer: &str,
        signature: &str,
    ) -> Result<(), sqlx::Error> {
        // Add signature to the pull request
        // This would involve updating the signatures JSONB field
        Ok(())
    }

    pub async fn log_governance_event(
        &self,
        event_type: &str,
        repo_name: Option<&str>,
        pr_number: Option<i32>,
        maintainer: Option<&str>,
        details: &serde_json::Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO governance_events (event_type, repo_name, pr_number, maintainer, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            event_type,
            repo_name,
            pr_number,
            maintainer,
            details
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}




