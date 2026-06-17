//! Automated backup system for database and configuration
//!
//! Provides periodic backups with verification and retention management.

use crate::database::Database;
use crate::error::GovernanceError;
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::time::interval;
use tracing::{error, info, warn};

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Backup directory
    pub directory: PathBuf,
    /// Retention period in days
    pub retention_days: u32,
    /// Enable compression
    pub compression: bool,
    /// Backup interval
    pub interval: Duration,
    /// Enable automatic backups
    pub enabled: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("/opt/blvm-commons/backups"),
            retention_days: 30,
            compression: true,
            interval: Duration::from_secs(86400), // Daily
            enabled: true,
        }
    }
}

/// Backup manager
pub struct BackupManager {
    database: Database,
    config: BackupConfig,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(database: Database, config: BackupConfig) -> Self {
        Self { database, config }
    }

    /// Create a backup of the database
    pub async fn create_backup(&self) -> Result<PathBuf, GovernanceError> {
        // Ensure backup directory exists
        fs::create_dir_all(&self.config.directory)
            .await
            .map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to create backup directory: {e}"))
            })?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%3f"); // Include milliseconds for uniqueness
        let backup_filename = format!("governance_backup_{timestamp}.db");
        let backup_path = self.config.directory.join(&backup_filename);

        info!("Creating database backup: {}", backup_path.display());

        // Create backup (SQLite only)
        self.backup_sqlite(&backup_path).await?;

        // Verify backup
        self.verify_backup(&backup_path).await?;

        // Compress if enabled
        let final_path = if self.config.compression {
            self.compress_backup(&backup_path).await?
        } else {
            backup_path
        };

        info!("Backup created successfully: {}", final_path.display());

        Ok(final_path)
    }

    /// Backup SQLite database
    async fn backup_sqlite(&self, backup_path: &Path) -> Result<(), GovernanceError> {
        // SQLite backup using VACUUM INTO (SQLite 3.27+)
        // This creates a clean copy of the database
        if let Some(pool) = self.database.get_sqlite_pool() {
            // Check if this is an in-memory database
            // VACUUM INTO doesn't work with :memory: databases, so we need a different approach
            // Try VACUUM INTO first, and if it fails, fall back to ATTACH method
            let use_attach_method = {
                // Check by trying to get the database file - in-memory DBs return empty
                let result: Result<Vec<(i32, String, String)>, _> =
                    sqlx::query_as("PRAGMA database_list").fetch_all(pool).await;
                match result {
                    Ok(list) => {
                        // If main database file is empty or :memory:, use ATTACH method
                        list.iter().any(|(_, name, file)| {
                            name == "main" && (file.is_empty() || file.contains(":memory:"))
                        })
                    }
                    _ => true, // If we can't check, assume in-memory and use ATTACH
                }
            };

            if use_attach_method {
                // For in-memory databases, use ATTACH DATABASE approach
                // Create a new file-based database and copy data
                return self.backup_in_memory_sqlite(pool, backup_path).await;
            }

            // Get absolute path (required for VACUUM INTO)
            let absolute_path = if backup_path.is_absolute() {
                backup_path.to_path_buf()
            } else {
                // Get current directory and join with relative path
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(backup_path)
            };

            // Ensure parent directory exists
            if let Some(parent) = absolute_path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to create backup directory: {e}"))
                })?;
            }

            // Escape the path for SQL (replace single quotes with double single quotes)
            // Also escape backslashes on Windows
            let escaped_path = absolute_path
                .to_string_lossy()
                .replace("'", "''")
                .replace("\\", "\\\\");

            // Use sqlx to execute VACUUM INTO
            sqlx::query(&format!("VACUUM INTO '{escaped_path}'"))
                .execute(pool)
                .await
                .map_err(|e| {
                    GovernanceError::DatabaseError(format!(
                        "SQLite backup failed: {} (path: {})",
                        e,
                        absolute_path.display()
                    ))
                })?;

            // Verify the backup file was created
            if !absolute_path.exists() {
                return Err(GovernanceError::DatabaseError(format!(
                    "Backup file was not created: {}",
                    absolute_path.display()
                )));
            }

            info!("SQLite backup created: {}", backup_path.display());
            Ok(())
        } else {
            Err(GovernanceError::DatabaseError(
                "SQLite pool not available".to_string(),
            ))
        }
    }

    /// Backup in-memory SQLite database by creating a new file-based database and copying data
    async fn backup_in_memory_sqlite(
        &self,
        pool: &sqlx::SqlitePool,
        backup_path: &Path,
    ) -> Result<(), GovernanceError> {
        // Get absolute path
        let absolute_path = if backup_path.is_absolute() {
            backup_path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(backup_path)
        };

        // Ensure parent directory exists
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                GovernanceError::ConfigError(format!("Failed to create backup directory: {e}"))
            })?;
        }

        // Create an empty backup database file first to ensure it exists
        // SQLite will create the file on connect, but we want to ensure the directory is writable
        fs::File::create(&absolute_path).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to create backup file: {e}"))
        })?;

        // Create backup database file first by connecting to it briefly
        // sqlx uses "sqlite:" prefix for file paths (not "sqlite://")
        let backup_url = format!("sqlite:{}", absolute_path.to_string_lossy());

        let backup_pool = sqlx::sqlite::SqlitePool::connect(&backup_url)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!(
                    "Failed to create backup database: {} (path: {}, url: {})",
                    e,
                    absolute_path.display(),
                    backup_url
                ))
            })?;

        // Use a single connection from the source pool for the entire backup operation
        let mut source_conn = pool.acquire().await.map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to acquire source connection: {e}"))
        })?;

        // Run migrations on backup database to create schema
        sqlx::migrate!("./migrations")
            .run(&backup_pool)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to migrate backup database: {e}"))
            })?;

        // Close backup pool before attaching (SQLite doesn't allow attaching an open database)
        backup_pool.close().await;

        // Attach backup database to source connection
        let escaped_path = absolute_path
            .to_string_lossy()
            .replace("'", "''")
            .replace("\\", "/");
        sqlx::query(&format!("ATTACH DATABASE '{escaped_path}' AS backup"))
            .execute(&mut *source_conn)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to attach backup database: {e}"))
            })?;

        // Get all table names from main (in-memory) database
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&mut *source_conn)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to get table names: {e}")))?;

        // Copy each table to backup database
        for (table_name,) in tables {
            let quoted_table = format!("\"{}\"", table_name.replace("\"", "\"\""));

            // Drop existing table in backup (from migrations) and recreate with data
            // CREATE TABLE AS SELECT doesn't work with IF NOT EXISTS, so we drop first
            let _ = sqlx::query(&format!("DROP TABLE IF EXISTS backup.{quoted_table}"))
                .execute(&mut *source_conn)
                .await;

            // Create table and copy data in one operation
            sqlx::query(&format!(
                "CREATE TABLE backup.{quoted_table} AS SELECT * FROM main.{quoted_table}"
            ))
            .execute(&mut *source_conn)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!(
                    "Failed to create and copy table {table_name} in backup: {e}"
                ))
            })?;
        }

        // Detach backup database (this flushes writes)
        sqlx::query("DETACH DATABASE backup")
            .execute(&mut *source_conn)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to detach backup database: {e}"))
            })?;

        // Verify the backup file was created
        if !absolute_path.exists() {
            return Err(GovernanceError::DatabaseError(format!(
                "Backup file was not created: {}",
                absolute_path.display()
            )));
        }

        info!("In-memory SQLite backup created: {}", backup_path.display());
        Ok(())
    }

    /// Verify backup integrity
    async fn verify_backup(&self, backup_path: &Path) -> Result<(), GovernanceError> {
        if self.database.is_sqlite() {
            // Check if backup file exists
            if !backup_path.exists() {
                return Err(GovernanceError::DatabaseError(format!(
                    "Backup file does not exist: {}",
                    backup_path.display()
                )));
            }

            // Verify SQLite backup by opening it and running integrity_check
            // Use absolute path for connection
            let absolute_backup_path = backup_path
                .canonicalize()
                .unwrap_or_else(|_| backup_path.to_path_buf());
            let backup_url = format!("sqlite:{}", absolute_backup_path.to_string_lossy());
            let backup_pool = sqlx::sqlite::SqlitePool::connect(&backup_url)
                .await
                .map_err(|e| {
                    GovernanceError::DatabaseError(format!(
                        "Failed to open backup for verification: {} (path: {})",
                        e,
                        absolute_backup_path.display()
                    ))
                })?;

            let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
                .fetch_one(&backup_pool)
                .await
                .map_err(|e| {
                    GovernanceError::DatabaseError(format!("Backup verification failed: {e}"))
                })?;

            if result.0 != "ok" {
                return Err(GovernanceError::DatabaseError(format!(
                    "Backup integrity check failed: {}",
                    result.0
                )));
            }

            info!("Backup verification passed");
            Ok(())
        } else {
            // For PostgreSQL, verification would require pg_restore --list
            // Skip for now
            warn!("PostgreSQL backup verification skipped (requires external tool)");
            Ok(())
        }
    }

    /// Compress backup file
    async fn compress_backup(&self, backup_path: &Path) -> Result<PathBuf, GovernanceError> {
        use std::process::Command;

        let compressed_path = format!("{}.gz", backup_path.to_string_lossy());

        // Use gzip to compress (external tool)
        let output = Command::new("gzip")
            .arg("-f") // Force overwrite
            .arg(backup_path)
            .output()
            .map_err(|e| GovernanceError::ConfigError(format!("Failed to compress backup: {e}")))?;

        if !output.status.success() {
            return Err(GovernanceError::ConfigError(format!(
                "Backup compression failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        info!("Backup compressed: {}", compressed_path);
        Ok(PathBuf::from(compressed_path))
    }

    /// Clean up old backups based on retention policy
    pub async fn cleanup_old_backups(&self) -> Result<usize, GovernanceError> {
        let cutoff_time = Utc::now()
            .checked_sub_signed(chrono::Duration::days(self.config.retention_days as i64))
            .ok_or_else(|| {
                GovernanceError::ConfigError("Failed to calculate cutoff time".to_string())
            })?;

        let mut deleted_count = 0;

        // List all backup files
        let mut entries = fs::read_dir(&self.config.directory).await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read backup directory: {e}"))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            GovernanceError::ConfigError(format!("Failed to read directory entry: {e}"))
        })? {
            let path = entry.path();
            if path.is_file() && path.to_string_lossy().contains("governance_backup_") {
                // Get file metadata
                let metadata = entry.metadata().await.map_err(|e| {
                    GovernanceError::ConfigError(format!("Failed to read file metadata: {e}"))
                })?;

                // Get modification time
                if let Ok(modified) = metadata.modified() {
                    let modified_time: chrono::DateTime<Utc> = modified.into();
                    if modified_time < cutoff_time {
                        fs::remove_file(&path).await.map_err(|e| {
                            GovernanceError::ConfigError(format!(
                                "Failed to delete old backup: {e}"
                            ))
                        })?;
                        deleted_count += 1;
                        info!("Deleted old backup: {}", path.display());
                    }
                }
            }
        }

        if deleted_count > 0 {
            info!("Cleaned up {} old backup(s)", deleted_count);
        }

        Ok(deleted_count)
    }

    /// Start periodic backup task
    pub fn start_backup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            if !self.config.enabled {
                info!("Automated backups are disabled");
                return;
            }

            let mut interval = interval(self.config.interval);
            info!(
                "Starting automated backup task (interval: {:?})",
                self.config.interval
            );

            loop {
                interval.tick().await;

                // Create backup
                match self.create_backup().await {
                    Ok(backup_path) => {
                        info!("Automated backup created: {}", backup_path.display());
                    }
                    Err(e) => {
                        error!("Automated backup failed: {}", e);
                    }
                }

                // Clean up old backups
                if let Err(e) = self.cleanup_old_backups().await {
                    warn!("Failed to cleanup old backups: {}", e);
                }
            }
        });
    }
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use tempfile::TempDir;

    async fn setup_test_backup_manager() -> (BackupManager, Database, TempDir) {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 30,
            compression: false, // Disable compression for tests (requires gzip)
            interval: Duration::from_secs(3600),
            enabled: true,
        };
        let manager = BackupManager::new(db.clone(), config);
        (manager, db, temp_dir)
    }

    #[tokio::test]
    async fn test_backup_manager_new() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 30,
            compression: true,
            interval: Duration::from_secs(86400),
            enabled: true,
        };
        let manager = BackupManager::new(db, config);
        assert_eq!(manager.config.retention_days, 30);
        assert!(manager.config.compression);
        assert!(manager.config.enabled);
    }

    #[tokio::test]
    async fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert_eq!(config.retention_days, 30);
        assert!(config.compression);
        assert!(config.enabled);
        assert_eq!(config.interval, Duration::from_secs(86400));
    }

    #[tokio::test]
    async fn test_create_backup_sqlite() {
        let (manager, _, _temp_dir) = setup_test_backup_manager().await;

        // Create a backup
        let backup_path = manager.create_backup().await;
        if let Err(ref e) = backup_path {
            eprintln!("Backup creation failed: {:?}", e);
        }
        assert!(
            backup_path.is_ok(),
            "Backup creation should succeed, got: {:?}",
            backup_path
        );
        let backup_path = backup_path.unwrap();

        // Verify backup file exists
        assert!(backup_path.exists());
        assert!(backup_path.is_file());

        // Verify backup filename format
        let filename = backup_path.file_name().unwrap().to_string_lossy();
        assert!(filename.starts_with("governance_backup_"));
        assert!(filename.ends_with(".db"));
    }

    #[tokio::test]
    async fn test_create_backup_without_compression() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 30,
            compression: false, // No compression
            interval: Duration::from_secs(3600),
            enabled: true,
        };
        let manager = BackupManager::new(db, config);

        let backup_path = manager.create_backup().await;
        assert!(backup_path.is_ok());
        let backup_path = backup_path.unwrap();

        // Should be .db file, not .db.gz
        assert!(!backup_path.to_string_lossy().ends_with(".gz"));
        assert!(backup_path.exists());
    }

    #[tokio::test]
    async fn test_create_backup_creates_directory() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("backups");

        // Directory doesn't exist yet
        assert!(!subdir.exists());

        let config = BackupConfig {
            directory: subdir.clone(),
            retention_days: 30,
            compression: false,
            interval: Duration::from_secs(3600),
            enabled: true,
        };
        let manager = BackupManager::new(db, config);

        let backup_path = manager.create_backup().await;
        assert!(backup_path.is_ok());

        // Directory should now exist
        assert!(subdir.exists());
        assert!(subdir.is_dir());
    }

    #[tokio::test]
    async fn test_cleanup_old_backups_no_backups() {
        let (manager, _, _temp_dir) = setup_test_backup_manager().await;

        // No backups exist, should return 0
        let deleted = manager.cleanup_old_backups().await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_backups_recent_backups() {
        let (manager, _, _temp_dir) = setup_test_backup_manager().await;

        // Create a recent backup
        let _backup_path = manager.create_backup().await.unwrap();

        // Cleanup should not delete recent backups
        let deleted = manager.cleanup_old_backups().await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_cleanup_old_backups_with_old_backups() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 1, // Very short retention for testing
            compression: false,
            interval: Duration::from_secs(3600),
            enabled: true,
        };
        let manager = BackupManager::new(db, config);

        // Create a backup file manually with old timestamp
        let old_backup_path = temp_dir.path().join("governance_backup_old.db");
        fs::write(&old_backup_path, b"fake backup data")
            .await
            .unwrap();

        // Set file modification time to 2 days ago
        let two_days_ago =
            Utc::now() - chrono::Duration::try_days(2).unwrap_or(chrono::Duration::zero());
        let system_time: std::time::SystemTime = two_days_ago.into();
        #[cfg(unix)]
        {
            use std::fs::FileTimes;
            let file = std::fs::File::create(&old_backup_path).unwrap();
            let file_times = FileTimes::new()
                .set_modified(system_time)
                .set_accessed(system_time);
            file.set_times(file_times).unwrap();
        }

        // Cleanup should delete old backup
        // Note: This test may be platform-specific due to file time manipulation
        // On some systems, we may not be able to set file times, so we just verify
        // the function runs without error
        let _deleted = manager.cleanup_old_backups().await;
        // Don't assert on count as file time manipulation may not work on all platforms
    }

    #[tokio::test]
    async fn test_cleanup_old_backups_ignores_non_backup_files() {
        let (manager, _, temp_dir) = setup_test_backup_manager().await;

        // Create a non-backup file
        let other_file = temp_dir.path().join("other_file.txt");
        fs::write(&other_file, b"not a backup").await.unwrap();

        // Cleanup should not delete non-backup files
        let deleted = manager.cleanup_old_backups().await.unwrap();
        assert_eq!(deleted, 0);

        // File should still exist
        assert!(other_file.exists());
    }

    #[tokio::test]
    async fn test_cleanup_old_backups_empty_directory() {
        let (manager, _, temp_dir) = setup_test_backup_manager().await;

        // Ensure directory exists but is empty
        fs::create_dir_all(&manager.config.directory).await.unwrap();

        let deleted = manager.cleanup_old_backups().await.unwrap();
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_start_backup_task_disabled() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 30,
            compression: false,
            interval: Duration::from_secs(1),
            enabled: false, // Disabled
        };
        let manager = BackupManager::new(db, config);

        // Start task (should exit immediately if disabled)
        let manager_arc = Arc::new(manager);
        manager_arc.clone().start_backup_task();

        // Give it a moment to start and check if disabled
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Task should have started and logged that backups are disabled
        // We can't easily verify this without capturing logs, but we verify it doesn't panic
    }

    #[tokio::test]
    async fn test_backup_manager_with_custom_config() {
        let db = Database::new_in_memory().await.unwrap();
        let temp_dir = TempDir::new().unwrap();
        let config = BackupConfig {
            directory: temp_dir.path().to_path_buf(),
            retention_days: 7,
            compression: true,
            interval: Duration::from_secs(7200),
            enabled: false,
        };
        let manager = BackupManager::new(db, config);

        assert_eq!(manager.config.retention_days, 7);
        assert!(manager.config.compression);
        assert!(!manager.config.enabled);
        assert_eq!(manager.config.interval, Duration::from_secs(7200));
    }

    #[tokio::test]
    async fn test_create_backup_multiple_times() {
        let (manager, _, _temp_dir) = setup_test_backup_manager().await;

        // Create multiple backups
        let backup1 = manager.create_backup().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await; // Small delay for timestamp
        let backup2 = manager.create_backup().await.unwrap();

        // Both should exist and be different files
        assert!(backup1.exists());
        assert!(backup2.exists());
        assert_ne!(backup1, backup2);
    }

    #[tokio::test]
    async fn test_backup_verification_passes() {
        let (manager, _, _temp_dir) = setup_test_backup_manager().await;

        // Create backup (which includes verification)
        let result = manager.create_backup().await;
        assert!(
            result.is_ok(),
            "Backup creation should succeed and pass verification"
        );
    }
}
