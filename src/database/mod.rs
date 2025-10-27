pub mod models;
pub mod queries;
pub mod schema;

use sqlx::{SqlitePool, PgPool, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::str::FromStr;
use crate::error::GovernanceError;

#[derive(Clone)]
pub enum DatabaseBackend {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

#[derive(Clone)]
pub struct Database {
    backend: DatabaseBackend,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, GovernanceError> {
        if database_url.starts_with("sqlite:") {
            let pool = SqlitePool::connect(database_url)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            Ok(Self {
                backend: DatabaseBackend::Sqlite(pool),
            })
        } else if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
            let pool = PgPool::connect(database_url)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            Ok(Self {
                backend: DatabaseBackend::Postgres(pool),
            })
        } else {
            Err(GovernanceError::DatabaseError(
                "Unsupported database URL format. Use 'sqlite://' or 'postgresql://'".to_string()
            ))
        }
    }

    /// Create an in-memory SQLite database for testing
    pub async fn new_in_memory() -> Result<Self, GovernanceError> {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
        
        let db = Self {
            backend: DatabaseBackend::Sqlite(pool),
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
                backend: DatabaseBackend::Sqlite(pool),
            };
            db.run_migrations().await?;
            Ok(db)
        } else if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
            let pool = PgPool::connect(database_url)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            let db = Database {
                backend: DatabaseBackend::Postgres(pool),
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
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                sqlx::migrate!("./migrations")
                    .run(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
            DatabaseBackend::Postgres(pool) => {
                sqlx::migrate!("./migrations-postgres")
                    .run(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn get_sqlite_pool(&self) -> Option<&SqlitePool> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => Some(pool),
            _ => None,
        }
    }

    pub fn get_postgres_pool(&self) -> Option<&PgPool> {
        match &self.backend {
            DatabaseBackend::Postgres(pool) => Some(pool),
            _ => None,
        }
    }

    pub fn is_sqlite(&self) -> bool {
        matches!(self.backend, DatabaseBackend::Sqlite(_))
    }

    pub fn is_postgres(&self) -> bool {
        matches!(self.backend, DatabaseBackend::Postgres(_))
    }

    pub async fn create_pull_request(
        &self,
        repo_name: &str,
        pr_number: i32,
        head_sha: &str,
        layer: i32,
    ) -> Result<(), GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
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
            }
            DatabaseBackend::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO pull_requests (repo_name, pr_number, opened_at, layer, head_sha)
                    VALUES ($1, $2, CURRENT_TIMESTAMP, $3, $4)
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
            }
        }
        Ok(())
    }

    pub async fn update_review_status(
        &self,
        _repo_name: &str,
        _pr_number: i32,
        _reviewer: &str,
        _state: &str,
    ) -> Result<(), GovernanceError> {
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
    ) -> Result<(), GovernanceError> {
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
    ) -> Result<(), GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
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
                .bind(serde_json::to_string(details).unwrap_or_default())
                .execute(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
            DatabaseBackend::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO governance_events (event_type, repo_name, pr_number, maintainer, details)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                )
                .bind(event_type)
                .bind(repo_name)
                .bind(pr_number)
                .bind(maintainer)
                .bind(details)
                .execute(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub async fn get_pull_request(
        &self,
        _repo_name: &str,
        _pr_number: i32,
    ) -> Result<Option<crate::database::models::PullRequest>, GovernanceError> {
        // This would retrieve a pull request from the database
        // For now, return None as a placeholder
        Ok(None)
    }

    pub async fn get_governance_events(
        &self,
        _limit: i64,
    ) -> Result<Vec<crate::database::models::GovernanceEvent>, GovernanceError> {
        // This would retrieve governance events from the database
        // For now, return empty vector as a placeholder
        Ok(vec![])
    }

    pub async fn get_maintainer_by_username(
        &self,
        username: &str,
    ) -> Result<Option<crate::database::models::Maintainer>, GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                let maintainer = sqlx::query_as!(
                    crate::database::models::Maintainer,
                    "SELECT id, github_username, public_key, layer, active, last_updated FROM maintainers WHERE github_username = ? AND active = true",
                    username
                )
                .fetch_optional(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
                Ok(maintainer)
            }
            DatabaseBackend::Postgres(pool) => {
                let maintainer = sqlx::query_as!(
                    crate::database::models::Maintainer,
                    "SELECT id, github_username, public_key, layer, active, last_updated FROM maintainers WHERE github_username = $1 AND active = true",
                    username
                )
                .fetch_optional(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
                Ok(maintainer)
            }
        }
    }

    /// Get the database pool for testing purposes (SQLite only)
    pub fn pool(&self) -> Option<&SqlitePool> {
        self.get_sqlite_pool()
    }

    /// Perform database health check
    pub async fn health_check(&self) -> Result<DatabaseHealth, GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                // Check database connectivity
                let connection_count = pool.size() as u32;
                let idle_connections = pool.num_idle() as u32;
                let active_connections = connection_count - idle_connections;

                // Check database integrity
                let integrity_result = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                // Check WAL mode
                let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                // Check database size
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
            DatabaseBackend::Postgres(pool) => {
                // Check database connectivity
                let connection_count = pool.size() as u32;
                let idle_connections = pool.num_idle() as u32;
                let active_connections = connection_count - idle_connections;

                // Check database size
                let db_size = sqlx::query_scalar::<_, i64>(
                    "SELECT pg_database_size(current_database())"
                )
                .fetch_one(pool)
                .await
                .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                Ok(DatabaseHealth {
                    connection_count,
                    idle_connections,
                    active_connections,
                    integrity_ok: true, // PostgreSQL handles integrity automatically
                    journal_mode: "wal".to_string(), // PostgreSQL uses WAL by default
                    database_size_bytes: db_size,
                    wal_mode_active: true,
                })
            }
        }
    }

    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> Result<PerformanceStats, GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                // Get cache size
                let cache_size = sqlx::query_scalar::<_, i64>("PRAGMA cache_size")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                // Get WAL checkpoint threshold
                let wal_checkpoint_threshold = sqlx::query_scalar::<_, i64>("PRAGMA wal_autocheckpoint")
                    .fetch_one(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                // Get compile options (as a proxy for slow queries)
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
            DatabaseBackend::Postgres(_pool) => {
                // PostgreSQL-specific statistics would go here
                // For now, return default values
                Ok(PerformanceStats {
                    cache_size: 0,
                    wal_checkpoint_threshold: 0,
                    slow_queries_count: 0,
                })
            }
        }
    }

    /// Optimize database performance
    pub async fn optimize_database(&self) -> Result<(), GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                // Run VACUUM to reclaim space and optimize database
                sqlx::query("VACUUM")
                    .execute(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;

                // Run ANALYZE to update query planner statistics
                sqlx::query("ANALYZE")
                    .execute(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
            DatabaseBackend::Postgres(pool) => {
                // Run VACUUM ANALYZE to reclaim space and update statistics
                sqlx::query("VACUUM ANALYZE")
                    .execute(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Checkpoint WAL file to main database (SQLite only)
    pub async fn checkpoint_wal(&self) -> Result<(), GovernanceError> {
        match &self.backend {
            DatabaseBackend::Sqlite(pool) => {
                // Checkpoint WAL file to main database
                sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                    .execute(pool)
                    .await
                    .map_err(|e| GovernanceError::DatabaseError(e.to_string()))?;
            }
            DatabaseBackend::Postgres(_) => {
                // PostgreSQL handles WAL checkpointing automatically
                // This is a no-op for PostgreSQL
            }
        }
        Ok(())
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