use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Datelike;
use std::net::SocketAddr;
use tokio::time::Duration;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod crypto;
mod database;
mod enforcement;
mod error;
mod github;
mod validation;
mod webhooks;
mod nostr;
mod ots;
mod audit;
mod authorization;

use config::AppConfig;
use database::Database;
use nostr::{NostrClient, StatusPublisher};
use ots::{OtsClient, RegistryAnchorer};
use audit::AuditLogger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "governance_app=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting BTCDecoded Governance App");

    // Load configuration
    let config = AppConfig::load()?;
    info!("Configuration loaded");

    // Initialize database
    let database = Database::new(&config.database_url).await?;
    info!("Database connected");

    // Run migrations
    database.run_migrations().await?;
    info!("Database migrations completed");

    // Initialize audit logger
    let mut audit_logger = if config.audit.enabled {
        Some(AuditLogger::new(config.audit.log_path.clone())?)
    } else {
        None
    };
    info!("Audit logger initialized");

    // Initialize Nostr client and status publisher
    let nostr_client = if config.nostr.enabled {
        let nsec = std::fs::read_to_string(&config.nostr.server_nsec_path)
            .map_err(|e| format!("Failed to read Nostr key: {}", e))?;
        
        let client = NostrClient::new(nsec, config.nostr.relays.clone()).await
            .map_err(|e| format!("Failed to create Nostr client: {}", e))?;
        
        Some(client)
    } else {
        None
    };

    let status_publisher = if let Some(client) = nostr_client {
        Some(StatusPublisher::new(
            client,
            database.clone(),
            config.server_id.clone(),
            std::env::current_exe().unwrap().to_string_lossy().to_string(),
            "config.toml".to_string(),
        ))
    } else {
        None
    };

    // Initialize OTS client and registry anchorer
    let ots_client = if config.ots.enabled {
        Some(OtsClient::new(config.ots.aggregator_url.clone()))
    } else {
        None
    };

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

    // Start background tasks
    let config_clone = config.clone();
    let database_clone = database.clone();
        // TODO: Implement audit logger cloning or use Arc

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

    // OTS monthly anchoring task
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
    if audit_logger.is_some() {
        let rotation_interval = Duration::from_secs(config.audit.rotation_interval_days as u64 * 86400);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(rotation_interval);
            loop {
                interval.tick().await;
                // Rotate audit log (implement rotation logic)
                info!("Audit log rotation triggered");
            }
        });
        info!("Audit log rotation started");
    }

    // Build application
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/webhooks/github", post(webhooks::github::handle_webhook))
        .route("/status", get(status_endpoint))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state((config, database));

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "governance-app",
        "timestamp": chrono::Utc::now()
    }))
}

async fn status_endpoint(State((config, database)): State<(AppConfig, Database)>) -> Json<serde_json::Value> {
    let mut status = serde_json::json!({
        "status": "healthy",
        "service": "governance-app",
        "timestamp": chrono::Utc::now(),
        "server_id": config.server_id,
        "features": {
            "nostr": config.nostr.enabled,
            "ots": config.ots.enabled,
            "audit": config.audit.enabled,
            "dry_run": config.dry_run_mode
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
