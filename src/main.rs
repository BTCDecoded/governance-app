use axum::{
    Router,
    extract::State,
    response::Json,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::Duration;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod audit;
mod authorization;
mod backup;
mod build;
mod config;
mod crypto;
mod database;
mod enforcement;
mod error;
mod github;
mod governance;
mod governance_review;
mod node_registry;
mod nostr;
#[cfg(feature = "opentimestamps")]
mod ots;
mod resilience;
mod validation;
mod webhooks;

use audit::AuditLogger;
use config::AppConfig;
use database::Database;
use governance::ContributionAggregator;
use nostr::{NostrClient, StatusPublisher, ZapTracker};
#[cfg(feature = "opentimestamps")]
use ots::{OtsClient, RegistryAnchorer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "blvm_commons=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Bitcoin Commons (blvm-commons)");

    // Load configuration
    let config = AppConfig::load()?;
    info!("Configuration loaded");

    // Initialize database
    let database = Database::new(&config.database_url).await?;
    info!("Database connected");

    // Run migrations
    database.run_migrations().await?;
    info!("Database migrations completed");

    // Start automated backup task
    let database_for_backup = database.clone();
    let backup_config = backup::BackupConfig {
        directory: std::path::PathBuf::from("/opt/blvm-commons/backups"),
        retention_days: 30,
        compression: true,
        interval: std::time::Duration::from_secs(86400), // Daily
        enabled: true,
    };
    let backup_manager = Arc::new(backup::BackupManager::new(
        database_for_backup,
        backup_config,
    ));
    backup_manager.clone().start_backup_task();
    info!("Automated backup task started");

    // Start database health monitoring task with reconnection capability
    let database_for_health = database.clone();
    let database_url_for_reconnect = config.database_url.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every 60 seconds
        let mut consecutive_failures = 0u32;
        let mut current_db = database_for_health;

        loop {
            interval.tick().await;

            // Check database health
            match current_db.check_health().await {
                Ok(true) => {
                    if consecutive_failures > 0 {
                        info!(
                            "Database health check passed after {} failures",
                            consecutive_failures
                        );
                        consecutive_failures = 0;
                    }

                    // Log pool stats periodically (every 10 checks = 10 minutes)
                    if consecutive_failures == 0 {
                        if let Ok(stats) = current_db.get_pool_stats().await {
                            debug!(
                                "Database pool stats: size={}, idle={}, closed={}",
                                stats.size, stats.idle, stats.is_closed
                            );
                        }
                    }
                }
                Ok(false) | Err(_) => {
                    consecutive_failures += 1;
                    warn!(
                        "Database health check failed (consecutive failures: {})",
                        consecutive_failures
                    );

                    // After 3 consecutive failures, attempt reconnection
                    if consecutive_failures >= 3 {
                        error!(
                            "Database connection unhealthy after {} consecutive failures - attempting reconnection",
                            consecutive_failures
                        );

                        // Check if pool is closed before attempting reconnection
                        let should_reconnect = current_db
                            .get_pool_stats()
                            .await
                            .map(|stats| stats.is_closed)
                            .unwrap_or(true);

                        if should_reconnect {
                            // Attempt to reconnect using stored database URL
                            match Database::new(&database_url_for_reconnect).await {
                                Ok(new_db) => {
                                    info!("Database reconnection successful");
                                    current_db = new_db;
                                    consecutive_failures = 0;
                                }
                                Err(e) => {
                                    error!(
                                        "Database reconnection failed: {} - will retry on next health check",
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // Initialize audit logger
    let audit_logger = if config.audit.enabled {
        let logger = AuditLogger::new(config.audit.log_path.clone())?;
        logger.load_existing_entries().await?;
        Some(logger)
    } else {
        None
    };
    info!("Audit logger initialized");

    // Initialize Nostr client and status publisher
    let nostr_client = if config.nostr.enabled {
        let nsec = std::fs::read_to_string(&config.nostr.server_nsec_path)
            .map_err(|e| format!("Failed to read Nostr key: {e}"))?;

        let client = NostrClient::new(nsec, config.nostr.relays.clone())
            .await
            .map_err(|e| format!("Failed to create Nostr client: {e}"))?;

        Some(client)
    } else {
        None
    };

    let status_publisher = if let Some(ref client) = nostr_client {
        Some(StatusPublisher::new(
            client.clone(),
            database.clone(),
            config.server_id.clone(),
            std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "blvm-commons".to_string()),
            "config.toml".to_string(),
            if config.audit.enabled {
                Some(config.audit.log_path.clone())
            } else {
                None
            },
        ))
    } else {
        None
    };

    // Initialize OTS client and registry anchorer (only if feature enabled)
    #[cfg(feature = "opentimestamps")]
    let ots_client = if config.ots.enabled {
        Some(OtsClient::new(config.ots.aggregator_url.clone()))
    } else {
        None
    };
    #[cfg(not(feature = "opentimestamps"))]
    let ots_client: Option<()> = None;

    #[cfg(feature = "opentimestamps")]
    let registry_anchorer = if let Some(client) = ots_client {
        Some(RegistryAnchorer::new(
            client,
            database.clone(),
            config.ots.registry_path.clone(),
            config.ots.proofs_path.clone(),
        ))
    } else {
        None
    };
    #[cfg(not(feature = "opentimestamps"))]
    let registry_anchorer: Option<()> = None;

    // Start background tasks
    let config_clone = config.clone();
    let database_clone = database.clone();
    let audit_logger_for_rotation = audit_logger.clone();

    // Nostr status publisher task
    if let Some(publisher) = status_publisher {
        let publish_interval = Duration::from_secs(config.nostr.publish_interval_secs);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(publish_interval);
            loop {
                interval.tick().await;
                if let Err(e) = publisher.publish_status().await {
                    error!("Failed to publish Nostr status: {}", e);
                }
            }
        });
        info!("Nostr status publisher started");
    }

    // OTS monthly anchoring task (only if feature enabled)
    #[cfg(feature = "opentimestamps")]
    if let Some(anchorer) = registry_anchorer {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(86400)); // Check daily
            loop {
                interval.tick().await;
                let now = chrono::Utc::now();
                if now.day() == config_clone.ots.monthly_anchor_day as u32 {
                    if let Err(e) = anchorer.anchor_registry().await {
                        error!("Failed to anchor registry: {}", e);
                    }
                }
            }
        });
        info!("OTS registry anchorer started");
    }

    // Audit log rotation task
    if let Some(ref logger) = audit_logger_for_rotation {
        let logger = logger.clone();
        let server_id = config.server_id.clone();
        let rotation_interval =
            Duration::from_secs(config.audit.rotation_interval_days as u64 * 86400);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(rotation_interval);
            interval.tick().await; // skip immediate first tick
            loop {
                interval.tick().await;
                if let Err(e) = logger.rotate(&server_id).await {
                    error!("Audit log rotation failed: {}", e);
                }
            }
        });
        info!(
            "Audit log rotation started (interval: {} days)",
            config.audit.rotation_interval_days
        );
    }

    // Initialize governance services
    let pool = database
        .get_sqlite_pool()
        .ok_or_else(|| "Database pool not available".to_string())?;

    // Start zap tracker if Nostr is enabled and governance tracking enabled
    if config.nostr.enabled && config.governance.contribution_tracking_enabled {
        if let Some(ref nostr_client) = nostr_client {
            // Collect all bot pubkeys for zap tracking
            let mut bot_pubkeys = Vec::new();
            if let Some(zap_addr) = &config.nostr.zap_address {
                // Legacy single bot - extract pubkey if available
                // For now, we'll need the pubkey from the nsec
                if let Ok(nsec) = std::fs::read_to_string(&config.nostr.server_nsec_path) {
                    // Parse nsec to get pubkey (simplified - in production use proper parsing)
                    // For now, we'll track the configured zap address
                    info!("Zap tracking configured for legacy zap address");
                }
            }

            // Add bot pubkeys from multi-bot config
            for (bot_id, bot_config) in &config.nostr.bots {
                bot_pubkeys.push(bot_config.npub.clone());
                info!(
                    "Zap tracking configured for bot: {} (npub: {})",
                    bot_id, bot_config.npub
                );
            }

            if !bot_pubkeys.is_empty() {
                let zap_tracker =
                    ZapTracker::new(pool.clone(), Arc::new(nostr_client.clone()), bot_pubkeys);
                if let Err(e) = zap_tracker.start_tracking().await {
                    error!("Failed to start zap tracking: {}", e);
                } else {
                    info!("Zap tracker started");
                }
            }
        }
    }

    // Fee forwarding removed - no longer tracked

    // Start periodic weight update task (if enabled)
    if config.governance.weight_updates_enabled {
        let pool_for_weights = pool.clone();
        let update_interval = Duration::from_secs(config.governance.weight_update_interval_secs);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            loop {
                interval.tick().await;
                info!("Starting periodic weight update");

                let aggregator = ContributionAggregator::new(pool_for_weights.clone());
                if let Err(e) = aggregator.update_all_weights().await {
                    error!("Failed to update participation weights: {}", e);
                } else {
                    info!("Periodic weight update completed");
                }
            }
        });
        info!(
            "Periodic weight update task started (interval: {}s)",
            config.governance.weight_update_interval_secs
        );
    }

    // Build application
    let port = config.server_port;
    // Add node registry API routes
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/webhooks/github", post(webhooks::github::handle_webhook))
        .route(
            "/webhooks/block",
            post(webhooks::block::handle_block_notification),
        )
        .route("/status", get(status_endpoint))
        .merge(node_registry::api::create_router())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state((config, database));

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "blvm-commons",
        "timestamp": chrono::Utc::now()
    }))
}

