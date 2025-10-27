# Configuration Reference

This document provides a comprehensive reference for configuring the BTCDecoded governance application.

## Configuration Files

### Main Configuration

- **`config/app.toml`** - Main application configuration
- **`config/production.toml`** - Production-specific settings
- **`config/development.toml`** - Development-specific settings

### Environment Variables

All configuration can be overridden with environment variables using the format `SECTION_KEY=value`.

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

**Options:**
- `url` - Database connection URL
- `max_connections` - Maximum number of connections
- `min_connections` - Minimum number of connections
- `acquire_timeout` - Connection acquisition timeout (seconds)
- `idle_timeout` - Idle connection timeout (seconds)
- `max_lifetime` - Maximum connection lifetime (seconds)

### GitHub Configuration

```toml
[github]
app_id = "123456"
private_key_path = "/path/to/private_key.pem"
webhook_secret = "your_webhook_secret"
base_url = "https://api.github.com"
timeout = 30
```

**Options:**
- `app_id` - GitHub App ID
- `private_key_path` - Path to private key file
- `webhook_secret` - Webhook secret for validation
- `base_url` - GitHub API base URL
- `timeout` - API request timeout (seconds)

### Server Configuration

```toml
[server]
host = "0.0.0.0"
port = 3000
workers = 4
max_request_size = 1048576
```

**Options:**
- `host` - Server bind address
- `port` - Server port
- `workers` - Number of worker threads
- `max_request_size` - Maximum request size (bytes)

### Logging Configuration

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/governance-app.log"
max_size = 10485760
max_files = 5
```

**Options:**
- `level` - Log level (trace, debug, info, warn, error)
- `format` - Log format (json, text)
- `file` - Log file path
- `max_size` - Maximum log file size (bytes)
- `max_files` - Maximum number of log files

### Governance Configuration

```toml
[governance]
config_path = "/path/to/governance/config"
cache_ttl = 300
signature_timeout = 30
```

**Options:**
- `config_path` - Path to governance configuration files
- `cache_ttl` - Configuration cache TTL (seconds)
- `signature_timeout` - Signature verification timeout (seconds)

## Environment Variables

### Database

```bash
DATABASE_URL="sqlite:governance.db"
DATABASE_MAX_CONNECTIONS="10"
DATABASE_MIN_CONNECTIONS="1"
DATABASE_ACQUIRE_TIMEOUT="30"
DATABASE_IDLE_TIMEOUT="600"
DATABASE_MAX_LIFETIME="1800"
```

### GitHub

```bash
GITHUB_APP_ID="123456"
GITHUB_PRIVATE_KEY_PATH="/path/to/private_key.pem"
GITHUB_WEBHOOK_SECRET="your_webhook_secret"
GITHUB_BASE_URL="https://api.github.com"
GITHUB_TIMEOUT="30"
```

### Server

```bash
SERVER_HOST="0.0.0.0"
SERVER_PORT="3000"
SERVER_WORKERS="4"
SERVER_MAX_REQUEST_SIZE="1048576"
```

### Logging

```bash
LOG_LEVEL="info"
LOG_FORMAT="json"
LOG_FILE="/var/log/governance-app.log"
LOG_MAX_SIZE="10485760"
LOG_MAX_FILES="5"
```

### Governance

```bash
GOVERNANCE_CONFIG_PATH="/path/to/governance/config"
GOVERNANCE_CACHE_TTL="300"
GOVERNANCE_SIGNATURE_TIMEOUT="30"
```

## Production Configuration

### Security Settings

```toml
[security]
require_https = true
allowed_origins = ["https://your-domain.com"]
rate_limit_requests = 100
rate_limit_window = 60
```

### Performance Settings

```toml
[performance]
cache_size = 1000
cache_ttl = 300
connection_pool_size = 20
worker_threads = 8
```

### Monitoring Settings

```toml
[monitoring]
metrics_enabled = true
metrics_port = 9090
health_check_interval = 30
```

## Development Configuration

### Debug Settings

```toml
[debug]
log_level = "debug"
log_requests = true
log_responses = true
mock_github_api = false
```

### Testing Settings

```toml
[testing]
test_database_url = "sqlite::memory:"
mock_external_apis = true
test_timeout = 30
```

## Configuration Validation

The application validates configuration on startup:

1. **Required fields** - All required fields must be present
2. **Type validation** - Values must be of correct type
3. **Range validation** - Numeric values must be in valid ranges
4. **File validation** - File paths must exist and be readable
5. **Network validation** - URLs and hosts must be reachable

## Configuration Examples

### Minimal Configuration

```toml
[database]
url = "sqlite:governance.db"

[github]
app_id = "123456"
private_key_path = "private_key.pem"
webhook_secret = "secret"

[server]
host = "0.0.0.0"
port = 3000
```

### Production Configuration

```toml
[database]
url = "sqlite:/var/lib/governance/governance.db"
max_connections = 20
min_connections = 5
acquire_timeout = 60
idle_timeout = 1200
max_lifetime = 3600

[github]
app_id = "123456"
private_key_path = "/etc/governance/private_key.pem"
webhook_secret = "your_webhook_secret"
base_url = "https://api.github.com"
timeout = 60

[server]
host = "0.0.0.0"
port = 3000
workers = 8
max_request_size = 2097152

[logging]
level = "info"
format = "json"
file = "/var/log/governance-app.log"
max_size = 104857600
max_files = 10

[security]
require_https = true
allowed_origins = ["https://governance.btcdecoded.org"]
rate_limit_requests = 1000
rate_limit_window = 60

[performance]
cache_size = 5000
cache_ttl = 600
connection_pool_size = 50
worker_threads = 16

[monitoring]
metrics_enabled = true
metrics_port = 9090
health_check_interval = 30
```

## Troubleshooting

### Common Configuration Issues

1. **Database Connection**: Check database URL and permissions
2. **GitHub API**: Verify App ID and private key
3. **File Permissions**: Ensure application can read configuration files
4. **Network Access**: Check firewall and network configuration
5. **Environment Variables**: Verify environment variable format

### Configuration Validation Errors

If you get configuration validation errors:

1. Check the error message for specific field issues
2. Verify the configuration file syntax
3. Check environment variable format
4. Ensure all required fields are present
5. Validate file paths and permissions

## Related Documentation

- [Getting Started Guide](./GETTING_STARTED.md) - Quick setup
- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues
- [Development Guide](./DEVELOPMENT.md) - Development setup
- [Main Governance Documentation](../../governance/README.md) - System overview




