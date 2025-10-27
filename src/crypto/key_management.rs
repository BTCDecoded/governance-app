//! Production Key Management System
//!
//! Handles secure key generation, storage, rotation, and lifecycle management
//! for the BTCDecoded Governance System.

use rand;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokio::sync::RwLock;
use tracing::info;

use super::signatures::SignatureManager;
use crate::error::GovernanceError;

/// Key types supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    Maintainer,
    EconomicNode,
    Emergency,
    GitHubApp,
    System,
}

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::Maintainer => "maintainer",
            KeyType::EconomicNode => "economic_node",
            KeyType::Emergency => "emergency",
            KeyType::GitHubApp => "github_app",
            KeyType::System => "system",
        }
    }
}

impl std::str::FromStr for KeyType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "maintainer" => Ok(KeyType::Maintainer),
            "economic_node" => Ok(KeyType::EconomicNode),
            "emergency" => Ok(KeyType::Emergency),
            "github_app" => Ok(KeyType::GitHubApp),
            "system" => Ok(KeyType::System),
            _ => Err(format!("Unknown key type: {}", s)),
        }
    }
}

impl KeyType {
    /// Get key rotation period for this key type
    pub fn rotation_period(&self) -> Duration {
        match self {
            KeyType::Maintainer => Duration::from_secs(365 * 24 * 60 * 60), // 1 year
            KeyType::EconomicNode => Duration::from_secs(365 * 24 * 60 * 60), // 1 year
            KeyType::Emergency => Duration::from_secs(180 * 24 * 60 * 60),  // 6 months
            KeyType::GitHubApp => Duration::from_secs(90 * 24 * 60 * 60),   // 3 months
            KeyType::System => Duration::from_secs(365 * 24 * 60 * 60),     // 1 year
        }
    }

    /// Get key strength requirements for this key type
    pub fn key_strength(&self) -> KeyStrength {
        match self {
            KeyType::Maintainer => KeyStrength::High,
            KeyType::EconomicNode => KeyStrength::High,
            KeyType::Emergency => KeyStrength::Critical,
            KeyType::GitHubApp => KeyStrength::Medium,
            KeyType::System => KeyStrength::High,
        }
    }
}

/// Key strength levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStrength {
    Low,
    Medium,
    High,
    Critical,
}

/// Key status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    Active,
    Pending,
    Revoked,
    Expired,
    Compromised,
}

impl KeyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyStatus::Active => "active",
            KeyStatus::Pending => "pending",
            KeyStatus::Revoked => "revoked",
            KeyStatus::Expired => "expired",
            KeyStatus::Compromised => "compromised",
        }
    }
}

impl std::str::FromStr for KeyStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(KeyStatus::Active),
            "pending" => Ok(KeyStatus::Pending),
            "revoked" => Ok(KeyStatus::Revoked),
            "expired" => Ok(KeyStatus::Expired),
            "compromised" => Ok(KeyStatus::Compromised),
            _ => Err(format!("Unknown key status: {}", s)),
        }
    }
}

/// Key metadata stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub id: Option<i32>,
    pub key_id: String,
    pub key_type: KeyType,
    pub owner: String,
    pub public_key: String,
    pub status: KeyStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub usage_count: u64,
    pub metadata: HashMap<String, String>,
}

/// Key rotation policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationPolicy {
    pub key_type: KeyType,
    pub rotation_period_days: u32,
    pub grace_period_days: u32,
    pub auto_rotation: bool,
    pub require_approval: bool,
    pub approval_threshold: u32,
}

/// Key management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyManagementConfig {
    pub hsm_enabled: bool,
    pub hsm_provider: Option<String>,
    pub backup_enabled: bool,
    pub backup_location: Option<String>,
    pub encryption_enabled: bool,
    pub rotation_policies: Vec<KeyRotationPolicy>,
}

