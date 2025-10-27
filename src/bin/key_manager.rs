//! Key Management CLI Tool
//!
//! Command-line tool for managing keys in the BTCDecoded Governance System

use clap::{Parser, Subcommand};
use governance_app::crypto::key_management::{KeyManagementConfig, KeyManager, KeyStatus, KeyType};
use governance_app::database::Database;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "key-manager")]
#[command(about = "BTCDecoded Governance System Key Management Tool")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database URL
    #[arg(long, default_value = "sqlite://governance.db")]
    database_url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new key pair
    Generate {
        /// Key type (maintainer, economic_node, emergency, github_app, system)
        #[arg(short, long)]
        key_type: String,

        /// Owner identifier (email, username, etc.)
        #[arg(short, long)]
        owner: String,

        /// Additional metadata (key=value pairs)
        #[arg(short, long)]
        metadata: Vec<String>,
    },

    /// List keys
    List {
        /// Filter by key type
        #[arg(short, long)]
        key_type: Option<String>,

        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by owner
        #[arg(short, long)]
        owner: Option<String>,
    },

    /// Get key details
    Get {
        /// Key ID
        key_id: String,
    },

    /// Rotate a key
    Rotate {
        /// Key ID to rotate
        key_id: String,

        /// New owner (optional)
        #[arg(short, long)]
        new_owner: Option<String>,
    },

    /// Revoke a key
    Revoke {
        /// Key ID to revoke
        key_id: String,

        /// Revocation reason
        #[arg(short, long)]
        reason: String,
    },

    /// Check for keys needing rotation
    CheckRotation,

    /// Get key statistics
    Stats,

    /// Update key usage
    UpdateUsage {
        /// Key ID
        key_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize database
    let db = Database::new(&cli.database_url).await?;

    // Initialize key manager
    let config = KeyManagementConfig {
        hsm_enabled: false,
        hsm_provider: None,
        backup_enabled: false,
        backup_location: None,
        encryption_enabled: false,
        rotation_policies: vec![],
    };
    let key_manager = KeyManager::new(db.pool().unwrap().clone(), config);

    // Execute command
    match cli.command {
        Commands::Generate {
            key_type,
            owner,
            metadata,
        } => {
            let key_type = parse_key_type(&key_type)?;
            let metadata = parse_metadata(metadata)?;

            let key_metadata = key_manager
                .generate_key_pair(key_type, &owner, Some(metadata))
                .await?;

            println!("Key generated successfully:");
            println!("  Key ID: {}", key_metadata.key_id);
            println!("  Type: {:?}", key_metadata.key_type);
            println!("  Owner: {}", key_metadata.owner);
            println!("  Public Key: {}", key_metadata.public_key);
            println!("  Status: {:?}", key_metadata.status);
            println!("  Created: {}", key_metadata.created_at);
            println!("  Expires: {}", key_metadata.expires_at);
        }

        Commands::List {
            key_type,
            status,
            owner,
        } => {
            let key_type = key_type.as_deref().map(parse_key_type).transpose()?;
            let status = status.as_deref().map(parse_key_status).transpose()?;

            if let (Some(key_type), Some(status)) = (key_type, status) {
                let keys = key_manager
                    .get_keys_by_type_and_status(&key_type, &status)
                    .await?;
                print_keys(&keys);
            } else {
                println!("Please specify both key_type and status for filtering");
            }
        }

        Commands::Get { key_id } => {
            if let Some(key_metadata) = key_manager.get_key_metadata(&key_id).await? {
                print_key_details(&key_metadata);
            } else {
                println!("Key not found: {}", key_id);
            }
        }

        Commands::Rotate { key_id, new_owner } => {
            let new_key = key_manager
                .rotate_key(&key_id, new_owner.as_deref())
                .await?;

            println!("Key rotated successfully:");
            println!("  Old Key ID: {}", key_id);
            println!("  New Key ID: {}", new_key.key_id);
            println!("  Owner: {}", new_key.owner);
            println!("  Expires: {}", new_key.expires_at);
        }

        Commands::Revoke { key_id, reason } => {
            key_manager.revoke_key(&key_id, &reason).await?;
            println!("Key revoked successfully: {}", key_id);
        }

        Commands::CheckRotation => {
            let keys_needing_rotation = key_manager.check_rotation_needed().await?;

            if keys_needing_rotation.is_empty() {
                println!("No keys need rotation");
            } else {
                println!("Keys needing rotation:");
                for key in keys_needing_rotation {
                    println!(
                        "  {} ({}) - expires {}",
                        key.key_id, key.owner, key.expires_at
                    );
                }
            }
        }

        Commands::Stats => {
            let stats = key_manager.get_key_statistics().await?;

            println!("Key Statistics:");
            println!("  Total Keys: {}", stats.total_keys);
            println!("  Active Keys: {}", stats.active_keys);
            println!("  Expired Keys: {}", stats.expired_keys);
            println!("  Revoked Keys: {}", stats.revoked_keys);
        }

        Commands::UpdateUsage { key_id } => {
            key_manager.update_key_usage(&key_id).await?;
            println!("Key usage updated: {}", key_id);
        }
    }

    Ok(())
}

