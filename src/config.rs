use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub github_app_id: u64,
    pub github_private_key_path: String,
    pub github_webhook_secret: String,
    pub governance_repo: String,
    pub server_host: String,
    pub server_port: u16,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://governance.db".to_string());
        
        let github_app_id = env::var("GITHUB_APP_ID")
            .unwrap_or_else(|_| "123456".to_string())
            .parse()?;
        
        let github_private_key_path = env::var("GITHUB_PRIVATE_KEY_PATH")
            .unwrap_or_else(|_| "/path/to/private-key.pem".to_string());
        
        let github_webhook_secret = env::var("GITHUB_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "your_webhook_secret_here".to_string());
        
        let governance_repo = env::var("GOVERNANCE_REPO")
            .unwrap_or_else(|_| "BTCDecoded/governance".to_string());
        
        let server_host = env::var("SERVER_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        Ok(AppConfig {
            database_url,
            github_app_id,
            github_private_key_path,
            github_webhook_secret,
            governance_repo,
            server_host,
            server_port,
        })
    }
}