/// Production key manager
pub struct KeyManager {
    pool: SqlitePool,
    signature_manager: SignatureManager,
    config: KeyManagementConfig,
    key_cache: Arc<RwLock<HashMap<String, KeyMetadata>>>,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new(pool: SqlitePool, config: KeyManagementConfig) -> Self {
        Self {
            pool,
            signature_manager: SignatureManager::new(),
            config,
            key_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new key pair
    pub async fn generate_key_pair(
        &self,
        key_type: KeyType,
        owner: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<KeyMetadata, GovernanceError> {
        info!(
            "Generating new key pair for {} (type: {:?})",
            owner, key_type
        );

        // Generate key pair using signature manager
        let keypair = self.signature_manager.generate_keypair()?;
        let public_key = keypair.public_key.to_string();
        let key_id = self.generate_key_id(&key_type, owner)?;

        // Calculate expiration date
        let rotation_period = key_type.rotation_period();
        let expires_at = Utc::now()
            + chrono::Duration::from_std(rotation_period)
                .map_err(|e| GovernanceError::CryptoError(format!("Invalid duration: {}", e)))?;

        // Create key metadata
        let key_metadata = KeyMetadata {
            id: None,
            key_id: key_id.clone(),
            key_type: key_type.clone(),
            owner: owner.to_string(),
            public_key,
            status: KeyStatus::Pending,
            created_at: Utc::now(),
            expires_at,
            last_used: None,
            usage_count: 0,
            metadata: metadata.unwrap_or_default(),
        };

        // Store in database
        self.store_key_metadata(&key_metadata).await?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            cache.insert(key_id.clone(), key_metadata.clone());
        }

        info!("Key pair generated successfully: {}", key_id);
        Ok(key_metadata)
    }

    /// Store key metadata in database
    async fn store_key_metadata(&self, metadata: &KeyMetadata) -> Result<(), GovernanceError> {
        sqlx::query(
            r#"
            INSERT INTO key_metadata 
            (key_id, key_type, owner, public_key, status, created_at, expires_at, last_used, usage_count, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&metadata.key_id)
        .bind(metadata.key_type.as_str())
        .bind(&metadata.owner)
        .bind(&metadata.public_key)
        .bind(metadata.status.as_str())
        .bind(metadata.created_at)
        .bind(metadata.expires_at)
        .bind(metadata.last_used)
        .bind(metadata.usage_count as i64)
        .bind(serde_json::to_string(&metadata.metadata)?)
        .execute(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to store key metadata: {}", e)))?;

        Ok(())
    }

    /// Generate unique key ID
    fn generate_key_id(&self, key_type: &KeyType, owner: &str) -> Result<String, GovernanceError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| GovernanceError::CryptoError(format!("Time error: {}", e)))?
            .as_secs();

        let random_part = rand::random::<u32>();
        Ok(format!(
            "{}_{}_{}_{}",
            key_type.as_str(),
            owner,
            timestamp,
            random_part
        ))
    }

    /// Get key metadata by key ID
    pub async fn get_key_metadata(
        &self,
        key_id: &str,
    ) -> Result<Option<KeyMetadata>, GovernanceError> {
        // Check cache first
        {
            let cache = self.key_cache.read().await;
            if let Some(metadata) = cache.get(key_id) {
                return Ok(Some(metadata.clone()));
            }
        }

        // Query database
        let row = sqlx::query(
            r#"
            SELECT id, key_id, key_type, owner, public_key, status, created_at, expires_at, last_used, usage_count, metadata
            FROM key_metadata
            WHERE key_id = ?
            "#
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to fetch key metadata: {}", e)))?;

        if let Some(row) = row {
            let metadata = KeyMetadata {
                id: Some(row.get("id")),
                key_id: row.get::<String, _>("key_id"),
                key_type: row.get::<String, _>("key_type").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key type: {}", e))
                })?,
                owner: row.get::<String, _>("owner"),
                public_key: row.get::<String, _>("public_key"),
                status: row.get::<String, _>("status").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key status: {}", e))
                })?,
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
                expires_at: row.get::<DateTime<Utc>, _>("expires_at"),
                last_used: row.get::<Option<DateTime<Utc>>, _>("last_used"),
                usage_count: row.get::<i64, _>("usage_count") as u64,
                metadata: serde_json::from_str(&row.get::<String, _>("metadata"))?,
            };

            // Update cache
            {
                let mut cache = self.key_cache.write().await;
                cache.insert(key_id.to_string(), metadata.clone());
            }

            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// Update key usage statistics
    pub async fn update_key_usage(&self, key_id: &str) -> Result<(), GovernanceError> {
        let now = Utc::now();

        sqlx::query(
            r#"
            UPDATE key_metadata 
            SET last_used = ?, usage_count = usage_count + 1
            WHERE key_id = ?
            "#,
        )
        .bind(now)
        .bind(key_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to update key usage: {}", e))
        })?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            if let Some(metadata) = cache.get_mut(key_id) {
                metadata.last_used = Some(now);
                metadata.usage_count += 1;
            }
        }

