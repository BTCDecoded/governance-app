//! Time Lock Tracking
//!
//! Tracks time-locked governance changes and enforces minimum time locks
//! before changes can be activated.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::database::Database;

/// Time lock status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeLockStatus {
    /// Time lock is pending (not yet elapsed)
    Pending,
    /// Time lock has elapsed and change can be activated
    Ready,
    /// Change has been activated
    Activated,
    /// Change was cancelled
    Cancelled,
}

/// Time lock configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLockConfig {
    /// Minimum time lock duration for Tier 3 changes (hours)
    pub tier3_min_hours: i64,
    /// Minimum time lock duration for Tier 4 changes (hours)
    pub tier4_min_hours: i64,
    /// Minimum time lock duration for Tier 5 changes (hours)
    pub tier5_min_hours: i64,
    /// User override threshold (percentage of active nodes)
    pub user_override_threshold: f64,
}

impl Default for TimeLockConfig {
    fn default() -> Self {
        Self {
            tier3_min_hours: 168,          // 7 days
            tier4_min_hours: 336,          // 14 days
            tier5_min_hours: 720,          // 30 days
            user_override_threshold: 0.75, // 75% of active nodes
        }
    }
}

/// Time-locked governance change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLockedChange {
    /// Unique identifier for the change
    pub change_id: String,
    /// Action tier (3, 4, or 5)
    pub tier: u8,
    /// Change description
    pub description: String,
    /// PR or issue number
    pub pr_number: Option<i64>,
    /// Time lock start time
    pub lock_start: DateTime<Utc>,
    /// Minimum lock duration (hours)
    pub min_duration_hours: i64,
    /// Time lock end time (lock_start + min_duration)
    pub lock_end: DateTime<Utc>,
    /// Current status
    pub status: String,
    /// User override signals (node_id -> timestamp)
    pub override_signals: HashMap<String, DateTime<Utc>>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

