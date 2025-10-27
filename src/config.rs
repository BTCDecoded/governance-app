use serde::{Deserialize, Serialize};
use std::env;

pub mod loader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub github_app_id: u64,
    pub github_private_key_path: String,
    pub github_webhook_secret: String,
    pub governance_repo: String,
    pub server_host: String,
    pub server_port: u16,
    pub dry_run_mode: bool,
    pub log_enforcement_decisions: bool,
    pub enforcement_log_path: Option<String>,
    pub server_id: String,
    pub nostr: NostrConfig,
    pub ots: OtsConfig,
    pub audit: AuditConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrConfig {
    pub enabled: bool,
    pub server_nsec_path: String,
    pub relays: Vec<String>,
    pub publish_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtsConfig {
    pub enabled: bool,
    pub aggregator_url: String,
    pub monthly_anchor_day: u8,
    pub registry_path: String,
    pub proofs_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub log_path: String,
    pub rotation_interval_days: u32,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://governance.db".to_string());

        let github_app_id = env::var("GITHUB_APP_ID")
            .unwrap_or_else(|_| "123456".to_string())
            .parse()?;

        let github_private_key_path = env::var("GITHUB_PRIVATE_KEY_PATH")
            .unwrap_or_else(|_| "/path/to/private-key.pem".to_string());

        let github_webhook_secret = env::var("GITHUB_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "your_webhook_secret_here".to_string());

        let governance_repo =
            env::var("GOVERNANCE_REPO").unwrap_or_else(|_| "BTCDecoded/governance".to_string());

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        let dry_run_mode = env::var("DRY_RUN_MODE")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let log_enforcement_decisions = env::var("LOG_ENFORCEMENT_DECISIONS")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        let enforcement_log_path = env::var("ENFORCEMENT_LOG_PATH").ok();

        let server_id = env::var("SERVER_ID")
            .unwrap_or_else(|_| "governance-01".to_string());

        let nostr_enabled = env::var("NOSTR_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let nostr_server_nsec_path = env::var("NOSTR_SERVER_NSEC_PATH")
            .unwrap_or_else(|_| "/etc/governance/server.nsec".to_string());

        let nostr_relays = env::var("NOSTR_RELAYS")
            .unwrap_or_else(|_| "wss://relay.damus.io,wss://nos.lol".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let nostr_publish_interval = env::var("NOSTR_PUBLISH_INTERVAL_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .unwrap_or(3600);

        let ots_enabled = env::var("OTS_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let ots_aggregator_url = env::var("OTS_AGGREGATOR_URL")
            .unwrap_or_else(|_| "https://alice.btc.calendar.opentimestamps.org".to_string());

        let ots_monthly_anchor_day = env::var("OTS_MONTHLY_ANCHOR_DAY")
            .unwrap_or_else(|_| "1".to_string())
            .parse()
            .unwrap_or(1);

        let ots_registry_path = env::var("OTS_REGISTRY_PATH")
            .unwrap_or_else(|_| "/var/lib/governance/registries".to_string());

        let ots_proofs_path = env::var("OTS_PROOFS_PATH")
            .unwrap_or_else(|_| "/var/lib/governance/ots-proofs".to_string());

        let audit_enabled = env::var("AUDIT_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);

        let audit_log_path = env::var("AUDIT_LOG_PATH")
            .unwrap_or_else(|_| "/var/lib/governance/audit-log.jsonl".to_string());

        let audit_rotation_interval = env::var("AUDIT_ROTATION_INTERVAL_DAYS")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .unwrap_or(30);

        Ok(AppConfig {
            database_url,
            github_app_id,
            github_private_key_path,
            github_webhook_secret,
            governance_repo,
            server_host,
            server_port,
            dry_run_mode,
            log_enforcement_decisions,
            enforcement_log_path,
            server_id,
            nostr: NostrConfig {
                enabled: nostr_enabled,
                server_nsec_path: nostr_server_nsec_path,
                relays: nostr_relays,
                publish_interval_secs: nostr_publish_interval,
            },
            ots: OtsConfig {
                enabled: ots_enabled,
                aggregator_url: ots_aggregator_url,
                monthly_anchor_day: ots_monthly_anchor_day,
                registry_path: ots_registry_path,
                proofs_path: ots_proofs_path,
            },
            audit: AuditConfig {
                enabled: audit_enabled,
                log_path: audit_log_path,
                rotation_interval_days: audit_rotation_interval,
            },
        })
    }
}
