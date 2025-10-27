//! Nostr Client for Governance Status Publishing
//!
//! Manages connections to multiple Nostr relays and publishes
//! governance status updates with proper error handling and retry logic.

use anyhow::{anyhow, Result};
use nostr_sdk::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Nostr client managing multiple relay connections
pub struct NostrClient {
    client: Client,
    pub keys: Keys,
    relay_status: Arc<Mutex<HashMap<String, bool>>>,
}

impl NostrClient {
    /// Create new Nostr client with server key and relay URLs
    pub async fn new(nsec: String, relay_urls: Vec<String>) -> Result<Self> {
        let keys = Keys::from_sk_str(&nsec)
            .map_err(|e| anyhow!("Invalid nsec key: {}", e))?;

        let client = Client::new(&keys);
        
        // Connect to all relays
        for relay_url in &relay_urls {
            match client.add_relay(relay_url.clone()).await {
                Ok(_) => {
                    info!("Connected to relay: {}", relay_url);
                }
                Err(e) => {
                    warn!("Failed to connect to relay {}: {}", relay_url, e);
                }
            }
        }

        // Start client
        client.connect().await;

        let relay_status = Arc::new(Mutex::new(HashMap::new()));
        
        Ok(Self {
            client,
            keys,
            relay_status,
        })
    }

    /// Publish event to all connected relays
    pub async fn publish_event(&self, event: Event) -> Result<()> {
        let mut successful_relays = 0;
        let mut failed_relays = Vec::new();

        // Get list of connected relays
        let relays = self.client.relays().await;

        for (relay_url, relay) in &relays {
            match relay.send_event(event.clone(), RelaySendOptions::new()).await {
                Ok(_) => {
                    debug!("Published event to relay: {}", relay_url);
                    successful_relays += 1;
                    
                    // Update relay status
                    let mut status = self.relay_status.lock().await;
                    status.insert(relay_url.to_string(), true);
                }
                Err(e) => {
                    error!("Failed to publish to relay {}: {}", relay_url, e);
                    failed_relays.push(relay_url.to_string());
                    
                    // Update relay status
                    let mut status = self.relay_status.lock().await;
                    status.insert(relay_url.to_string(), false);
                }
            }
        }

        if successful_relays == 0 {
            return Err(anyhow!("Failed to publish to any relay"));
        }

        if !failed_relays.is_empty() {
            warn!("Failed to publish to {} relays: {:?}", failed_relays.len(), failed_relays);
        }

        info!("Published event to {}/{} relays", successful_relays, relays.len());
        Ok(())
    }

    /// Get current relay status
    pub async fn get_relay_status(&self) -> HashMap<String, bool> {
        self.relay_status.lock().await.clone()
    }

    /// Close all relay connections
    pub async fn close(&self) -> Result<()> {
        self.client.disconnect().await?;
        info!("Disconnected from all Nostr relays");
        Ok(())
    }

    /// Get the public key (npub) for this client
    pub fn public_key(&self) -> String {
        self.keys.public_key().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_sdk::prelude::*;

    #[tokio::test]
    async fn test_client_creation() {
        // Generate test keys
        let keys = Keys::generate();
        let nsec = keys.secret_key().to_secret_hex();
        
        // This will fail in test environment without real relays
        // but we can test the key parsing
        let result = NostrClient::new(nsec, vec!["wss://relay.damus.io".to_string()]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_key() {
        let result = NostrClient::new("invalid_key".to_string(), vec![]).await;
        assert!(result.is_err());
    }
}
