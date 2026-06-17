//! Audit Logger
//!
//! Manages append-only audit log files with cryptographic hash chains
//! for tamper-evident logging of governance operations.

use anyhow::{Result, anyhow};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::audit::entry::AuditLogEntry;

/// Audit logger managing append-only JSONL file
#[derive(Clone)]
pub struct AuditLogger {
    log_path: String,
    file: Arc<Mutex<Option<File>>>,
    head_hash: Arc<Mutex<String>>,
    entry_count: Arc<Mutex<u64>>,
}

impl AuditLogger {
    /// Create new audit logger
    pub fn new(log_path: String) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = Path::new(&log_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow!("Failed to create log directory: {}", e))?;
        }

        // Open file for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| anyhow!("Failed to open audit log file: {}", e))?;

        let logger = Self {
            log_path: log_path.clone(),
            file: Arc::new(Mutex::new(Some(file))),
            head_hash: Arc::new(Mutex::new(String::new())),
            entry_count: Arc::new(Mutex::new(0)),
        };

        // Initialize if file is new (synchronous initialization)
        let path = Path::new(&log_path);
        if path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            // File is new or empty, will be initialized on first append
        }

        Ok(logger)
    }

    /// Append new entry to audit log
    pub async fn append_entry(&self, entry: AuditLogEntry) -> Result<()> {
        // Verify entry hash
        if !entry.verify_hash() {
            return Err(anyhow!("Invalid entry hash"));
        }

        // Serialize entry to JSON
        let json = serde_json::to_string(&entry)
            .map_err(|e| anyhow!("Failed to serialize entry: {}", e))?;

        // Write to file
        if let Some(file) = self.file.lock().await.as_mut() {
            writeln!(file, "{json}").map_err(|e| anyhow!("Failed to write to audit log: {}", e))?;
            file.flush()
                .map_err(|e| anyhow!("Failed to flush audit log: {}", e))?;
        } else {
            return Err(anyhow!("Audit log file not available"));
        }

        // Update head hash and count
        {
            let mut head_hash = self.head_hash.lock().await;
            *head_hash = entry.this_log_hash.clone();
        }

        {
            let mut count = self.entry_count.lock().await;
            *count += 1;
        }

        debug!("Appended audit entry: {}", entry.summary());
        Ok(())
    }

    /// Get current head hash
    pub async fn get_head_hash(&self) -> String {
        self.head_hash.lock().await.clone()
    }

    /// Get entry count
    pub async fn get_entry_count(&self) -> u64 {
        *self.entry_count.lock().await
    }

    /// Load existing entries to initialize head hash and count.
    /// Call on startup when opening an existing log file.
    pub async fn load_existing_entries(&self) -> Result<()> {
        let path = Path::new(&self.log_path);
        let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);

        if !path.exists() || file_size == 0 {
            // Create genesis entry for new log
            let genesis = crate::audit::entry::create_genesis_entry("governance-01".to_string());
            self.append_entry(genesis).await?;
            return Ok(());
        }

        let file =
            File::open(path).map_err(|e| anyhow!("Failed to open existing log file: {}", e))?;

        let reader = BufReader::new(file);
        let mut last_hash = String::new();
        let mut count = 0;

        for line in reader.lines() {
            let line = line.map_err(|e| anyhow!("Failed to read log line: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: AuditLogEntry = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Failed to parse log entry: {}", e))?;

            // Verify hash chain
            if !last_hash.is_empty() && entry.previous_log_hash != last_hash {
                return Err(anyhow!("Hash chain broken at entry {}", count));
            }

            if !entry.verify_hash() {
                return Err(anyhow!("Invalid hash in entry {}", count));
            }

            last_hash = entry.this_log_hash.clone();
            count += 1;
        }

        // Update state
        {
            let mut head_hash = self.head_hash.lock().await;
            *head_hash = last_hash;
        }

        {
            let mut entry_count = self.entry_count.lock().await;
            *entry_count = count;
        }

        info!("Loaded {} existing audit entries", count);
        Ok(())
    }

    /// Get all entries from log file
    pub async fn get_all_entries(&self) -> Result<Vec<AuditLogEntry>> {
        let path = Path::new(&self.log_path);
        if !path.exists() {
            return Ok(vec![]);
        }

        let file = File::open(path).map_err(|e| anyhow!("Failed to open log file: {}", e))?;

        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| anyhow!("Failed to read log line: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: AuditLogEntry = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Failed to parse log entry: {}", e))?;

            entries.push(entry);
        }

        Ok(entries)
    }

    /// Get entries for a specific time range
    pub async fn get_entries_in_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AuditLogEntry>> {
        let all_entries = self.get_all_entries().await?;

        let filtered: Vec<AuditLogEntry> = all_entries
            .into_iter()
            .filter(|entry| entry.timestamp >= start && entry.timestamp <= end)
            .collect();

        Ok(filtered)
    }

    /// Get entries by job type
    pub async fn get_entries_by_type(&self, job_type: &str) -> Result<Vec<AuditLogEntry>> {
        let all_entries = self.get_all_entries().await?;

        let filtered: Vec<AuditLogEntry> = all_entries
            .into_iter()
            .filter(|entry| entry.job_type == job_type)
            .collect();

        Ok(filtered)
    }

    /// Close the audit logger (flush only; does not drop file handle)
    pub async fn close(&self) -> Result<()> {
        if let Some(file) = self.file.lock().await.as_mut() {
            file.flush()
                .map_err(|e| anyhow!("Failed to flush audit log on close: {}", e))?;
        }
        Ok(())
    }

    /// Rotate log: archive current file, open new one, write rotation entry.
    /// Preserves hash chain by linking new file's first entry to previous file's last hash.
    pub async fn rotate(&self, server_id: &str) -> Result<()> {
        use std::fs;
        use std::path::Path;

        let path = Path::new(&self.log_path);

        // 1. Get current head before closing
        let previous_head = self.get_head_hash().await;
        if previous_head.is_empty() {
            // No entries yet, nothing to rotate
            return Ok(());
        }

        // 2. Flush, close, and drop file handle
        {
            let mut file_guard = self.file.lock().await;
            if let Some(ref mut f) = *file_guard {
                f.flush()
                    .map_err(|e| anyhow!("Failed to flush before rotation: {}", e))?;
            }
            *file_guard = None;
        }

        // 3. Rename current file to archive (timestamped)
        if path.exists() {
            let archive_name = format!(
                "{}.{}",
                self.log_path,
                chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ")
            );
            fs::rename(&self.log_path, &archive_name)
                .map_err(|e| anyhow!("Failed to rename audit log for rotation: {}", e))?;
            info!("Audit log rotated to {}", archive_name);
        }

        // 4. Open new file
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| anyhow!("Failed to open new audit log after rotation: {}", e))?;

        {
            let mut file_guard = self.file.lock().await;
            *file_guard = Some(new_file);
        }

        // 5. Reset entry count for new file (append_entry will increment to 1)
        {
            let mut count = self.entry_count.lock().await;
            *count = 0;
        }

        // 6. Write rotation entry (links to previous file's last hash)
        let rotation_entry =
            crate::audit::entry::create_rotation_entry(server_id.to_string(), previous_head);
        self.append_entry(rotation_entry).await?;

        info!("Audit log rotation complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_audit_logger_creation() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir
            .path()
            .join("audit.log")
            .to_string_lossy()
            .to_string();

        let logger = AuditLogger::new(log_path).unwrap();
        // Initialize by loading existing entries (which will create genesis if empty)
        logger.load_existing_entries().await.unwrap();
        assert_eq!(logger.get_entry_count().await, 1); // Genesis entry
    }

    #[tokio::test]
    async fn test_append_entry() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir
            .path()
            .join("audit.log")
            .to_string_lossy()
            .to_string();

        let logger = AuditLogger::new(log_path).unwrap();
        // Initialize by loading existing entries (which will create genesis if empty)
        logger.load_existing_entries().await.unwrap();

        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        let entry = AuditLogEntry::new(
            "test-job".to_string(),
            "test_type".to_string(),
            "governance-01".to_string(),
            "sha256:input".to_string(),
            "sha256:output".to_string(),
            logger.get_head_hash().await,
            metadata,
        );

        logger.append_entry(entry).await.unwrap();
        assert_eq!(logger.get_entry_count().await, 2); // Genesis + test entry
    }

    #[tokio::test]
    async fn test_hash_chain_verification() {
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir
            .path()
            .join("audit.log")
            .to_string_lossy()
            .to_string();

        let logger = AuditLogger::new(log_path).unwrap();
        // Initialize by loading existing entries (which will create genesis if empty)
        logger.load_existing_entries().await.unwrap();

        // Add multiple entries
        for i in 0..5 {
            let mut metadata = HashMap::new();
            metadata.insert("index".to_string(), i.to_string());

            let entry = AuditLogEntry::new(
                format!("job-{}", i),
                "test_type".to_string(),
                "governance-01".to_string(),
                format!("sha256:input{}", i),
                format!("sha256:output{}", i),
                logger.get_head_hash().await,
                metadata,
            );

            logger.append_entry(entry).await.unwrap();
        }

        // Verify all entries
        let entries = logger.get_all_entries().await.unwrap();
        assert_eq!(entries.len(), 6); // Genesis + 5 test entries

        // Verify hash chain
        for i in 1..entries.len() {
            assert_eq!(entries[i].previous_log_hash, entries[i - 1].this_log_hash);
        }
    }
}