fn parse_key_type(s: &str) -> Result<KeyType, String> {
    match s.to_lowercase().as_str() {
        "maintainer" => Ok(KeyType::Maintainer),
        "economic_node" => Ok(KeyType::EconomicNode),
        "emergency" => Ok(KeyType::Emergency),
        "github_app" => Ok(KeyType::GitHubApp),
        "system" => Ok(KeyType::System),
        _ => Err(format!("Invalid key type: {}", s)),
    }
}

fn parse_key_status(s: &str) -> Result<KeyStatus, String> {
    match s.to_lowercase().as_str() {
        "active" => Ok(KeyStatus::Active),
        "pending" => Ok(KeyStatus::Pending),
        "revoked" => Ok(KeyStatus::Revoked),
        "expired" => Ok(KeyStatus::Expired),
        "compromised" => Ok(KeyStatus::Compromised),
        _ => Err(format!("Invalid key status: {}", s)),
    }
}

fn parse_metadata(metadata: Vec<String>) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();

    for item in metadata {
        let parts: Vec<&str> = item.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid metadata format: {}", item));
        }
        map.insert(parts[0].to_string(), parts[1].to_string());
    }

    Ok(map)
}

fn print_keys(keys: &[governance_app::crypto::key_management::KeyMetadata]) {
    if keys.is_empty() {
        println!("No keys found");
        return;
    }

    println!(
        "{:<20} {:<15} {:<20} {:<10} {:<20}",
        "Key ID", "Type", "Owner", "Status", "Expires"
    );
    println!("{}", "-".repeat(85));

    for key in keys {
        println!(
            "{:<20} {:<15} {:<20} {:<10} {:<20}",
            key.key_id,
            format!("{:?}", key.key_type),
            key.owner,
            format!("{:?}", key.status),
            key.expires_at.format("%Y-%m-%d %H:%M:%S")
        );
    }
}

fn print_key_details(key: &governance_app::crypto::key_management::KeyMetadata) {
    println!("Key Details:");
    println!("  Key ID: {}", key.key_id);
    println!("  Type: {:?}", key.key_type);
    println!("  Owner: {}", key.owner);
    println!("  Public Key: {}", key.public_key);
    println!("  Status: {:?}", key.status);
    println!("  Created: {}", key.created_at);
    println!("  Expires: {}", key.expires_at);
    println!(
        "  Last Used: {}",
        key.last_used
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Never".to_string())
    );
    println!("  Usage Count: {}", key.usage_count);

    if !key.metadata.is_empty() {
        println!("  Metadata:");
        for (k, v) in &key.metadata {
            println!("    {}: {}", k, v);
        }
    }
}




