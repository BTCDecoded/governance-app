//! OpenTimestamps Verification Utilities
//!
//! Provides public utilities for verifying OTS proofs and registries.

use anyhow::{anyhow, Result};
use sha2::Digest;
use std::fs;
use tracing::{debug, info};

use crate::ots::anchor::GovernanceRegistry;
use crate::ots::client::{OtsClient, VerificationResult};

/// Verify a governance registry against its OTS proof
pub async fn verify_registry(
    registry_path: &str,
    proof_path: &str,
) -> Result<VerificationResult> {
    info!("Verifying registry: {} with proof: {}", registry_path, proof_path);

    // Load registry data
    let registry_data = fs::read(registry_path)
        .map_err(|e| anyhow!("Failed to read registry file: {}", e))?;

    // Load OTS proof
    let proof_data = fs::read(proof_path)
        .map_err(|e| anyhow!("Failed to read proof file: {}", e))?;

    // Create OTS client
    let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());

    // Verify timestamp
    let result = ots_client.verify(&registry_data, &proof_data).await?;

    match result {
        VerificationResult::Pending => {
            info!("Registry timestamp is pending confirmation");
        }
        VerificationResult::Confirmed(block_height) => {
            info!("Registry timestamp confirmed at Bitcoin block height: {}", block_height);
        }
    }

    Ok(result)
}

/// Verify registry JSON structure
pub fn verify_registry_structure(registry_path: &str) -> Result<GovernanceRegistry> {
    debug!("Verifying registry structure: {}", registry_path);

    let content = fs::read_to_string(registry_path)
        .map_err(|e| anyhow!("Failed to read registry file: {}", e))?;

    let registry: GovernanceRegistry = serde_json::from_str(&content)
        .map_err(|e| anyhow!("Failed to parse registry JSON: {}", e))?;

    // Validate required fields
    if registry.version.is_empty() {
        return Err(anyhow!("Registry version is empty"));
    }

    if registry.maintainers.is_empty() {
        return Err(anyhow!("Registry has no maintainers"));
    }

    if registry.multisig_config.required_signatures == 0 {
        return Err(anyhow!("Invalid multisig configuration"));
    }

    info!("Registry structure is valid");
    Ok(registry)
}

/// Verify OTS proof file format
pub fn verify_proof_format(proof_path: &str) -> Result<()> {
    debug!("Verifying OTS proof format: {}", proof_path);

    let proof_data = fs::read(proof_path)
        .map_err(|e| anyhow!("Failed to read proof file: {}", e))?;

    // For now, just check if it's a mock proof
    if proof_data.starts_with(b"MOCK_OTS_PROOF:") {
        info!("OTS proof format is valid (mock)");
        Ok(())
    } else {
        Err(anyhow!("Invalid OTS proof format"))
    }
}

/// Get Bitcoin block height from confirmed proof
pub async fn get_bitcoin_block_height(proof_path: &str) -> Result<Option<u32>> {
    debug!("Getting Bitcoin block height from proof: {}", proof_path);

    let proof_data = fs::read(proof_path)
        .map_err(|e| anyhow!("Failed to read proof file: {}", e))?;

    // For mock proofs, return a mock block height
    if proof_data.starts_with(b"MOCK_OTS_PROOF:") {
        Ok(Some(12345)) // Mock block height
    } else {
        Ok(None)
    }
}

/// Verify complete registry chain
pub async fn verify_registry_chain(registry_dir: &str) -> Result<Vec<String>> {
    info!("Verifying complete registry chain in: {}", registry_dir);

    let mut verified_registries = Vec::new();
    let mut previous_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();

    // Find all registry files
    let entries = fs::read_dir(registry_dir)
        .map_err(|e| anyhow!("Failed to read registry directory: {}", e))?;

    let mut registry_files: Vec<_> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension()? == "json" {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    // Sort by filename (which should be YYYY-MM.json)
    registry_files.sort();

    for registry_file in registry_files {
        let registry = verify_registry_structure(registry_file.to_str().unwrap())?;

        // Verify hash chain
        if registry.previous_registry_hash != previous_hash {
            return Err(anyhow!(
                "Hash chain broken at {}: expected {}, got {}",
                registry.version,
                previous_hash,
                registry.previous_registry_hash
            ));
        }

        // Verify OTS proof if it exists
        let proof_file = registry_file.with_extension("json.ots");
        if proof_file.exists() {
            let result = verify_registry(
                registry_file.to_str().unwrap(),
                proof_file.to_str().unwrap(),
            ).await?;

            if !result.is_confirmed() {
                return Err(anyhow!("Registry {} is not confirmed", registry.version));
            }
        }

        verified_registries.push(registry.version.clone());
        previous_hash = format!("sha256:{}", hex::encode(sha2::Sha256::digest(
            fs::read(registry_file).unwrap()
        )));

        info!("Verified registry: {}", registry.version);
    }

    info!("Verified {} registries in chain", verified_registries.len());
    Ok(verified_registries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[tokio::test]
    async fn test_verify_registry_structure() {
        let temp_dir = tempdir().unwrap();
        let registry_file = temp_dir.path().join("test.json");

        // Create a valid registry JSON
        let registry = GovernanceRegistry {
            version: "2025-01".to_string(),
            timestamp: chrono::Utc::now(),
            previous_registry_hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            maintainers: vec![crate::ots::anchor::Maintainer {
                id: 1,
                name: "Test Maintainer".to_string(),
                npub: "npub1test".to_string(),
                added_at: chrono::Utc::now(),
                status: "active".to_string(),
            }],
            authorized_servers: vec![],
            audit_logs: std::collections::HashMap::new(),
            multisig_config: crate::ots::anchor::MultisigConfig {
                required_signatures: 3,
                total_maintainers: 5,
            },
        };

        let json = serde_json::to_string_pretty(&registry).unwrap();
        fs::write(&registry_file, json).unwrap();

        let result = verify_registry_structure(registry_file.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().version, "2025-01");
    }

    #[test]
    fn test_verify_proof_format() {
        let temp_dir = tempdir().unwrap();
        let proof_file = temp_dir.path().join("test.ots");

        // Create a minimal proof file (this would fail in real usage)
        fs::write(&proof_file, b"invalid proof").unwrap();

        let result = verify_proof_format(proof_file.to_str().unwrap());
        assert!(result.is_err());
    }
}
