//! Nostr Bot Manager
//!
//! Manages multiple bot identities for different purposes:
//! - gov: Governance announcements
//! - dev: Development updates
//! - research: Educational content (optional)
//! - network: Network metrics (optional)

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::config::{BotConfig, NostrConfig};
use crate::nostr::client::NostrClient;

/// Manages multiple Nostr bot identities
pub struct NostrBotManager {
    bots: HashMap<String, NostrClient>,
    config: NostrConfig,
    bot_configs: HashMap<String, BotConfig>,
}

impl NostrBotManager {
    /// Create a new bot manager from config
    pub async fn new(config: NostrConfig) -> Result<Self> {
        let mut bots = HashMap::new();
        let mut bot_configs = HashMap::new();

        // Load each bot configuration
        for (bot_id, bot_config) in &config.bots {
            info!("Initializing Nostr bot: {}", bot_id);

            // Resolve nsec path (supports "env:VAR_NAME" for GitHub secrets)
            let nsec = Self::resolve_nsec(&bot_config.nsec_path)?;

            // Create Nostr client for this bot
            let client = NostrClient::new(nsec, config.relays.clone())
                .await
                .map_err(|e| anyhow!("Failed to create Nostr client for bot {}: {}", bot_id, e))?;

            bots.insert(bot_id.clone(), client);
            bot_configs.insert(bot_id.clone(), bot_config.clone());
        }

        // If no bots configured but legacy config exists, create default "gov" bot
        if bots.is_empty() && !config.server_nsec_path.is_empty() {
            warn!("No bots configured, using legacy single-bot mode");
            let nsec = Self::resolve_nsec(&config.server_nsec_path)?;
            let client = NostrClient::new(nsec, config.relays.clone()).await?;
            bots.insert("gov".to_string(), client);
        }

        Ok(Self {
            bots,
            config,
            bot_configs,
        })
    }

    /// Resolve nsec path - supports file paths and "env:VAR_NAME" format
    fn resolve_nsec(nsec_path: &str) -> Result<String> {
        if nsec_path.starts_with("env:") {
            // Extract environment variable name
            let var_name = nsec_path.strip_prefix("env:").unwrap();
            std::env::var(var_name)
                .map_err(|e| anyhow!("Failed to read environment variable {}: {}", var_name, e))
        } else {
            // Read from file
            std::fs::read_to_string(nsec_path)
                .map_err(|e| anyhow!("Failed to read nsec file {}: {}", nsec_path, e))
        }
    }

    /// Get a bot client by ID
    pub fn get_bot(&self, bot_id: &str) -> Result<&NostrClient> {
        self.bots.get(bot_id).ok_or_else(|| {
            anyhow!(
                "Bot '{}' not found. Available bots: {:?}",
                bot_id,
                self.bots.keys().collect::<Vec<_>>()
            )
        })
    }

    /// Get bot config by ID
    pub fn get_bot_config(&self, bot_id: &str) -> Option<&BotConfig> {
        self.bot_configs.get(bot_id)
    }

    /// Get the governance bot (defaults to "gov")
    pub fn get_gov_bot(&self) -> Result<&NostrClient> {
        self.get_bot("gov")
    }

    /// Get the development bot (defaults to "dev" or "gov" if not available)
    pub fn get_dev_bot(&self) -> Result<&NostrClient> {
        self.get_bot("dev").or_else(|_| self.get_bot("gov"))
    }

    /// Get the research bot (defaults to "dev" or "gov" if not available)
    pub fn get_research_bot(&self) -> Result<&NostrClient> {
        self.get_bot("research")
            .or_else(|_| self.get_bot("dev"))
            .or_else(|_| self.get_bot("gov"))
    }

    /// Get the network bot (defaults to "dev" or "gov" if not available)
    pub fn get_network_bot(&self) -> Result<&NostrClient> {
        self.get_bot("network")
            .or_else(|_| self.get_bot("dev"))
            .or_else(|_| self.get_bot("gov"))
    }

    /// Get all bot IDs
    pub fn bot_ids(&self) -> Vec<String> {
        self.bots.keys().cloned().collect()
    }

    /// Get the governance config name
    pub fn governance_config(&self) -> &str {
        &self.config.governance_config
    }

    /// Get the lightning address for a bot
    pub fn get_lightning_address(&self, bot_id: &str) -> Option<String> {
        self.bot_configs
            .get(bot_id)
            .map(|c| c.lightning_address.clone())
    }

    /// Close all bot connections
    pub async fn close_all(&self) -> Result<()> {
        for (bot_id, client) in &self.bots {
            if let Err(e) = client.close().await {
                warn!("Failed to close bot {}: {}", bot_id, e);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bot_manager_creation() {
        // This test would require actual Nostr keys
        // For now, just verify the structure compiles
    }

    #[test]
    fn test_resolve_nsec_from_env() {
        std::env::set_var("TEST_NSEC_VAR", "test_nsec_value");

        let result = NostrBotManager::resolve_nsec("env:TEST_NSEC_VAR");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_nsec_value");

        std::env::remove_var("TEST_NSEC_VAR");
    }

    #[test]
    fn test_resolve_nsec_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "test_nsec_from_file").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let result = NostrBotManager::resolve_nsec(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "test_nsec_from_file");
    }

    #[test]
    fn test_resolve_nsec_invalid_env() {
        let result = NostrBotManager::resolve_nsec("env:NONEXISTENT_VAR");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_nsec_invalid_file() {
        let result = NostrBotManager::resolve_nsec("/nonexistent/path/nsec");
        assert!(result.is_err());
    }
}
