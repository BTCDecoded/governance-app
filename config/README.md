# Configuration Files

This directory contains configuration files for the BTCDecoded governance application.

## Available Files

- **`app.toml`** - Main application configuration
- **`production.toml`** - Production-specific settings
- **`development.toml`** - Development-specific settings

## Configuration Overview

The governance application uses TOML format for configuration files, with support for environment variable overrides.

### Configuration Hierarchy

1. **Default values** - Built-in defaults
2. **Configuration file** - `app.toml` or environment-specific file
3. **Environment variables** - Override file settings
4. **Command line arguments** - Override all other settings

## Quick Start

### Basic Configuration

```toml
[database]
url = "sqlite:governance.db"

[github]
app_id = "123456"
private_key_path = "private_key.pem"
webhook_secret = "your_webhook_secret"

[server]
host = "0.0.0.0"
port = 3000
```

### Environment Variables

```bash
export DATABASE_URL="sqlite:governance.db"
export GITHUB_APP_ID="123456"
export GITHUB_PRIVATE_KEY_PATH="private_key.pem"
export GITHUB_WEBHOOK_SECRET="your_webhook_secret"
export SERVER_HOST="0.0.0.0"
export SERVER_PORT="3000"
```

## Configuration Sections

### Database Configuration

```toml
[database]
url = "sqlite:governance.db"
max_connections = 10
min_connections = 1
acquire_timeout = 30
idle_timeout = 600
max_lifetime = 1800
```

### GitHub Configuration

```toml
[github]
app_id = "123456"
private_key_path = "/path/to/private_key.pem"
webhook_secret = "your_webhook_secret"
base_url = "https://api.github.com"
timeout = 30
```

### Server Configuration

```toml
[server]
host = "0.0.0.0"
port = 3000
workers = 4
max_request_size = 1048576
```

### Logging Configuration

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/governance-app.log"
max_size = 10485760
max_files = 5
```

### Governance Configuration

```toml
[governance]
config_path = "/path/to/governance/config"
cache_ttl = 300
signature_timeout = 30
```

## Environment-Specific Files

### Development Configuration

```toml
# config/development.toml
[logging]
level = "debug"

[database]
url = "sqlite:dev.db"

[github]
base_url = "https://api.github.com"
```

### Production Configuration

```toml
# config/production.toml
[logging]
level = "info"
file = "/var/log/governance-app.log"

[database]
url = "sqlite:/var/lib/governance/prod.db"
max_connections = 20

[security]
require_https = true
rate_limit_requests = 1000
```

## Configuration Validation

The application validates configuration on startup:

1. **Required fields** - All required fields must be present
2. **Type validation** - Values must be of correct type
3. **Range validation** - Numeric values must be in valid ranges
4. **File validation** - File paths must exist and be readable
5. **Network validation** - URLs and hosts must be reachable

## Security Considerations

### Sensitive Data

- **Private keys** - Store in secure locations
- **Webhook secrets** - Use strong, random secrets
- **Database URLs** - Use secure connection strings
- **API keys** - Rotate regularly

### File Permissions

```bash
# Secure configuration files
chmod 600 config/app.toml
chmod 600 private_key.pem

# Secure directories
chmod 700 config/
chmod 700 /var/lib/governance/
```

## Troubleshooting

### Common Issues

1. **Configuration not loaded** - Check file path and permissions
2. **Validation failed** - Check required fields and types
3. **Environment variables not working** - Check variable names and format
4. **File not found** - Check file paths and permissions

### Debug Configuration

```bash
# Validate configuration
cargo run --bin governance-app -- validate-config

# Show current configuration
cargo run --bin governance-app -- show-config

# Test configuration
cargo run --bin governance-app -- test-config
```

## Related Documentation

- [Configuration Reference](../docs/CONFIGURATION.md) - Detailed configuration options
- [Getting Started Guide](../docs/GETTING_STARTED.md) - Quick setup
- [Troubleshooting Guide](../docs/TROUBLESHOOTING.md) - Common issues
- [Development Guide](../docs/DEVELOPMENT.md) - Development setup
- [Main Governance Documentation](../../governance/README.md) - System overview




