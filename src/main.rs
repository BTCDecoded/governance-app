use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod error;
mod webhooks;
mod validation;
mod enforcement;
mod database;
mod crypto;
mod github;

use config::AppConfig;
use database::Database;

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

    // Build application
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/webhooks/github", post(webhooks::github::handle_webhook))
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




