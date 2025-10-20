use governance_app::database::Database;
use governance_app::crypto::{SignatureManager, MultisigManager};
use secp256k1::{SecretKey, Secp256k1, PublicKey};
use rand::rngs::OsRng;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

/// Setup an in-memory SQLite database for testing
pub async fn setup_test_db() -> Database {
    Database::new_in_memory().await.expect("Failed to create test database")
}

/// Create a test signature manager
pub fn create_test_signature_manager() -> SignatureManager {
    SignatureManager::new()
}

/// Create a test multisig manager
pub fn create_test_multisig_manager() -> MultisigManager {
    MultisigManager::new()
}

/// Generate test keypairs for testing
pub fn generate_test_keypairs(count: usize) -> Vec<(String, SecretKey, PublicKey)> {
    let secp = Secp256k1::new();
    let mut keypairs = Vec::new();
    
    for i in 0..count {
        let secret_key = SecretKey::new(&mut OsRng);
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let username = format!("testuser{}", i);
        
        keypairs.push((username, secret_key, public_key));
    }
    
    keypairs
}

/// Create test maintainers data
pub fn create_test_maintainers() -> Vec<(String, String, i32)> {
    vec![
        ("alice".to_string(), "pubkey_alice".to_string(), 1),
        ("bob".to_string(), "pubkey_bob".to_string(), 1),
        ("charlie".to_string(), "pubkey_charlie".to_string(), 2),
        ("dave".to_string(), "pubkey_dave".to_string(), 2),
        ("eve".to_string(), "pubkey_eve".to_string(), 3),
    ]
}

/// Create test emergency keyholders
pub fn create_test_emergency_keyholders() -> Vec<(String, String)> {
    vec![
        ("emergency_alice".to_string(), "emergency_pubkey_alice".to_string()),
        ("emergency_bob".to_string(), "emergency_pubkey_bob".to_string()),
        ("emergency_charlie".to_string(), "emergency_pubkey_charlie".to_string()),
        ("emergency_dave".to_string(), "emergency_pubkey_dave".to_string()),
        ("emergency_eve".to_string(), "emergency_pubkey_eve".to_string()),
        ("emergency_frank".to_string(), "emergency_pubkey_frank".to_string()),
        ("emergency_grace".to_string(), "emergency_pubkey_grace".to_string()),
    ]
}

/// Create test pull request data
pub fn create_test_pull_request(
    repo_name: &str,
    pr_number: i32,
    layer: i32,
    opened_days_ago: i64,
) -> (String, i32, String, i32, DateTime<Utc>) {
    let opened_at = Utc::now() - Duration::days(opened_days_ago);
    let head_sha = format!("abc123def456{}", pr_number);
    
    (repo_name.to_string(), pr_number, head_sha, layer, opened_at)
}

/// Create test cross-layer rules
pub fn create_test_cross_layer_rules() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "source_repo": "BTCDecoded/consensus-proof",
            "source_pattern": "src/consensus/**",
            "target_repo": "BTCDecoded/protocol-engine",
            "target_pattern": "src/validation/**",
            "validation_type": "corresponding_file_exists"
        }),
        serde_json::json!({
            "source_repo": "BTCDecoded/protocol-engine",
            "source_pattern": "src/network/**",
            "target_repo": "BTCDecoded/reference-node",
            "target_pattern": "src/network/**",
            "validation_type": "references_latest_version"
        }),
    ]
}

/// Create test signatures for a pull request
pub fn create_test_signatures(signers: &[String]) -> Vec<serde_json::Value> {
    signers.iter().map(|signer| {
        serde_json::json!({
            "signer": signer,
            "signature": format!("signature_{}", signer),
            "timestamp": Utc::now()
        })
    }).collect()
}

/// Mock GitHub webhook payloads
pub mod github_mocks {
    use serde_json::Value;
    
    pub fn pull_request_opened_payload(repo: &str, pr_number: u64) -> Value {
        serde_json::json!({
            "action": "opened",
            "repository": {
                "full_name": repo
            },
            "pull_request": {
                "number": pr_number,
                "head": {
                    "sha": "abc123def456"
                }
            }
        })
    }
    
    pub fn pull_request_synchronize_payload(repo: &str, pr_number: u64) -> Value {
        serde_json::json!({
            "action": "synchronize",
            "repository": {
                "full_name": repo
            },
            "pull_request": {
                "number": pr_number,
                "head": {
                    "sha": "def456ghi789"
                }
            }
        })
    }
    
    pub fn review_submitted_payload(repo: &str, pr_number: u64, reviewer: &str, state: &str) -> Value {
        serde_json::json!({
            "action": "submitted",
            "repository": {
                "full_name": repo
            },
            "pull_request": {
                "number": pr_number
            },
            "review": {
                "user": {
                    "login": reviewer
                },
                "state": state
            }
        })
    }
    
    pub fn comment_created_payload(repo: &str, pr_number: u64, commenter: &str, body: &str) -> Value {
        serde_json::json!({
            "action": "created",
            "repository": {
                "full_name": repo
            },
            "issue": {
                "number": pr_number
            },
            "comment": {
                "user": {
                    "login": commenter
                },
                "body": body
            }
        })
    }
    
    pub fn push_payload(repo: &str, pusher: &str, ref_name: &str) -> Value {
        serde_json::json!({
            "repository": {
                "full_name": repo
            },
            "pusher": {
                "name": pusher
            },
            "ref": ref_name
        })
    }
}

/// Test data fixtures
pub mod fixtures {
    use super::*;
    
    pub async fn setup_test_database_with_data() -> Database {
        let db = setup_test_db().await;
        
        // Insert test maintainers
        let maintainers = create_test_maintainers();
        for (username, public_key, layer) in maintainers {
            // This would use actual database insertion in a real implementation
            // For now, we'll just return the database
        }
        
        // Insert test emergency keyholders
        let keyholders = create_test_emergency_keyholders();
        for (username, public_key) in keyholders {
            // This would use actual database insertion in a real implementation
        }
        
        db
    }
}
