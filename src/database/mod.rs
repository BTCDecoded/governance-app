pub mod models;
pub mod queries;
pub mod schema;

use crate::error::GovernanceError;
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::str::FromStr;

#[derive(Clone)]
pub struct DatabaseBackend {
    pool: SqlitePool,
}

#[derive(Clone)]
pub struct Database {
    backend: DatabaseBackend,
    database_url: String,
}

/// Database connection pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub size: u32,
    pub idle: usize,
    pub is_closed: bool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, GovernanceError> {
        if database_url.starts_with("sqlite:") {
            let pool = SqlitePool::connect(database_url)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            Ok(Self {
                backend: DatabaseBackend { pool },
                database_url: database_url.to_string(),
            })
        } else {
            Err(GovernanceError::DatabaseError(
                "Unsupported database URL format. Use 'sqlite://' or 'postgresql://'".to_string(),
            ))
        }
    }

    /// Create an in-memory SQLite database for testing
    pub async fn new_in_memory() -> Result<Self, GovernanceError> {
        // Use a unique in-memory database for each instance to avoid migration conflicts
        // SQLite in-memory databases are isolated per connection, so each call gets a fresh DB
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let db = Self {
            backend: DatabaseBackend { pool },
            database_url: "sqlite::memory:".to_string(),
        };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create a new production database with optimized settings
    pub async fn new_production(database_url: &str) -> Result<Self, GovernanceError> {
        if database_url.starts_with("sqlite:") {
            let options = SqliteConnectOptions::from_str(database_url)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .locking_mode(sqlx::sqlite::SqliteLockingMode::Normal)
                .foreign_keys(true)
                .create_if_missing(true);

            let pool = SqlitePoolOptions::new()
                .max_connections(10)
                .min_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(30))
                .idle_timeout(std::time::Duration::from_secs(600))
                .max_lifetime(std::time::Duration::from_secs(1800))
                .connect_with(options)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

            let db = Database {
                backend: DatabaseBackend { pool },
                database_url: database_url.to_string(),
            };
            db.run_migrations().await?;
            Ok(db)
        } else {
            Err(GovernanceError::DatabaseError(
                "Unsupported database URL format for production. Use 'sqlite://' or 'postgresql://'".to_string()
            ))
        }
    }

    pub async fn run_migrations(&self) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        let result = sqlx::migrate!("./migrations").run(pool).await;

        match result {
            Ok(_) => {
                // Verify migrations ran by checking if key tables exist
                let tables_check = sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('pull_requests', 'tier_overrides', 'build_runs')"
                        )
                        .fetch_one(pool)
                        .await;

                if let Ok(count) = tables_check {
                    if count < 3 {
                        return Err(GovernanceError::DatabaseError(format!(
                            "Migrations may have failed: expected at least 3 tables, found {count}"
                        )));
                    }
                }
                Ok(())
            }
            Err(e) => {
                // Only ignore UNIQUE constraint errors on the _sqlx_migrations table itself
                // This happens when migrations run in parallel during tests
                let err_str = e.to_string();
                if err_str.contains("_sqlx_migrations") && err_str.contains("UNIQUE constraint") {
                    // Migration table entry already exists, migrations are already applied
                    Ok(())
                } else {
                    Err(GovernanceError::DatabaseError(format!(
                        "Migration failed: {err_str}"
                    )))
                }
            }
        }
    }

    pub fn get_sqlite_pool(&self) -> Option<&SqlitePool> {
        Some(&self.backend.pool)
    }

    pub fn is_sqlite(&self) -> bool {
        true
    }

    /// Reconnect to database using stored database URL
    /// Useful when connection pool is closed or unhealthy
    /// Returns a new Database instance with fresh connection pool
    pub async fn reconnect(&self) -> Result<Self, GovernanceError> {
        Self::new(&self.database_url).await
    }

    /// Check database connection health
    /// Returns true if connection is healthy, false otherwise
    pub async fn check_health(&self) -> Result<bool, GovernanceError> {
        sqlx::query("SELECT 1")
            .execute(&self.backend.pool)
            .await
            .map(|_| true)
            .map_err(|e| GovernanceError::DatabaseError(format!("Health check failed: {e}")))
    }

    /// Get database connection pool statistics
    pub async fn get_pool_stats(&self) -> Result<PoolStats, GovernanceError> {
        let pool = &self.backend.pool;
        Ok(PoolStats {
            size: pool.size(),
            idle: pool.num_idle(),
            is_closed: pool.is_closed(),
        })
    }

    pub async fn create_pull_request(
        &self,
        repo_name: &str,
        pr_number: i32,
        head_sha: &str,
        layer: i32,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query(
            r#"
            INSERT INTO pull_requests (repo_name, pr_number, opened_at, layer, head_sha)
            VALUES (?, ?, CURRENT_TIMESTAMP, ?, ?)
            ON CONFLICT (repo_name, pr_number) DO UPDATE SET
                head_sha = EXCLUDED.head_sha,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(repo_name)
        .bind(pr_number)
        .bind(layer)
        .bind(head_sha)
        .execute(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn update_review_status(
        &self,
        repo_name: &str,
        pr_number: i32,
        reviewer: &str,
        state: &str,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query!(
            r#"
            INSERT INTO reviews (repo_name, pr_number, reviewer, state, submitted_at, updated_at)
            VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT(repo_name, pr_number, reviewer) DO UPDATE SET
                state = EXCLUDED.state,
                updated_at = CURRENT_TIMESTAMP
            "#,
            repo_name,
            pr_number,
            reviewer,
            state
        )
        .execute(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Check if maintainer has reviewed PR
    pub async fn has_maintainer_reviewed(
        &self,
        repo_name: &str,
        pr_number: i32,
        maintainer: &str,
    ) -> Result<bool, GovernanceError> {
        let pool = &self.backend.pool;
        let exists: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as count
            FROM reviews
            WHERE repo_name = ? AND pr_number = ? AND reviewer = ? 
            AND state IN ('approved', 'changes_requested', 'commented')
            "#,
            repo_name,
            pr_number,
            maintainer
        )
        .fetch_one(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(exists > 0)
    }

    /// Store review with comment
    pub async fn store_review(
        &self,
        repo_name: &str,
        pr_number: i32,
        reviewer: &str,
        state: &str,
        review_comment: Option<&str>,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query!(
            r#"
            INSERT INTO reviews (repo_name, pr_number, reviewer, state, review_comment, submitted_at, updated_at)
            VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT(repo_name, pr_number, reviewer) DO UPDATE SET
                state = EXCLUDED.state,
                review_comment = EXCLUDED.review_comment,
                updated_at = CURRENT_TIMESTAMP
            "#,
            repo_name,
            pr_number,
            reviewer,
            state,
            review_comment
        )
        .execute(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn add_signature(
        &self,
        repo_name: &str,
        pr_number: i32,
        signer: &str,
        signature: &str,
        reasoning: Option<&str>,
    ) -> Result<(), GovernanceError> {
        use crate::database::models::Signature;
        use chrono::Utc;
        use serde_json::Value;

        // Create new signature with reasoning
        let new_signature = Signature {
            signer: signer.to_string(),
            signature: signature.to_string(),
            timestamp: Utc::now(),
            reasoning: reasoning.map(|s| s.to_string()),
        };

        let pool = &self.backend.pool;
        // Get current signatures
        let signatures_json: Option<String> = sqlx::query_scalar(
            "SELECT signatures FROM pull_requests WHERE repo_name = ? AND pr_number = ?",
        )
        .bind(repo_name)
        .bind(pr_number)
        .fetch_optional(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        // Parse existing signatures or create empty array
        let mut signatures: Vec<Value> = if let Some(json_str) = signatures_json {
            serde_json::from_str(&json_str).map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to parse signatures JSON: {e}"))
            })?
        } else {
            vec![]
        };

        // Add new signature
        signatures.push(serde_json::to_value(&new_signature).map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to serialize signature: {e}"))
        })?);

        // Update signatures in database
        let updated_json = serde_json::to_string(&signatures).map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to serialize signatures: {e}"))
        })?;

        sqlx::query(
                    "UPDATE pull_requests SET signatures = ?, updated_at = CURRENT_TIMESTAMP WHERE repo_name = ? AND pr_number = ?"
                )
                .bind(&updated_json)
                .bind(repo_name)
                .bind(pr_number)
                .execute(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn log_governance_event(
        &self,
        event_type: &str,
        repo_name: Option<&str>,
        pr_number: Option<i32>,
        maintainer: Option<&str>,
        details: &serde_json::Value,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query(
            r#"
            INSERT INTO governance_events (event_type, repo_name, pr_number, maintainer, details)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(event_type)
        .bind(repo_name)
        .bind(pr_number)
        .bind(maintainer)
        .bind(serde_json::to_string(details).map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to serialize event details: {e}"))
        })?)
        .execute(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    pub async fn get_pull_request(
        &self,
        repo_name: &str,
        pr_number: i32,
    ) -> Result<Option<crate::database::models::PullRequest>, GovernanceError> {
        let pool = &self.backend.pool;
        use crate::database::queries::Queries;
        Queries::get_pull_request(pool, repo_name, pr_number)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))
    }

    pub async fn get_governance_events(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::database::models::GovernanceEvent>, GovernanceError> {
        let pool = &self.backend.pool;
        let rows = sqlx::query(
            r#"
                    SELECT 
                        id,
                        event_type,
                        repo_name,
                        pr_number,
                        maintainer,
                        details,
                        timestamp
                    FROM governance_events
                    ORDER BY timestamp DESC
                    LIMIT ?
                    "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let mut events = Vec::new();
        for row in rows {
            let id: i32 = row
                .try_get(0)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let event_type: String = row
                .try_get(1)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let repo_name: Option<String> = row
                .try_get(2)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let pr_number: Option<i32> = row
                .try_get(3)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let maintainer: Option<String> = row
                .try_get(4)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let details_str: String = row
                .try_get(5)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let timestamp: chrono::DateTime<chrono::Utc> = row
                .try_get(6)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

            let details: serde_json::Value = serde_json::from_str(&details_str).map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to parse event details JSON: {e}"))
            })?;

            events.push(crate::database::models::GovernanceEvent {
                id,
                event_type,
                repo_name,
                pr_number,
                maintainer,
                details,
                timestamp,
            });
        }
        Ok(events)
    }

    /// Get the last merged PR information
    pub async fn get_last_merged_pr(
        &self,
    ) -> Result<Option<(Option<i32>, Option<chrono::DateTime<chrono::Utc>>)>, GovernanceError> {
        let pool = &self.backend.pool;
        let row = sqlx::query(
            r#"
                    SELECT pr_number, timestamp
                    FROM governance_events
                    WHERE event_type IN ('merge', 'merged', 'pr_merged')
                    ORDER BY timestamp DESC
                    LIMIT 1
                    "#,
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        if let Some(row) = row {
            let pr_number: Option<i32> = row
                .try_get(0)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let timestamp: Option<chrono::DateTime<chrono::Utc>> = row
                .try_get(1)
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            Ok(Some((pr_number, timestamp)))
        } else {
            Ok(None)
        }
    }

    /// Count merges today
    pub async fn count_merges_today(&self) -> Result<u64, GovernanceError> {
        let pool = &self.backend.pool;
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM governance_events
            WHERE event_type IN ('merge', 'merged', 'pr_merged')
            AND date(timestamp) = date('now')
            "#,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(count as u64)
    }

    /// Add or update a tier override for a PR
    pub async fn set_tier_override(
        &self,
        repo_name: &str,
        pr_number: i32,
        override_tier: u32,
        justification: &str,
        overridden_by: &str,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        // SQLite doesn't support ON CONFLICT with named columns in older versions
        // Use REPLACE INTO instead (works with UNIQUE constraint)
        sqlx::query(
                    r#"
                    INSERT OR REPLACE INTO tier_overrides (repo_name, pr_number, override_tier, justification, overridden_by, created_at)
                    VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                    "#
                )
                .bind(repo_name)
                .bind(pr_number)
                .bind(override_tier as i32)
                .bind(justification)
                .bind(overridden_by)
                .execute(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Get tier override for a PR if it exists
    pub async fn get_tier_override(
        &self,
        repo_name: &str,
        pr_number: i32,
    ) -> Result<Option<crate::database::models::TierOverride>, GovernanceError> {
        use sqlx::Row;
        let pool = &self.backend.pool;
        let row = sqlx::query(
                    r#"
                    SELECT id, repo_name, pr_number, override_tier, justification, overridden_by, created_at
                    FROM tier_overrides
                    WHERE repo_name = ? AND pr_number = ?
                    "#
                )
                .bind(repo_name)
                .bind(pr_number)
                .fetch_optional(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        if let Some(row) = row {
            Ok(Some(crate::database::models::TierOverride {
                id: row
                    .try_get(0)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
                repo_name: row
                    .try_get(1)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
                pr_number: row
                    .try_get(2)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
                override_tier: row
                    .try_get::<i32, _>(3)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?
                    as u32,
                justification: row
                    .try_get(4)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
                overridden_by: row
                    .try_get(5)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
                created_at: row
                    .try_get(6)
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_maintainer_by_username(
        &self,
        username: &str,
    ) -> Result<Option<crate::database::models::Maintainer>, GovernanceError> {
        let pool = &self.backend.pool;
        let maintainer = sqlx::query_as::<_, crate::database::models::Maintainer>(
            "SELECT id, github_username, public_key, layer, active, last_updated FROM maintainers WHERE github_username = ? AND active = true"
        )
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(maintainer)
    }

    pub async fn get_maintainers_for_layer(
        &self,
        layer: i32,
    ) -> Result<Vec<crate::database::models::Maintainer>, GovernanceError> {
        let pool = &self.backend.pool;
        use crate::database::queries::Queries;
        Queries::get_maintainers_for_layer(pool, layer)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))
    }

    pub async fn get_emergency_keyholders(
        &self,
    ) -> Result<Vec<crate::database::models::EmergencyKeyholder>, GovernanceError> {
        let pool = &self.backend.pool;
        use crate::database::queries::Queries;
        Queries::get_emergency_keyholders(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))
    }

    /// Get the database pool for testing purposes (SQLite only)
    pub fn pool(&self) -> Option<&SqlitePool> {
        self.get_sqlite_pool()
    }

    /// Perform database health check
    pub async fn health_check(&self) -> Result<DatabaseHealth, GovernanceError> {
        let pool = &self.backend.pool;
        let connection_count = pool.size();
        let idle_connections = pool.num_idle() as u32;
        let active_connections = connection_count - idle_connections;

        let integrity_result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let page_count = sqlx::query_scalar::<_, i64>("PRAGMA page_count")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        let page_size = sqlx::query_scalar::<_, i64>("PRAGMA page_size")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        let db_size = page_count * page_size;

        Ok(DatabaseHealth {
            connection_count,
            idle_connections,
            active_connections,
            integrity_ok: integrity_result == "ok",
            journal_mode: journal_mode.clone(),
            database_size_bytes: db_size,
            wal_mode_active: journal_mode == "wal",
        })
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> Result<PerformanceStats, GovernanceError> {
        let pool = &self.backend.pool;
        let cache_size = sqlx::query_scalar::<_, i64>("PRAGMA cache_size")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let wal_checkpoint_threshold = sqlx::query_scalar::<_, i64>("PRAGMA wal_autocheckpoint")
            .fetch_one(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        let compile_options = sqlx::query_scalar::<_, String>("PRAGMA compile_options")
            .fetch_all(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        Ok(PerformanceStats {
            cache_size,
            wal_checkpoint_threshold,
            slow_queries_count: compile_options.len() as i64,
        })
    }

    /// Optimize database performance
    pub async fn optimize_database(&self) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query("VACUUM")
            .execute(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        sqlx::query("ANALYZE")
            .execute(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Checkpoint WAL file to main database (SQLite only)
    pub async fn checkpoint_wal(&self) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    /// Create or update build run state
    pub async fn upsert_build_run(
        &self,
        release_version: &str,
        repo_name: &str,
        workflow_run_id: Option<u64>,
        status: &str,
    ) -> Result<i64, GovernanceError> {
        let pool = &self.backend.pool;
        let build_id = sqlx::query_scalar::<_, i64>(
                    r#"
                    INSERT INTO build_runs (release_version, repo_name, workflow_run_id, status, started_at, updated_at)
                    VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    ON CONFLICT(release_version, repo_name) DO UPDATE SET
                        workflow_run_id = COALESCE(EXCLUDED.workflow_run_id, workflow_run_id),
                        status = EXCLUDED.status,
                        updated_at = CURRENT_TIMESTAMP
                    RETURNING id
                    "#
                )
                .bind(release_version)
                .bind(repo_name)
                .bind(workflow_run_id.map(|id| id as i64))
                .bind(status)
                .fetch_one(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        Ok(build_id)
    }

    /// Update build run status with state transition tracking
    pub async fn update_build_status(
        &self,
        release_version: &str,
        repo_name: &str,
        new_status: &str,
        error_message: Option<&str>,
    ) -> Result<(), GovernanceError> {
        let pool = &self.backend.pool;
        // Get current status
        let current_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM build_runs WHERE release_version = ? AND repo_name = ?",
        )
        .bind(release_version)
        .bind(repo_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        // Update build run
        let completed_at = if new_status == "success"
            || new_status == "failure"
            || new_status == "cancelled"
            || new_status == "timed_out"
        {
            Some("CURRENT_TIMESTAMP")
        } else {
            None
        };

        if let Some(completed) = completed_at {
            sqlx::query(
                        &format!(
                            r#"
                            UPDATE build_runs 
                            SET status = ?, error_message = ?, completed_at = {completed}, updated_at = CURRENT_TIMESTAMP
                            WHERE release_version = ? AND repo_name = ?
                            "#
                        )
                    )
                    .bind(new_status)
                    .bind(error_message)
                    .bind(release_version)
                    .bind(repo_name)
                    .execute(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        } else {
            sqlx::query(
                r#"
                        UPDATE build_runs 
                        SET status = ?, error_message = ?, updated_at = CURRENT_TIMESTAMP
                        WHERE release_version = ? AND repo_name = ?
                        "#,
            )
            .bind(new_status)
            .bind(error_message)
            .bind(release_version)
            .bind(repo_name)
            .execute(pool)
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        }

        // Log state transition
        if let Some(from_status) = current_status {
            if from_status != new_status {
                let build_id: i64 = sqlx::query_scalar(
                    "SELECT id FROM build_runs WHERE release_version = ? AND repo_name = ?",
                )
                .bind(release_version)
                .bind(repo_name)
                .fetch_one(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                sqlx::query(
                            r#"
                            INSERT INTO build_state_transitions (build_run_id, from_status, to_status, reason)
                            VALUES (?, ?, ?, ?)
                            "#
                        )
                        .bind(build_id)
                        .bind(&from_status)
                        .bind(new_status)
                        .bind(error_message)
                        .execute(pool)
                        .await
                        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Get all build runs for a release
    pub async fn get_build_runs_for_release(
        &self,
        release_version: &str,
    ) -> Result<Vec<(String, String)>, GovernanceError> {
        let pool = &self.backend.pool;
        let rows =
            sqlx::query("SELECT repo_name, status FROM build_runs WHERE release_version = ?")
                .bind(release_version)
                .fetch_all(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|row| (row.get::<String, _>(0), row.get::<String, _>(1)))
            .collect())
    }

    /// Get all build runs with workflow_run_ids for a release
    pub async fn get_build_runs_with_ids_for_release(
        &self,
        release_version: &str,
    ) -> Result<Vec<(String, Option<u64>, String)>, GovernanceError> {
        let pool = &self.backend.pool;
        let rows = sqlx::query(
            "SELECT repo_name, workflow_run_id, status FROM build_runs WHERE release_version = ?",
        )
        .bind(release_version)
        .fetch_all(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|row| {
                (
                    row.get::<String, _>(0),
                    row.get::<Option<i64>, _>(1).map(|id| id as u64),
                    row.get::<String, _>(2),
                )
            })
            .collect())
    }

    /// Check if all builds for a release are complete
    pub async fn are_all_builds_complete(
        &self,
        release_version: &str,
    ) -> Result<bool, GovernanceError> {
        let pool = &self.backend.pool;
        let incomplete_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM build_runs 
            WHERE release_version = ? 
            AND status NOT IN ('success', 'failure', 'cancelled', 'timed_out')
            "#,
        )
        .bind(release_version)
        .fetch_one(pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

        Ok(incomplete_count == 0)
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub connection_count: u32,
    pub idle_connections: u32,
    pub active_connections: u32,
    pub integrity_ok: bool,
    pub journal_mode: String,
    pub database_size_bytes: i64,
    pub wal_mode_active: bool,
}

#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub cache_size: i64,
    pub wal_checkpoint_threshold: i64,
    pub slow_queries_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_database_new_in_memory() {
        let db = Database::new_in_memory().await.unwrap();
        assert!(db.is_sqlite());
        assert!(db.get_sqlite_pool().is_some());
    }

    #[tokio::test]
    async fn test_database_new_invalid_url() {
        let result = Database::new("invalid://url").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_health() {
        let db = Database::new_in_memory().await.unwrap();
        let health = db.check_health().await.unwrap();
        assert!(health, "Database should be healthy");
    }

    #[tokio::test]
    async fn test_get_pool_stats() {
        let db = Database::new_in_memory().await.unwrap();
        let stats = db.get_pool_stats().await.unwrap();
        assert!(stats.size > 0, "Pool should have connections");
        assert!(!stats.is_closed, "Pool should not be closed");
    }

    #[tokio::test]
    async fn test_create_pull_request() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.create_pull_request("test/repo", 1, "abc123", 2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_pull_request_duplicate() {
        let db = Database::new_in_memory().await.unwrap();
        db.create_pull_request("test/repo", 1, "abc123", 2)
            .await
            .unwrap();
        // Should update existing PR
        let result = db.create_pull_request("test/repo", 1, "def456", 2).await;
        assert!(result.is_ok());

        let pr = db.get_pull_request("test/repo", 1).await.unwrap().unwrap();
        assert_eq!(pr.head_sha, "def456", "Head SHA should be updated");
    }

    #[tokio::test]
    async fn test_get_pull_request_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.get_pull_request("test/repo", 999).await.unwrap();
        assert!(result.is_none(), "PR should not exist");
    }

    #[tokio::test]
    async fn test_add_signature() {
        let db = Database::new_in_memory().await.unwrap();
        db.create_pull_request("test/repo", 1, "abc123", 2)
            .await
            .unwrap();

        let result = db
            .add_signature("test/repo", 1, "alice", "sig123", Some("reason"))
            .await;
        assert!(result.is_ok());

        let pr = db.get_pull_request("test/repo", 1).await.unwrap().unwrap();
        assert_eq!(pr.signatures.len(), 1);
        assert_eq!(pr.signatures[0].signer, "alice");
        assert_eq!(pr.signatures[0].signature, "sig123");
    }

    #[tokio::test]
    async fn test_add_multiple_signatures() {
        let db = Database::new_in_memory().await.unwrap();
        db.create_pull_request("test/repo", 1, "abc123", 2)
            .await
            .unwrap();

        db.add_signature("test/repo", 1, "alice", "sig1", None)
            .await
            .unwrap();
        db.add_signature("test/repo", 1, "bob", "sig2", None)
            .await
            .unwrap();

        let pr = db.get_pull_request("test/repo", 1).await.unwrap().unwrap();
        assert_eq!(pr.signatures.len(), 2);
    }

    #[tokio::test]
    async fn test_log_governance_event() {
        let db = Database::new_in_memory().await.unwrap();
        let details = json!({"test": "value"});

        let result = db
            .log_governance_event(
                "test_event",
                Some("test/repo"),
                Some(1),
                Some("alice"),
                &details,
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_governance_events() {
        let db = Database::new_in_memory().await.unwrap();
        let details = json!({"test": "value"});

        db.log_governance_event("event1", None, None, None, &details)
            .await
            .unwrap();
        db.log_governance_event("event2", None, None, None, &details)
            .await
            .unwrap();

        let events = db.get_governance_events(10).await.unwrap();
        assert!(events.len() >= 2);
    }

    #[tokio::test]
    async fn test_get_governance_events_limit() {
        let db = Database::new_in_memory().await.unwrap();
        let details = json!({"test": "value"});

        for i in 0..5 {
            db.log_governance_event(&format!("event{}", i), None, None, None, &details)
                .await
                .unwrap();
        }

        let events = db.get_governance_events(3).await.unwrap();
        assert_eq!(events.len(), 3, "Should respect limit");
    }

    #[tokio::test]
    async fn test_get_last_merged_pr_none() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.get_last_merged_pr().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_last_merged_pr_exists() {
        let db = Database::new_in_memory().await.unwrap();
        let details = json!({"pr": 123});
        db.log_governance_event("merge", Some("test/repo"), Some(123), None, &details)
            .await
            .unwrap();

        let result = db.get_last_merged_pr().await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_count_merges_today() {
        let db = Database::new_in_memory().await.unwrap();
        let details = json!({});

        db.log_governance_event("merge", None, Some(1), None, &details)
            .await
            .unwrap();
        db.log_governance_event("merge", None, Some(2), None, &details)
            .await
            .unwrap();

        let count = db.count_merges_today().await.unwrap();
        assert_eq!(count, 2, "Should count exactly 2 merges from today");
    }

    #[tokio::test]
    async fn test_set_tier_override() {
        let db = Database::new_in_memory().await.unwrap();
        db.create_pull_request("test/repo", 1, "abc123", 2)
            .await
            .unwrap();

        let result = db
            .set_tier_override("test/repo", 1, 3, "Justification", "alice")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_tier_override() {
        let db = Database::new_in_memory().await.unwrap();
        db.create_pull_request("test/repo", 1, "abc123", 2)
            .await
            .unwrap();
        db.set_tier_override("test/repo", 1, 3, "Justification", "alice")
            .await
            .unwrap();

        let override_val = db.get_tier_override("test/repo", 1).await.unwrap();
        assert!(override_val.is_some());
        assert_eq!(override_val.unwrap().override_tier, 3);
    }

    #[tokio::test]
    async fn test_get_tier_override_not_found() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.get_tier_override("test/repo", 999).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_emergency_keyholders_empty() {
        let db = Database::new_in_memory().await.unwrap();
        let keyholders = db.get_emergency_keyholders().await.unwrap();
        assert_eq!(keyholders.len(), 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let db = Database::new_in_memory().await.unwrap();
        let health = db.health_check().await.unwrap();
        assert!(health.integrity_ok);
    }

    #[tokio::test]
    async fn test_optimize_database() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.optimize_database().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_checkpoint_wal() {
        let db = Database::new_in_memory().await.unwrap();
        let result = db.checkpoint_wal().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_upsert_build_run() {
        let db = Database::new_in_memory().await.unwrap();
        let build_id = db
            .upsert_build_run("v1.0.0", "test/repo", Some(123), "running")
            .await
            .unwrap();
        assert!(build_id > 0);
    }

    #[tokio::test]
    async fn test_upsert_build_run_update() {
        let db = Database::new_in_memory().await.unwrap();
        let _id1 = db
            .upsert_build_run("v1.0.0", "test/repo", Some(123), "running")
            .await
            .unwrap();
        let _id2 = db
            .upsert_build_run("v1.0.0", "test/repo", Some(456), "success")
            .await
            .unwrap();
        // Should update existing, not create new
        assert_eq!(_id1, _id2);
    }

    #[tokio::test]
    async fn test_update_build_run_status() {
        let db = Database::new_in_memory().await.unwrap();
        db.upsert_build_run("v1.0.0", "test/repo", Some(123), "running")
            .await
            .unwrap();

        let result = db
            .update_build_status("v1.0.0", "test/repo", "success", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_build_runs_for_release() {
        let db = Database::new_in_memory().await.unwrap();
        db.upsert_build_run("v1.0.0", "repo1", Some(1), "success")
            .await
            .unwrap();
        db.upsert_build_run("v1.0.0", "repo2", Some(2), "running")
            .await
            .unwrap();

        let builds = db.get_build_runs_for_release("v1.0.0").await.unwrap();
        assert_eq!(builds.len(), 2);
    }

    #[tokio::test]
    async fn test_are_all_builds_complete_true() {
        let db = Database::new_in_memory().await.unwrap();
        db.upsert_build_run("v1.0.0", "repo1", Some(1), "success")
            .await
            .unwrap();
        db.upsert_build_run("v1.0.0", "repo2", Some(2), "success")
            .await
            .unwrap();

        let complete = db.are_all_builds_complete("v1.0.0").await.unwrap();
        assert!(complete);
    }

    #[tokio::test]
    async fn test_are_all_builds_complete_false() {
        let db = Database::new_in_memory().await.unwrap();
        db.upsert_build_run("v1.0.0", "repo1", Some(1), "success")
            .await
            .unwrap();
        db.upsert_build_run("v1.0.0", "repo2", Some(2), "running")
            .await
            .unwrap();

        let complete = db.are_all_builds_complete("v1.0.0").await.unwrap();
        assert!(!complete);
    }

    #[tokio::test]
    async fn test_is_sqlite() {
        let db = Database::new_in_memory().await.unwrap();
        assert!(db.is_sqlite());
        assert!(db.get_sqlite_pool().is_some());
    }

    #[tokio::test]
    async fn test_get_sqlite_pool() {
        let db = Database::new_in_memory().await.unwrap();
        assert!(db.get_sqlite_pool().is_some());
        assert!(db.get_sqlite_pool().is_some());
    }
}