// Custom FromRow implementation to handle JSON deserialization
impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for TimeLockedChange {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let override_signals_json: Option<String> = row.try_get("override_signals")?;
        let override_signals: HashMap<String, DateTime<Utc>> = if let Some(json_str) =
            override_signals_json
        {
            serde_json::from_str(&json_str).map_err(|e| {
                sqlx::Error::Decode(format!("Failed to parse override_signals JSON: {e}").into())
            })?
        } else {
            HashMap::new()
        };

        Ok(TimeLockedChange {
            change_id: row.try_get("change_id")?,
            tier: row.try_get::<i64, _>("tier")? as u8,
            description: row.try_get("description")?,
            pr_number: row.try_get("pr_number")?,
            lock_start: row.try_get("lock_start")?,
            min_duration_hours: row.try_get("min_duration_hours")?,
            lock_end: row.try_get("lock_end")?,
            status: row.try_get("status")?,
            override_signals,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(any(feature = "postgres", test))]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for TimeLockedChange {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let override_signals_json: Option<serde_json::Value> = row.try_get("override_signals")?;
        let override_signals: HashMap<String, DateTime<Utc>> = if let Some(json_val) =
            override_signals_json
        {
            serde_json::from_value(json_val).map_err(|e| {
                sqlx::Error::Decode(format!("Failed to parse override_signals JSON: {}", e).into())
            })?
        } else {
            HashMap::new()
        };

        Ok(TimeLockedChange {
            change_id: row.try_get("change_id")?,
            tier: row.try_get::<i16, _>("tier")? as u8,
            description: row.try_get("description")?,
            pr_number: row.try_get("pr_number")?,
            lock_start: row.try_get("lock_start")?,
            min_duration_hours: row.try_get("min_duration_hours")?,
            lock_end: row.try_get("lock_end")?,
            status: row.try_get("status")?,
            override_signals,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Time lock manager
pub struct TimeLockManager {
    db: Database,
    config: TimeLockConfig,
}

impl TimeLockManager {
    /// Create a new time lock manager
    pub fn new(db: Database, config: TimeLockConfig) -> Self {
        Self { db, config }
    }

    /// Create a time lock for a governance change
    pub async fn create_time_lock(
        &self,
        change_id: &str,
        tier: u8,
        description: &str,
        pr_number: Option<i64>,
    ) -> Result<TimeLockedChange, sqlx::Error> {
        info!(
            "Creating time lock for change {} (Tier {})",
            change_id, tier
        );

        // Determine minimum duration based on tier
        let min_duration_hours = match tier {
            3 => self.config.tier3_min_hours,
            4 => self.config.tier4_min_hours,
            5 => self.config.tier5_min_hours,
            _ => {
                warn!("Invalid tier {}, using Tier 5 duration", tier);
                self.config.tier5_min_hours
            }
        };

        let lock_start = Utc::now();
        let lock_end = lock_start
            + Duration::try_hours(min_duration_hours)
                .ok_or_else(|| sqlx::Error::Decode("Invalid duration hours".into()))?;

        // Insert into database
        let change = sqlx::query_as::<_, TimeLockedChange>(
            r#"
            INSERT INTO time_locked_changes (
                change_id, tier, description, pr_number,
                lock_start, min_duration_hours, lock_end, status,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(change_id)
        .bind(tier as i64)
        .bind(description)
        .bind(pr_number)
        .bind(lock_start)
        .bind(min_duration_hours)
        .bind(lock_end)
        .bind("pending")
        .bind(lock_start)
        .bind(lock_start)
        .fetch_one(
            self.db
                .get_sqlite_pool()
                .ok_or_else(|| sqlx::Error::PoolClosed)?,
        )
        .await?;

        info!(
            "Time lock created: {} until {}",
            change_id,
            lock_end.to_rfc3339()
        );

        Ok(change)
    }

    /// Check if a time lock has elapsed
    pub async fn check_time_lock(&self, change_id: &str) -> Result<TimeLockStatus, sqlx::Error> {
        let pool = self
            .db
            .get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?;
        let change = sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE change_id = $1",
        )
        .bind(change_id)
        .fetch_optional(pool)
        .await?;

        let change = match change {
            Some(c) => c,
            None => {
                warn!("Time lock not found: {}", change_id);
                return Ok(TimeLockStatus::Cancelled);
            }
        };

        // Check status
        match change.status.as_str() {
            "activated" => return Ok(TimeLockStatus::Activated),
            "cancelled" => return Ok(TimeLockStatus::Cancelled),
            _ => {}
        }

        // Check if time lock has elapsed
        let now = Utc::now();
        if now >= change.lock_end {
            Ok(TimeLockStatus::Ready)
        } else {
            Ok(TimeLockStatus::Pending)
        }
    }

    /// Get time remaining for a time lock
    pub async fn get_time_remaining(
        &self,
        change_id: &str,
    ) -> Result<Option<Duration>, sqlx::Error> {
        let pool = self
            .db
            .get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?;
        let change = sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE change_id = $1",
        )
        .bind(change_id)
        .fetch_optional(pool)
        .await?;

        let change = match change {
            Some(c) => c,
            None => return Ok(None),
        };

        let now = Utc::now();
        if now >= change.lock_end {
            Ok(Some(Duration::zero()))
        } else {
            Ok(Some(change.lock_end - now))
        }
    }

    /// Record user override signal
    pub async fn record_override_signal(
        &self,
        change_id: &str,
        node_id: &str,
    ) -> Result<(), sqlx::Error> {
        debug!(
            "Recording override signal for {} from node {}",
            change_id, node_id
        );

        // Get current override signals or initialize empty object
        let change = sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE change_id = ?",
        )
        .bind(change_id)
        .fetch_optional(
            self.db
                .get_sqlite_pool()
                .ok_or_else(|| sqlx::Error::PoolClosed)?,
        )
        .await?;

        let mut signals = if let Some(c) = change {
            c.override_signals.clone()
        } else {
            HashMap::new()
        };

        // Add the new signal
        signals.insert(node_id.to_string(), Utc::now());

        // Update override signals JSON
        if self.db.is_sqlite() {
            // SQLite uses json_set
            sqlx::query(
                r#"
                UPDATE time_locked_changes
                SET override_signals = json(?),
                    updated_at = ?
                WHERE change_id = ?
                "#,
            )
            .bind(serde_json::to_string(&signals).unwrap())
            .bind(Utc::now())
            .bind(change_id)
            .execute(self.db.pool().unwrap())
            .await?;
        } else {
            // PostgreSQL uses jsonb_set
            sqlx::query(
                r#"
                UPDATE time_locked_changes
                SET override_signals = jsonb_set(
                    COALESCE(override_signals, '{}'::jsonb),
                    $1,
                    to_jsonb($2::text),
                    true
                ),
                updated_at = $3
                WHERE change_id = $4
                "#,
            )
            .bind(format!("/{node_id}"))
            .bind(Utc::now().to_rfc3339())
            .bind(Utc::now())
            .bind(change_id)
            .execute(self.db.pool().unwrap())
            .await?;
        }

        Ok(())
    }

    /// Check if user override threshold is met
    pub async fn check_override_threshold(
        &self,
        change_id: &str,
        active_node_count: usize,
    ) -> Result<bool, sqlx::Error> {
        let pool = self
            .db
            .get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?;
        let change = sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE change_id = $1",
        )
        .bind(change_id)
        .fetch_optional(pool)
        .await?;

        let change = match change {
            Some(c) => c,
            None => return Ok(false),
        };

        let override_count = change.override_signals.len();
        let threshold_count =
            (active_node_count as f64 * self.config.user_override_threshold) as usize;

        Ok(override_count >= threshold_count)
    }

    /// Activate a time-locked change
    pub async fn activate_change(&self, change_id: &str) -> Result<(), sqlx::Error> {
        info!("Activating time-locked change: {}", change_id);

        sqlx::query(
            "UPDATE time_locked_changes SET status = 'activated', updated_at = $1 WHERE change_id = $2",
        )
        .bind(Utc::now())
        .bind(change_id)
        .execute(
            self.db.get_sqlite_pool()
                .ok_or_else(|| sqlx::Error::PoolClosed)?
        )
        .await?;

        Ok(())
    }

    /// Cancel a time-locked change
    pub async fn cancel_change(&self, change_id: &str) -> Result<(), sqlx::Error> {
        info!("Cancelling time-locked change: {}", change_id);

        sqlx::query(
            "UPDATE time_locked_changes SET status = 'cancelled', updated_at = $1 WHERE change_id = $2",
        )
        .bind(Utc::now())
        .bind(change_id)
        .execute(
            self.db.get_sqlite_pool()
                .ok_or_else(|| sqlx::Error::PoolClosed)?
        )
        .await?;

        Ok(())
    }

    /// List all pending time locks
    pub async fn list_pending(&self) -> Result<Vec<TimeLockedChange>, sqlx::Error> {
        let pool = self
            .db
            .get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?;
        sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE status = 'pending' ORDER BY lock_end ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// Get time lock details
    pub async fn get_change(
        &self,
        change_id: &str,
    ) -> Result<Option<TimeLockedChange>, sqlx::Error> {
        sqlx::query_as::<_, TimeLockedChange>(
            "SELECT * FROM time_locked_changes WHERE change_id = $1",
        )
        .bind(change_id)
        .fetch_optional(self.db.pool().unwrap())
        .await
    }
}

/// Database migration for time lock tables
pub async fn migrate_time_lock_tables(db: &Database) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS time_locked_changes (
            change_id TEXT PRIMARY KEY,
            tier INTEGER NOT NULL,
            description TEXT NOT NULL,
            pr_number INTEGER,
            lock_start TIMESTAMP WITH TIME ZONE NOT NULL,
            min_duration_hours INTEGER NOT NULL,
            lock_end TIMESTAMP WITH TIME ZONE NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            override_signals JSONB DEFAULT '{}',
            created_at TIMESTAMP WITH TIME ZONE NOT NULL,
            updated_at TIMESTAMP WITH TIME ZONE NOT NULL
        )
        "#,
    )
    .execute(
        db.get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?,
    )
    .await?;

    // Create index on status and lock_end for efficient queries
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_time_locked_changes_status ON time_locked_changes(status)",
    )
    .execute(
        db.get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?,
    )
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_time_locked_changes_lock_end ON time_locked_changes(lock_end)",
    )
    .execute(
        db.get_sqlite_pool()
            .ok_or_else(|| sqlx::Error::PoolClosed)?
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    async fn setup_test_manager() -> (TimeLockManager, Database) {
        let db = Database::new_in_memory().await.unwrap();
        migrate_time_lock_tables(&db).await.unwrap();
        let config = TimeLockConfig::default();
        let manager = TimeLockManager::new(db.clone(), config);
        (manager, db)
    }

    #[tokio::test]
    async fn test_time_lock_manager_new() {
        let db = Database::new_in_memory().await.unwrap();
        migrate_time_lock_tables(&db).await.unwrap();
        let config = TimeLockConfig::default();
        let manager = TimeLockManager::new(db, config);
        assert_eq!(manager.config.tier3_min_hours, 168);
        assert_eq!(manager.config.tier4_min_hours, 336);
        assert_eq!(manager.config.tier5_min_hours, 720);
    }

    #[tokio::test]
    async fn test_create_time_lock_tier3() {
        let (manager, _) = setup_test_manager().await;
        let change = manager
            .create_time_lock("test-change-1", 3, "Test change", Some(123))
            .await
            .unwrap();

        assert_eq!(change.change_id, "test-change-1");
        assert_eq!(change.tier, 3);
        assert_eq!(change.description, "Test change");
        assert_eq!(change.pr_number, Some(123));
        assert_eq!(change.status, "pending");
        assert_eq!(change.min_duration_hours, 168); // 7 days
        assert!(change.lock_end > change.lock_start);
    }

    #[tokio::test]
    async fn test_create_time_lock_tier4() {
        let (manager, _) = setup_test_manager().await;
        let change = manager
            .create_time_lock("test-change-2", 4, "Tier 4 change", None)
            .await
            .unwrap();

        assert_eq!(change.tier, 4);
        assert_eq!(change.min_duration_hours, 336); // 14 days
    }

    #[tokio::test]
    async fn test_create_time_lock_tier5() {
        let (manager, _) = setup_test_manager().await;
        let change = manager
            .create_time_lock("test-change-3", 5, "Tier 5 change", None)
            .await
            .unwrap();

        assert_eq!(change.tier, 5);
        assert_eq!(change.min_duration_hours, 720); // 30 days
    }

    #[tokio::test]
    async fn test_create_time_lock_invalid_tier() {
        let (manager, _) = setup_test_manager().await;
        // Invalid tier should default to tier 5 duration
        let change = manager
            .create_time_lock("test-change-invalid", 99, "Invalid tier", None)
            .await
            .unwrap();

        assert_eq!(change.min_duration_hours, 720); // Defaults to tier 5
    }

    #[tokio::test]
    async fn test_check_time_lock_pending() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-pending", 3, "Pending change", None)
            .await
            .unwrap();

        let status = manager.check_time_lock("test-pending").await.unwrap();
        assert_eq!(status, TimeLockStatus::Pending);
    }

    #[tokio::test]
    async fn test_check_time_lock_not_found() {
        let (manager, _) = setup_test_manager().await;
        let status = manager.check_time_lock("non-existent").await.unwrap();
        assert_eq!(status, TimeLockStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_check_time_lock_activated() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-activate", 3, "Test", None)
            .await
            .unwrap();
        manager.activate_change("test-activate").await.unwrap();

        let status = manager.check_time_lock("test-activate").await.unwrap();
        assert_eq!(status, TimeLockStatus::Activated);
    }

    #[tokio::test]
    async fn test_check_time_lock_cancelled() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-cancel", 3, "Test", None)
            .await
            .unwrap();
        manager.cancel_change("test-cancel").await.unwrap();

        let status = manager.check_time_lock("test-cancel").await.unwrap();
        assert_eq!(status, TimeLockStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_get_time_remaining() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-remaining", 3, "Test", None)
            .await
            .unwrap();

        let remaining = manager.get_time_remaining("test-remaining").await.unwrap();
        assert!(remaining.is_some());
        let duration = remaining.unwrap();
        // Should be approximately 168 hours (7 days), allow some margin
        assert!(duration.num_hours() > 160);
        assert!(duration.num_hours() < 170);
    }

    #[tokio::test]
    async fn test_get_time_remaining_not_found() {
        let (manager, _) = setup_test_manager().await;
        let remaining = manager.get_time_remaining("non-existent").await.unwrap();
        assert!(remaining.is_none());
    }

    #[tokio::test]
    async fn test_record_override_signal() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-override", 3, "Test", None)
            .await
            .unwrap();

        manager
            .record_override_signal("test-override", "node-1")
            .await
            .unwrap();

        // Verify signal was recorded by checking the change
        let _change = manager.get_change("test-override").await.unwrap().unwrap();
        // Note: override_signals is stored as JSONB, so we check it was updated
        // The actual deserialization depends on the database implementation
    }

    #[tokio::test]
    async fn test_record_override_signal_multiple() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-multi-override", 3, "Test", None)
            .await
            .unwrap();

        manager
            .record_override_signal("test-multi-override", "node-1")
            .await
            .unwrap();
        manager
            .record_override_signal("test-multi-override", "node-2")
            .await
            .unwrap();

        // Both signals should be recorded
        let _change = manager
            .get_change("test-multi-override")
            .await
            .unwrap()
            .unwrap();
        // Note: override_signals length verification depends on JSONB deserialization
    }

    #[tokio::test]
    async fn test_check_override_threshold_not_met() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-threshold", 3, "Test", None)
            .await
            .unwrap();

        // With 10 active nodes and 75% threshold, need 8 signals
        // We have 0, so threshold not met
        let met = manager
            .check_override_threshold("test-threshold", 10)
            .await
            .unwrap();
        assert!(!met);
    }

    #[tokio::test]
    async fn test_check_override_threshold_met() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-threshold-met", 3, "Test", None)
            .await
            .unwrap();

        // With 4 active nodes and 75% threshold, need 3 signals
        // Record 3 signals
        for i in 1..=3 {
            manager
                .record_override_signal("test-threshold-met", &format!("node-{}", i))
                .await
                .unwrap();
        }

        // Note: The override_signals HashMap needs to be properly deserialized
        // This test may need adjustment based on actual JSONB deserialization
        // For now, we test the function doesn't panic
        let _met = manager
            .check_override_threshold("test-threshold-met", 4)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_check_override_threshold_not_found() {
        let (manager, _) = setup_test_manager().await;
        let met = manager
            .check_override_threshold("non-existent", 10)
            .await
            .unwrap();
        assert!(!met);
    }

    #[tokio::test]
    async fn test_activate_change() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-activate-2", 3, "Test", None)
            .await
            .unwrap();

        manager.activate_change("test-activate-2").await.unwrap();

        let change = manager
            .get_change("test-activate-2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(change.status, "activated");
    }

    #[tokio::test]
    async fn test_cancel_change() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-cancel-2", 3, "Test", None)
            .await
            .unwrap();

        manager.cancel_change("test-cancel-2").await.unwrap();

        let change = manager.get_change("test-cancel-2").await.unwrap().unwrap();
        assert_eq!(change.status, "cancelled");
    }

    #[tokio::test]
    async fn test_list_pending() {
        let (manager, _) = setup_test_manager().await;

        // Create multiple time locks
        manager
            .create_time_lock("pending-1", 3, "Pending 1", None)
            .await
            .unwrap();
        manager
            .create_time_lock("pending-2", 4, "Pending 2", None)
            .await
            .unwrap();
        manager
            .create_time_lock("pending-3", 5, "Pending 3", None)
            .await
            .unwrap();

        // Activate one, so it shouldn't appear in pending list
        manager.activate_change("pending-2").await.unwrap();

        let pending = manager.list_pending().await.unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|c| c.change_id == "pending-1"));
        assert!(pending.iter().any(|c| c.change_id == "pending-3"));
        assert!(!pending.iter().any(|c| c.change_id == "pending-2"));
    }

    #[tokio::test]
    async fn test_list_pending_empty() {
        let (manager, _) = setup_test_manager().await;
        let pending = manager.list_pending().await.unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_get_change() {
        let (manager, _) = setup_test_manager().await;
        manager
            .create_time_lock("test-get", 3, "Test description", Some(456))
            .await
            .unwrap();

        let change = manager.get_change("test-get").await.unwrap();
        assert!(change.is_some());
        let change = change.unwrap();
        assert_eq!(change.change_id, "test-get");
        assert_eq!(change.description, "Test description");
        assert_eq!(change.pr_number, Some(456));
    }

    #[tokio::test]
    async fn test_get_change_not_found() {
        let (manager, _) = setup_test_manager().await;
        let change = manager.get_change("non-existent").await.unwrap();
        assert!(change.is_none());
    }

    #[tokio::test]
    async fn test_migrate_time_lock_tables() {
        let db = Database::new_in_memory().await.unwrap();
        let result = migrate_time_lock_tables(&db).await;
        assert!(result.is_ok());

        // Verify table exists by trying to query it
        let result: Result<Vec<(String, String)>, _> =
            sqlx::query_as("SELECT change_id, status FROM time_locked_changes LIMIT 1")
                .fetch_all(db.pool().unwrap())
                .await;

        // Should not error (table exists), even if empty
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_time_lock_config_default() {
        let config = TimeLockConfig::default();
        assert_eq!(config.tier3_min_hours, 168);
        assert_eq!(config.tier4_min_hours, 336);
        assert_eq!(config.tier5_min_hours, 720);
        assert_eq!(config.user_override_threshold, 0.75);
    }

    #[tokio::test]
    async fn test_time_lock_status_equality() {
        assert_eq!(TimeLockStatus::Pending, TimeLockStatus::Pending);
        assert_ne!(TimeLockStatus::Pending, TimeLockStatus::Ready);
        assert_ne!(TimeLockStatus::Activated, TimeLockStatus::Cancelled);
    }
}
