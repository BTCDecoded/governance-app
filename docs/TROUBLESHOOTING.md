# Troubleshooting Guide

This guide helps you diagnose and resolve common issues with the BTCDecoded governance application.

## Common Issues

### Database Issues

#### Database Connection Failed

**Symptoms:**
- Application fails to start
- Error: "Database connection failed"
- Logs show connection timeout

**Solutions:**
1. Check database URL in configuration
2. Verify SQLite is installed
3. Check file permissions on database file
4. Ensure database directory exists

```bash
# Check SQLite installation
sqlite3 --version

# Check database file permissions
ls -la governance.db

# Test database connection
sqlite3 governance.db "SELECT 1;"
```

#### Database Migration Failed

**Symptoms:**
- Error: "Migration failed"
- Database schema not updated
- Application crashes on startup

**Solutions:**
1. Check database file permissions
2. Verify migration files exist
3. Run migrations manually
4. Check for conflicting schema changes

```bash
# Run migrations manually
cargo run --bin governance-app -- migrate

# Check migration status
cargo run --bin governance-app -- migrate-status
```

### GitHub API Issues

#### GitHub App Authentication Failed

**Symptoms:**
- Error: "GitHub authentication failed"
- API calls return 401 Unauthorized
- Webhook validation fails

**Solutions:**
1. Verify App ID and private key
2. Check private key file permissions
3. Ensure app is installed on repositories
4. Verify webhook secret

```bash
# Check private key file
ls -la private_key.pem

# Test GitHub API connection
curl -H "Authorization: Bearer <token>" https://api.github.com/app
```

#### Webhook Validation Failed

**Symptoms:**
- Error: "Webhook validation failed"
- GitHub webhooks not processed
- Logs show signature mismatch

**Solutions:**
1. Verify webhook secret in configuration
2. Check webhook URL in GitHub settings
3. Ensure webhook secret matches
4. Check webhook event types

```bash
# Check webhook secret
echo $GITHUB_WEBHOOK_SECRET

# Test webhook locally
ngrok http 3000
```

### Configuration Issues

#### Configuration Validation Failed

**Symptoms:**
- Error: "Configuration validation failed"
- Application fails to start
- Logs show configuration errors

**Solutions:**
1. Check configuration file syntax
2. Verify all required fields are present
3. Check environment variable format
4. Validate file paths and permissions

```bash
# Validate configuration
cargo run --bin governance-app -- validate-config

# Check configuration file
toml-cli validate config/app.toml
```

#### Missing Environment Variables

**Symptoms:**
- Error: "Missing required environment variable"
- Configuration not loaded
- Application uses default values

**Solutions:**
1. Set all required environment variables
2. Check environment variable names
3. Verify variable format
4. Use configuration file as fallback

```bash
# Check environment variables
env | grep -E "(DATABASE|GITHUB|SERVER|LOG)"

# Set missing variables
export DATABASE_URL="sqlite:governance.db"
export GITHUB_APP_ID="123456"
```

### Performance Issues

#### High Memory Usage

**Symptoms:**
- Application uses excessive memory
- System becomes slow
- Out of memory errors

**Solutions:**
1. Check connection pool settings
2. Reduce cache size
3. Optimize database queries
4. Monitor memory usage

```bash
# Monitor memory usage
htop
ps aux | grep governance-app

# Check connection pool
cargo run --bin governance-app -- stats
```

#### Slow Database Queries

**Symptoms:**
- Slow API responses
- Database timeouts
- High CPU usage

**Solutions:**
1. Add database indexes
2. Optimize query patterns
3. Increase connection pool size
4. Check database file size

```bash
# Check database size
ls -lh governance.db

# Analyze database
sqlite3 governance.db "ANALYZE;"

# Check query performance
sqlite3 governance.db "EXPLAIN QUERY PLAN SELECT * FROM pull_requests;"
```

### Security Issues

#### Signature Verification Failed

**Symptoms:**
- Error: "Signature verification failed"
- Maintainer signatures rejected
- Economic node vetoes rejected

**Solutions:**
1. Check signature format
2. Verify public key format
3. Ensure correct message signing
4. Check signature algorithm

```bash
# Test signature verification
cargo run --bin governance-app -- test-signature

# Check public key format
openssl rsa -pubin -in public_key.pem -text
```

#### Key Management Issues

**Symptoms:**
- Error: "Key not found"
- Key rotation failed
- Key validation failed

**Solutions:**
1. Check key ID format
2. Verify key exists in database
3. Check key permissions
4. Ensure key is not expired

```bash
# List all keys
cargo run --bin governance-app -- list-keys

# Check key status
cargo run --bin governance-app -- key-status <key_id>
```

## Debugging

### Enable Debug Logging

```toml
[logging]
level = "debug"
format = "json"
file = "/var/log/governance-app.log"
```

### Check Application Logs

```bash
# Follow logs
tail -f /var/log/governance-app.log

# Filter by level
grep "ERROR" /var/log/governance-app.log

# Filter by component
grep "database" /var/log/governance-app.log
```

### Test Individual Components

```bash
# Test database connection
cargo run --bin governance-app -- test-database

# Test GitHub API
cargo run --bin governance-app -- test-github

# Test signature verification
cargo run --bin governance-app -- test-signatures

# Test webhook validation
cargo run --bin governance-app -- test-webhook
```

### Monitor System Resources

```bash
# Monitor CPU and memory
htop

# Monitor disk usage
df -h

# Monitor network connections
netstat -tulpn | grep governance-app

# Monitor database connections
sqlite3 governance.db "PRAGMA database_list;"
```

## Performance Tuning

### Database Optimization

```toml
[database]
max_connections = 20
min_connections = 5
acquire_timeout = 60
idle_timeout = 1200
max_lifetime = 3600
```

### Caching Configuration

```toml
[performance]
cache_size = 5000
cache_ttl = 600
connection_pool_size = 50
worker_threads = 16
```

### Logging Optimization

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/governance-app.log"
max_size = 104857600
max_files = 10
```

## Monitoring and Alerting

### Health Checks

```bash
# Check application health
curl http://localhost:3000/health

# Check database health
curl http://localhost:3000/health/database

# Check GitHub API health
curl http://localhost:3000/health/github
```

### Metrics Collection

```bash
# Enable metrics
curl http://localhost:9090/metrics

# Check system metrics
curl http://localhost:9090/metrics/system

# Check application metrics
curl http://localhost:9090/metrics/app
```

## Getting Help

### Check Documentation

1. [Getting Started Guide](./GETTING_STARTED.md)
2. [Configuration Reference](./CONFIGURATION.md)
3. [API Reference](./API_REFERENCE.md)
4. [Development Guide](./DEVELOPMENT.md)

### Check Logs

1. Application logs: `/var/log/governance-app.log`
2. System logs: `/var/log/syslog`
3. Database logs: Check SQLite journal files

### Check Configuration

1. Configuration file: `config/app.toml`
2. Environment variables: `env | grep -E "(DATABASE|GITHUB|SERVER|LOG)"`
3. Database schema: `sqlite3 governance.db ".schema"`

### Check Network

1. GitHub API connectivity: `curl https://api.github.com`
2. Webhook endpoint: `curl http://localhost:3000/webhook/github`
3. Health endpoint: `curl http://localhost:3000/health`

## Related Documentation

- [Getting Started Guide](./GETTING_STARTED.md) - Quick setup
- [Configuration Reference](./CONFIGURATION.md) - Configuration options
- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Development Guide](./DEVELOPMENT.md) - Development setup
- [Main Governance Documentation](../../governance/README.md) - System overview