        Ok(())
    }

    /// Rotate a key
    pub async fn rotate_key(
        &self,
        key_id: &str,
        new_owner: Option<&str>,
    ) -> Result<KeyMetadata, GovernanceError> {
        info!("Rotating key: {}", key_id);

        // Get current key metadata
        let current_metadata = self
            .get_key_metadata(key_id)
            .await?
            .ok_or_else(|| GovernanceError::CryptoError("Key not found".to_string()))?;

        // Check if key is eligible for rotation
        if current_metadata.status != KeyStatus::Active {
            return Err(GovernanceError::CryptoError(
                "Key is not active".to_string(),
            ));
        }

        // Generate new key pair
        let owner = new_owner.unwrap_or(&current_metadata.owner);
        let new_metadata = self
            .generate_key_pair(
                current_metadata.key_type.clone(),
                owner,
                Some(current_metadata.metadata.clone()),
            )
            .await?;

        // Mark old key as revoked
        self.revoke_key(key_id, "Key rotated").await?;

        info!(
            "Key rotated successfully: {} -> {}",
            key_id, new_metadata.key_id
        );
        Ok(new_metadata)
    }

    /// Revoke a key
    pub async fn revoke_key(&self, key_id: &str, reason: &str) -> Result<(), GovernanceError> {
        info!("Revoking key: {} (reason: {})", key_id, reason);

        sqlx::query(
            r#"
            UPDATE key_metadata 
            SET status = ?, metadata = json_set(metadata, '$.revocation_reason', ?)
            WHERE key_id = ?
            "#,
        )
        .bind(KeyStatus::Revoked.as_str())
        .bind(reason)
        .bind(key_id)
        .execute(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to revoke key: {}", e)))?;

        // Update cache
        {
            let mut cache = self.key_cache.write().await;
            if let Some(metadata) = cache.get_mut(key_id) {
                metadata.status = KeyStatus::Revoked;
                metadata
                    .metadata
                    .insert("revocation_reason".to_string(), reason.to_string());
            }
        }

        info!("Key revoked successfully: {}", key_id);
        Ok(())
    }

    /// Get keys by type and status
    pub async fn get_keys_by_type_and_status(
        &self,
        key_type: &KeyType,
        status: &KeyStatus,
    ) -> Result<Vec<KeyMetadata>, GovernanceError> {
        let rows = sqlx::query(
            r#"
            SELECT id, key_id, key_type, owner, public_key, status, created_at, expires_at, last_used, usage_count, metadata
            FROM key_metadata
            WHERE key_type = ? AND status = ?
            ORDER BY created_at DESC
            "#
        )
        .bind(key_type.as_str())
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to fetch keys: {}", e)))?;

        let mut keys = Vec::new();
        for row in rows {
            let metadata = KeyMetadata {
                id: Some(row.get("id")),
                key_id: row.get::<String, _>("key_id"),
                key_type: row.get::<String, _>("key_type").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key type: {}", e))
                })?,
                owner: row.get::<String, _>("owner"),
                public_key: row.get::<String, _>("public_key"),
                status: row.get::<String, _>("status").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key status: {}", e))
                })?,
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
                expires_at: row.get::<DateTime<Utc>, _>("expires_at"),
                last_used: row.get::<Option<DateTime<Utc>>, _>("last_used"),
                usage_count: row.get::<i64, _>("usage_count") as u64,
                metadata: serde_json::from_str(&row.get::<String, _>("metadata"))?,
            };
            keys.push(metadata);
        }

        Ok(keys)
    }

    /// Check for keys that need rotation
    pub async fn check_rotation_needed(&self) -> Result<Vec<KeyMetadata>, GovernanceError> {
        let now = Utc::now();
        let rotation_threshold = now
            - chrono::Duration::try_days(30)
                .ok_or_else(|| GovernanceError::CryptoError("Invalid duration".to_string()))?; // 30 days before expiration

        let rows = sqlx::query(
            r#"
            SELECT id, key_id, key_type, owner, public_key, status, created_at, expires_at, last_used, usage_count, metadata
            FROM key_metadata
            WHERE status = 'active' AND expires_at < ?
            ORDER BY expires_at ASC
            "#
        )
        .bind(rotation_threshold)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to fetch keys needing rotation: {}", e)))?;

        let mut keys = Vec::new();
        for row in rows {
            let metadata = KeyMetadata {
                id: Some(row.get("id")),
                key_id: row.get::<String, _>("key_id"),
                key_type: row.get::<String, _>("key_type").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key type: {}", e))
                })?,
                owner: row.get::<String, _>("owner"),
                public_key: row.get::<String, _>("public_key"),
                status: row.get::<String, _>("status").parse().map_err(|e| {
                    GovernanceError::CryptoError(format!("Invalid key status: {}", e))
                })?,
                created_at: row.get::<DateTime<Utc>, _>("created_at"),
                expires_at: row.get::<DateTime<Utc>, _>("expires_at"),
                last_used: row.get::<Option<DateTime<Utc>>, _>("last_used"),
                usage_count: row.get::<i64, _>("usage_count") as u64,
                metadata: serde_json::from_str(&row.get::<String, _>("metadata"))?,
            };
            keys.push(metadata);
        }

        Ok(keys)
    }

    /// Get key statistics
    pub async fn get_key_statistics(&self) -> Result<KeyStatistics, GovernanceError> {
        let total_keys = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM key_metadata")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                GovernanceError::DatabaseError(format!("Failed to get total keys: {}", e))
            })?;

        let active_keys = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM key_metadata WHERE status = 'active'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| GovernanceError::DatabaseError(format!("Failed to get active keys: {}", e)))?;

        let expired_keys = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM key_metadata WHERE status = 'expired'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to get expired keys: {}", e))
        })?;

        let revoked_keys = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM key_metadata WHERE status = 'revoked'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            GovernanceError::DatabaseError(format!("Failed to get revoked keys: {}", e))
        })?;

        Ok(KeyStatistics {
            total_keys: total_keys as u64,
            active_keys: active_keys as u64,
            expired_keys: expired_keys as u64,
            revoked_keys: revoked_keys as u64,
        })
    }
}

