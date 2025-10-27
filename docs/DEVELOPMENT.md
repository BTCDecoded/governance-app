# Development Guide

This guide provides information for developers who want to contribute to the BTCDecoded governance application.

## Development Setup

### Prerequisites

- **Rust 1.70+** ([rustup.rs](https://rustup.rs/))
- **Git** for version control
- **SQLite 3** for database
- **GitHub account** for testing
- **Docker** (optional) for containerized development

### Clone and Setup

```bash
git clone https://github.com/BTCDecoded/governance-app.git
cd governance-app
cargo build
```

### Development Dependencies

```bash
# Install development tools
cargo install cargo-watch
cargo install cargo-clippy
cargo install cargo-fmt
cargo install cargo-audit
cargo install cargo-tarpaulin
```

## Project Structure

```
governance-app/
├── src/
│   ├── main.rs                 # Application entry point
│   ├── lib.rs                  # Library root
│   ├── bin/                    # Binary executables
│   ├── api/                    # API handlers
│   ├── crypto/                 # Cryptographic functions
│   ├── database/               # Database layer
│   ├── economic_nodes/         # Economic node system
│   ├── fork/                   # Governance fork system
│   ├── github/                 # GitHub integration
│   ├── validation/             # Validation logic
│   ├── webhooks/               # Webhook handlers
│   └── error.rs                # Error types
├── tests/                      # Integration tests
├── docs/                       # Documentation
├── config/                     # Configuration files
├── migrations/                 # Database migrations
└── Cargo.toml                  # Dependencies
```

## Development Workflow

### 1. Feature Development

```bash
# Create feature branch
git checkout -b feature/new-feature

# Make changes
# ... edit code ...

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt

# Commit changes
git add .
git commit -m "Add new feature"
```

### 2. Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run integration tests
cargo test --test integration_test

# Run with coverage
cargo tarpaulin --out html
```

### 3. Code Quality

```bash
# Format code
cargo fmt

# Check for issues
cargo clippy

# Check for security issues
cargo audit

# Check for outdated dependencies
cargo outdated
```

### 4. Database Development

```bash
# Create new migration
cargo run --bin governance-app -- create-migration migration_name

# Run migrations
cargo run --bin governance-app -- migrate

# Rollback migration
cargo run --bin governance-app -- rollback

# Check migration status
cargo run --bin governance-app -- migrate-status
```

## Code Standards

### Rust Conventions

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for consistent formatting
- Use `cargo clippy` for linting
- Write comprehensive documentation
- Use meaningful variable and function names

### Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GovernanceError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Crypto error: {0}")]
    CryptoError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
}
```

### Logging

```rust
use tracing::{info, warn, error, debug};

// Use appropriate log levels
info!("Operation completed successfully");
warn!("Deprecated API used");
error!("Operation failed: {}", error);
debug!("Debug information: {:?}", data);
```

### Documentation

```rust
/// Brief description of the function
///
/// # Arguments
///
/// * `param1` - Description of parameter 1
/// * `param2` - Description of parameter 2
///
/// # Returns
///
/// Returns a Result containing the operation result or an error
///
/// # Examples
///
/// ```
/// let result = function_name(param1, param2)?;
/// ```
pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType, Error> {
    // Implementation
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        let input = "test_input";
        let expected = "expected_output";
        let result = function_name(input);
        assert_eq!(result, expected);
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs
use governance_app::Database;

#[tokio::test]
async fn test_database_connection() {
    let db = Database::new_in_memory().await.unwrap();
    // Test database operations
}
```

### Mock Testing

```rust
use mockito::mock;

#[tokio::test]
async fn test_github_api() {
    let mock = mock("GET", "/repos/test/repo")
        .with_status(200)
        .with_body(r#"{"name": "test"}"#)
        .create();

    // Test with mock
}
```

## Database Development

### Migration Files

```sql
-- migrations/001_initial.sql
CREATE TABLE IF NOT EXISTS pull_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_name TEXT NOT NULL,
    pr_number INTEGER NOT NULL,
    head_sha TEXT NOT NULL,
    layer INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(repo_name, pr_number)
);
```

### Database Models

```rust
use serde::{Serialize, Deserialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PullRequest {
    pub id: Option<i32>,
    pub repo_name: String,
    pub pr_number: i32,
    pub head_sha: String,
    pub layer: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

## API Development

### Request Handlers

```rust
use axum::{extract::Path, response::Json};
use serde_json::Value;

pub async fn get_pull_request(
    Path(id): Path<i32>,
    State(db): State<Database>,
) -> Result<Json<Value>, StatusCode> {
    match db.get_pull_request(id).await {
        Ok(pr) => Ok(Json(serde_json::to_value(pr).unwrap())),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}
```

### Response Types

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub status: String,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            data: Some(data),
            error: None,
        }
    }
}
```

## GitHub Integration

### Webhook Handlers

```rust
use axum::{extract::State, response::Json};
use serde_json::Value;

pub async fn handle_webhook(
    State(db): State<Database>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, StatusCode> {
    // Validate webhook signature
    // Process webhook payload
    // Update database
    // Return response
}
```

### GitHub API Client

```rust
use octocrab::Octocrab;

pub struct GitHubClient {
    client: Octocrab,
}

impl GitHubClient {
    pub async fn get_pull_request(&self, owner: &str, repo: &str, pr: u64) -> Result<Value, Error> {
        let pr = self.client
            .pulls(owner, repo)
            .get(pr)
            .await?;
        Ok(serde_json::to_value(pr)?)
    }
}
```

## Cryptographic Functions

### Signature Management

```rust
use secp256k1::{PublicKey, SecretKey, Secp256k1};
use sha2::{Digest, Sha256};

pub struct SignatureManager {
    secp: Secp256k1<secp256k1::All>,
}

impl SignatureManager {
    pub fn verify_signature(
        &self,
        message: &str,
        signature: &str,
        public_key: &str,
    ) -> Result<bool, Error> {
        // Implementation
    }
}
```

## Performance Optimization

### Database Queries

```rust
// Use prepared statements
let pr = sqlx::query_as!(
    PullRequest,
    "SELECT * FROM pull_requests WHERE id = ?",
    id
)
.fetch_one(&pool)
.await?;
```

### Caching

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Cache {
    data: Arc<RwLock<HashMap<String, Value>>>,
    ttl: Duration,
}
```

## Security Considerations

### Input Validation

```rust
use validator::{Validate, ValidationError};

#[derive(Validate)]
pub struct CreatePullRequest {
    #[validate(length(min = 1, max = 100))]
    pub repo_name: String,
    
    #[validate(range(min = 1, max = 1000000))]
    pub pr_number: i32,
}
```

### Error Handling

```rust
// Don't expose internal errors
pub fn handle_error(error: Error) -> ApiResponse<()> {
    match error {
        Error::DatabaseError(_) => ApiResponse::error("Internal server error"),
        Error::ValidationError(msg) => ApiResponse::error(&msg),
        _ => ApiResponse::error("Unknown error"),
    }
}
```

## Deployment

### Docker Development

```dockerfile
FROM rust:1.70-slim

WORKDIR /app
COPY . .
RUN cargo build --release

EXPOSE 3000
CMD ["./target/release/governance-app"]
```

### Environment Configuration

```bash
# Development
export RUST_LOG=debug
export DATABASE_URL=sqlite:dev.db
export GITHUB_APP_ID=123456

# Production
export RUST_LOG=info
export DATABASE_URL=sqlite:/var/lib/governance/prod.db
export GITHUB_APP_ID=789012
```

## Contributing

### Pull Request Process

1. Fork the repository
2. Create feature branch
3. Make changes
4. Add tests
5. Run quality checks
6. Submit pull request

### Code Review

- All code must be reviewed
- Tests must pass
- Documentation must be updated
- Security implications must be considered

### Release Process

1. Update version in Cargo.toml
2. Update CHANGELOG.md
3. Create release tag
4. Build and test release
5. Deploy to production

## Related Documentation

- [Getting Started Guide](./GETTING_STARTED.md) - Quick setup
- [Configuration Reference](./CONFIGURATION.md) - Configuration options
- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues
- [Main Governance Documentation](../../governance/README.md) - System overview




