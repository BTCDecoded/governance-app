pub mod models;
pub mod queries;
pub mod schema;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Database { pool })
    }

    pub async fn new_in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect("sqlite::memory:").await?;
        let db = Database { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn create_pull_request(
        &self,
        repo_name: &str,
        pr_number: i32,
        head_sha: &str,
        layer: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO pull_requests (repo_name, pr_number, opened_at, layer, head_sha)
            VALUES (?, ?, CURRENT_TIMESTAMP, ?, ?)
            ON CONFLICT (repo_name, pr_number) DO UPDATE SET
                head_sha = EXCLUDED.head_sha,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(repo_name)
        .bind(pr_number)
        .bind(layer)
        .bind(head_sha)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn update_review_status(
        &self,
        _repo_name: &str,
        _pr_number: i32,
        _reviewer: &str,
        _state: &str,
    ) -> Result<(), sqlx::Error> {
        // This would update review status in the database
        // Implementation depends on specific review tracking requirements
        Ok(())
    }

    pub async fn add_signature(
        &self,
        _repo_name: &str,
        _pr_number: i32,
        _signer: &str,
        _signature: &str,
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
        sqlx::query(
            r#"
            INSERT INTO governance_events (event_type, repo_name, pr_number, maintainer, details)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(event_type)
        .bind(repo_name)
        .bind(pr_number)
        .bind(maintainer)
        .bind(serde_json::to_string(details).unwrap_or_default())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    pub async fn get_pull_request(
        &self,
        _repo_name: &str,
        _pr_number: i32,
    ) -> Result<Option<crate::database::models::PullRequest>, sqlx::Error> {
        // This would retrieve a pull request from the database
        // For now, return None as a placeholder
        Ok(None)
    }

    pub async fn get_governance_events(
        &self,
        _limit: i64,
    ) -> Result<Vec<crate::database::models::GovernanceEvent>, sqlx::Error> {
        // This would retrieve governance events from the database
        // For now, return empty vector as a placeholder
        Ok(vec![])
    }
}