async fn status_endpoint(
    State((config, database)): State<(AppConfig, Database)>,
) -> Json<serde_json::Value> {
    let pool = database.get_sqlite_pool();
    let governance_status = if let Some(pool) = pool {
        // Check governance tables exist
        let tables_exist = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('unified_contributions', 'participation_weights', 'zap_contributions')"
        )
        .fetch_one(pool)
        .await
        .ok()
        .map(|count| count >= 3)
        .unwrap_or(false);

        // Get contributor count
        let contributor_count: i64 =
            sqlx::query_scalar("SELECT COUNT(DISTINCT contributor_id) FROM unified_contributions")
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        serde_json::json!({
            "enabled": config.governance.contribution_tracking_enabled,
            "tables_exist": tables_exist,
            "contributor_count": contributor_count,
            "weight_updates_enabled": config.governance.weight_updates_enabled,
            "commons_addresses_count": config.governance.commons_addresses.len(),
        })
    } else {
        serde_json::json!({
            "enabled": false,
            "error": "Database pool not available"
        })
    };

    let mut status = serde_json::json!({
        "status": "healthy",
        "service": "blvm-commons",
        "timestamp": chrono::Utc::now(),
        "server_id": config.server_id,
        "features": {
            "nostr": config.nostr.enabled,
            "ots": config.ots.enabled,
            "audit": config.audit.enabled,
            "dry_run": config.dry_run_mode,
            "governance": governance_status,
        }
    });

    // Add database status
    if let Ok(stats) = database.get_performance_stats().await {
        status["database"] = serde_json::json!({
            "status": "healthy",
            "cache_size": stats.cache_size,
            "slow_queries": stats.slow_queries_count
        });
    } else {
        status["database"] = serde_json::json!({
            "status": "error"
        });
    }

    Json(status)
}
