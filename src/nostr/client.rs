//! Nostr Client for Governance Status Publishing
//!
//! Manages connections to multiple Nostr relays and publishes
//! governance status updates with proper error handling and retry logic.

use anyhow::{Result, anyhow};
use nostr_sdk::prelude::*;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Nostr client managing multiple relay connections
#[derive(Clone)]
pub struct NostrClient {
    client: Arc<Client>,
    pub keys: Keys,
    relay_status: Arc<Mutex<HashMap<String, bool>>>,
}

impl NostrClient {
    /// Create new Nostr client with server key and relay URLs
    pub async fn new(nsec: String, relay_urls: Vec<String>) -> Result<Self> {
        let keys = Keys::from_sk_str(&nsec).map_err(|e| anyhow!("Invalid nsec key: {}", e))?;

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
            client: Arc::new(client),
            keys,
            relay_status,
        })
    }

    /// Publish event to all connected relays
    pub async fn publish_event(&self, event: Event) -> Result<()> {
        let mut successful_relays = 0;
        let mut failed_relays = Vec::new();

        // Get list of connected relays
        let relays = (*self.client).relays().await;

        for (relay_url, relay) in &relays {
            match relay
                .send_event(event.clone(), RelaySendOptions::new())
                .await
            {
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
            warn!(
                "Failed to publish to {} relays: {:?}",
                failed_relays.len(),
                failed_relays
            );
        }

        info!(
            "Published event to {}/{} relays",
            successful_relays,
            relays.len()
        );
        Ok(())
    }

    /// Get current relay status
    pub async fn get_relay_status(&self) -> HashMap<String, bool> {
        self.relay_status.lock().await.clone()
    }

    /// Close all relay connections
    pub async fn close(&self) -> Result<()> {
        (*self.client).disconnect().await?;
        info!("Disconnected from all Nostr relays");
        Ok(())
    }

    /// Get the public key (npub) for this client
    pub fn public_key(&self) -> String {
        self.keys.public_key().to_string()
    }

    /// Subscribe to zap receipt events (NIP-57, kind 9735) for a recipient pubkey
    /// Returns a receiver channel that will receive zap events
    pub async fn subscribe_to_zaps(
        &self,
        recipient_pubkey: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<ZapEvent>> {
        use nostr_sdk::prelude::*;

        // Parse recipient pubkey
        let recipient_key = XOnlyPublicKey::from_str(recipient_pubkey)
            .map_err(|e| anyhow!("Invalid recipient pubkey: {}", e))?;

        // Create filter for zap receipts (kind 9735) to this pubkey
        let filter = Filter::new()
            .kind(Kind::ZapReceipt) // Kind 9735
            .pubkey(recipient_key);

        // Subscribe to events
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Get client handle for spawning task
        let client_handle = self.client.clone();

        // Spawn task to handle incoming zap events
        tokio::spawn(async move {
            // Subscribe to the filter (subscribe only takes filters, not subscription_id)
            client_handle.subscribe(vec![filter]).await;

            // Listen for events using get_events_of
            loop {
                // Create filter again for querying
                let query_filter = Filter::new().kind(Kind::ZapReceipt).pubkey(recipient_key);

                match (*client_handle)
                    .get_events_of(vec![query_filter], Some(Duration::from_secs(10)))
                    .await
                {
                    Ok(events) => {
                        for event in events {
                            if event.kind == Kind::ZapReceipt {
                                // Parse zap event
                                if let Ok(zap) = parse_zap_event(&event) {
                                    if let Err(_) = tx.send(zap).await {
                                        // Receiver dropped, stop processing
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error getting zap events: {}", e);
                    }
                }

                // Wait a bit before next query
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        info!("Subscribed to zap events for pubkey: {}", recipient_pubkey);
        Ok(rx)
    }
}

/// Parsed zap event from Nostr (NIP-57)
#[derive(Debug, Clone)]
pub struct ZapEvent {
    pub recipient_pubkey: String,
    pub sender_pubkey: Option<String>,
    pub amount_msat: u64,
    pub timestamp: i64,
    pub invoice: Option<String>,
    pub message: Option<String>,
    pub zapped_event_id: Option<String>, // Event being zapped (for proposal zaps)
}

/// Parse a Nostr event into a ZapEvent
fn parse_zap_event(event: &nostr_sdk::prelude::Event) -> Result<ZapEvent> {
    // Extract recipient (p tag)
    let recipient = event
        .tags
        .iter()
        .find(|tag| {
            let vec = tag.as_vec();
            vec.first().map(|s| s.as_str()) == Some("p")
        })
        .and_then(|tag| {
            let vec = tag.as_vec();
            vec.get(1).map(|s| s.to_string())
        })
        .ok_or_else(|| anyhow!("Missing p tag in zap event"))?;

    // Extract amount (amount tag)
    let amount_msat = event
        .tags
        .iter()
        .find(|tag| {
            let vec = tag.as_vec();
            vec.first().map(|s| s.as_str()) == Some("amount")
        })
        .and_then(|tag| {
            let vec = tag.as_vec();
            vec.get(1).and_then(|amt| amt.parse::<u64>().ok())
        })
        .unwrap_or(0);

    // Extract invoice (bolt11 tag)
    let invoice = event
        .tags
        .iter()
        .find(|tag| {
            let vec = tag.as_vec();
            vec.first().map(|s| s.as_str()) == Some("bolt11")
        })
        .and_then(|tag| {
            let vec = tag.as_vec();
            vec.get(1).map(|s| s.to_string())
        });

    // Extract description (contains sender info as JSON)
    let (sender_pubkey, message) = event
        .tags
        .iter()
        .find(|tag| {
            let vec = tag.as_vec();
            vec.first().map(|s| s.as_str()) == Some("description")
        })
        .and_then(|tag| {
            let vec = tag.as_vec();
            vec.get(1)
                .and_then(|desc| serde_json::from_str::<serde_json::Value>(desc).ok())
        })
        .map(|desc| {
            let sender = desc
                .get("pubkey")
                .and_then(|p| p.as_str())
                .map(|s| s.to_string());
            let msg = desc
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            (sender, msg)
        })
        .unwrap_or((None, None));

    // Extract zapped event (e tag) - for proposal zaps
    let zapped_event_id = event
        .tags
        .iter()
        .find(|tag| {
            let vec = tag.as_vec();
            vec.first().map(|s| s.as_str()) == Some("e")
        })
        .and_then(|tag| {
            let vec = tag.as_vec();
            vec.get(1).map(|s| s.to_string())
        });

    Ok(ZapEvent {
        recipient_pubkey: recipient,
        sender_pubkey,
        amount_msat,
        timestamp: event.created_at.as_i64(),
        invoice,
        message,
        zapped_event_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        // Generate test keys
        let keys = Keys::generate();
        let nsec = keys.secret_key().unwrap().display_secret().to_string();

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
