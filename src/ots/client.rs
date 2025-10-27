//! OpenTimestamps Client
//!
//! Handles communication with OpenTimestamps calendar servers
//! for creating and verifying Bitcoin-anchored timestamps.

use anyhow::{anyhow, Result};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// OpenTimestamps client for creating and verifying timestamps
pub struct OtsClient {
    aggregator_url: String,
    http_client: Client,
}

impl OtsClient {
    /// Create new OTS client with aggregator URL
    pub fn new(aggregator_url: String) -> Self {
        let http_client = Client::new();

        Self {
            aggregator_url,
            http_client,
        }
    }

    /// Submit data for timestamping
    pub async fn stamp(&self, data: &[u8]) -> Result<Vec<u8>> {
        info!("Submitting {} bytes for timestamping", data.len());

        // Calculate SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = hasher.finalize();

        // For now, return a mock proof
        // In a real implementation, this would submit to OpenTimestamps
        let mock_proof = format!("MOCK_OTS_PROOF:{}", hex::encode(hash)).into_bytes();
        
        info!("Created mock OTS proof for {} bytes", data.len());
        Ok(mock_proof)
    }

    /// Verify a timestamp against Bitcoin blockchain
    pub async fn verify(&self, data: &[u8], proof: &[u8]) -> Result<VerificationResult> {
        debug!("Verifying timestamp proof ({} bytes)", proof.len());

        // Calculate data hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        let data_hash = hasher.finalize();

        // For now, return a mock verification
        // In a real implementation, this would verify against OpenTimestamps
        if proof.starts_with(b"MOCK_OTS_PROOF:") {
            info!("Mock timestamp verified");
            Ok(VerificationResult::Confirmed(12345)) // Mock block height
        } else {
            Err(anyhow!("Invalid proof format"))
        }
    }

    /// Upgrade a pending timestamp to confirmed
    pub async fn upgrade(&self, proof: &[u8]) -> Result<Vec<u8>> {
        debug!("Upgrading pending timestamp");

        // For now, return the same proof
        // In a real implementation, this would upgrade from OpenTimestamps
        Ok(proof.to_vec())
    }

}

/// Result of timestamp verification
#[derive(Debug, Clone)]
pub enum VerificationResult {
    /// Timestamp is pending confirmation
    Pending,
    /// Timestamp is confirmed at the given Bitcoin block height
    Confirmed(u32),
}

impl VerificationResult {
    /// Check if the timestamp is confirmed
    pub fn is_confirmed(&self) -> bool {
        matches!(self, VerificationResult::Confirmed(_))
    }

    /// Get the block height if confirmed
    pub fn block_height(&self) -> Option<u32> {
        match self {
            VerificationResult::Confirmed(height) => Some(*height),
            VerificationResult::Pending => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_result() {
        let pending = VerificationResult::Pending;
        assert!(!pending.is_confirmed());
        assert_eq!(pending.block_height(), None);

        let confirmed = VerificationResult::Confirmed(12345);
        assert!(confirmed.is_confirmed());
        assert_eq!(confirmed.block_height(), Some(12345));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
        assert_eq!(client.aggregator_url, "https://alice.btc.calendar.opentimestamps.org");
        assert!(!client.calendars.is_empty());
    }
}