/// Key statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStatistics {
    pub total_keys: u64,
    pub active_keys: u64,
    pub expired_keys: u64,
    pub revoked_keys: u64,
}

// Helper functions for string conversion

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[tokio::test]
    #[ignore] // Skip database tests for now
    async fn test_key_generation() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new("sqlite::memory:test_key_gen").await?;
        db.run_migrations().await?;
        let config = KeyManagementConfig {
            hsm_enabled: false,
            hsm_provider: None,
            backup_enabled: false,
            backup_location: None,
            encryption_enabled: false,
            rotation_policies: vec![],
        };
        let key_manager = KeyManager::new(db.pool().unwrap().clone(), config);

        let metadata = key_manager
            .generate_key_pair(KeyType::Maintainer, "test@example.com", None)
            .await?;

        assert_eq!(metadata.key_type, KeyType::Maintainer);
        assert_eq!(metadata.owner, "test@example.com");
        assert_eq!(metadata.status, KeyStatus::Pending);

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Skip database tests for now
    async fn test_key_retrieval() -> Result<(), Box<dyn std::error::Error>> {
        let db = Database::new("sqlite::memory:test_key_ret").await?;
        db.run_migrations().await?;
        let config = KeyManagementConfig {
            hsm_enabled: false,
            hsm_provider: None,
            backup_enabled: false,
            backup_location: None,
            encryption_enabled: false,
            rotation_policies: vec![],
        };
        let key_manager = KeyManager::new(db.pool().unwrap().clone(), config);

        let metadata = key_manager
            .generate_key_pair(KeyType::Maintainer, "test@example.com", None)
            .await?;

        let retrieved = key_manager.get_key_metadata(&metadata.key_id).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().key_id, metadata.key_id);

        Ok(())
    }
}
