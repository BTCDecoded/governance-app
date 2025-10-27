# Governance App Deployment Guide

## Overview

This guide covers production deployment of the BTCDecoded Governance App, including environment configuration, database setup, GitHub App installation, and monitoring.

## Prerequisites

### System Requirements

- **OS**: Linux (Ubuntu 20.04+ recommended)
- **CPU**: 2+ cores
- **RAM**: 4GB+ (8GB+ recommended for production)
- **Storage**: 50GB+ SSD
- **Network**: Stable internet connection

### Software Requirements

- **Rust**: 1.70+ (for building from source)
- **SQLite**: 3.35+ (for development) or PostgreSQL 13+ (for production)
- **Git**: 2.30+
- **Docker**: 20.10+ (optional, for containerized deployment)

## Environment Configuration

### Environment Variables

Create a `.env` file with the following variables:

```bash
# Database Configuration
DATABASE_URL=postgresql://user:password@localhost:5432/governance
# For development: DATABASE_URL=sqlite:governance.db

# GitHub App Configuration
GITHUB_APP_ID=123456
GITHUB_PRIVATE_KEY_PATH=/etc/governance/github-app.pem
GITHUB_WEBHOOK_SECRET=your_webhook_secret_here
GOVERNANCE_REPO=BTCDecoded/governance

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
SERVER_ID=governance-01

# Governance Configuration
DRY_RUN_MODE=false
LOG_ENFORCEMENT_DECISIONS=true
ENFORCEMENT_LOG_PATH=/var/log/governance/enforcement.log

# Nostr Configuration
NOSTR_ENABLED=true
NOSTR_SERVER_NSEC_PATH=/etc/governance/server.nsec
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band
NOSTR_PUBLISH_INTERVAL_SECS=3600

# OpenTimestamps Configuration
OTS_ENABLED=true
OTS_AGGREGATOR_URL=https://alice.btc.calendar.opentimestamps.org
OTS_MONTHLY_ANCHOR_DAY=1
OTS_REGISTRY_PATH=/var/lib/governance/registries
OTS_PROOFS_PATH=/var/lib/governance/ots-proofs

# Audit Log Configuration
AUDIT_ENABLED=true
AUDIT_LOG_PATH=/var/lib/governance/audit-log.jsonl
AUDIT_ROTATION_INTERVAL_DAYS=30
```

### Configuration File

Create `config.toml` for additional configuration:

```toml
[server]
database_url = "postgresql://user:password@localhost:5432/governance"
github_app_id = 123456
github_private_key_path = "/etc/governance/github-app.pem"
github_webhook_secret = "env:GITHUB_WEBHOOK_SECRET"
governance_repo = "BTCDecoded/governance"
server_host = "0.0.0.0"
server_port = 8080
dry_run_mode = false
log_enforcement_decisions = true
enforcement_log_path = "/var/log/governance/enforcement.log"
server_id = "governance-01"

[nostr]
enabled = true
server_nsec_path = "/etc/governance/server.nsec"
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band"
]
publish_interval_secs = 3600

[ots]
enabled = true
aggregator_url = "https://alice.btc.calendar.opentimestamps.org"
monthly_anchor_day = 1
registry_path = "/var/lib/governance/registries"
proofs_path = "/var/lib/governance/ots-proofs"

[audit]
enabled = true
log_path = "/var/lib/governance/audit-log.jsonl"
rotation_interval_days = 30
```

## Database Setup

### PostgreSQL (Production)

1. **Install PostgreSQL**:
   ```bash
   sudo apt update
   sudo apt install postgresql postgresql-contrib
   ```

2. **Create Database and User**:
   ```sql
   sudo -u postgres psql
   CREATE DATABASE governance;
   CREATE USER governance_user WITH PASSWORD 'secure_password';
   GRANT ALL PRIVILEGES ON DATABASE governance TO governance_user;
   \q
   ```

3. **Configure PostgreSQL**:
   ```bash
   sudo nano /etc/postgresql/13/main/postgresql.conf
   # Set shared_preload_libraries = 'pg_stat_statements'
   # Set max_connections = 100
   # Set shared_buffers = 256MB
   ```

4. **Restart PostgreSQL**:
   ```bash
   sudo systemctl restart postgresql
   ```

### SQLite (Development)

SQLite is used for development and testing. No additional setup required.

## GitHub App Setup

### 1. Create GitHub App

1. Go to GitHub Settings → Developer settings → GitHub Apps
2. Click "New GitHub App"
3. Fill in app details:
   - **App name**: BTCDecoded Governance
   - **Homepage URL**: https://btcdecoded.org
   - **Webhook URL**: https://your-domain.com/webhooks/github
   - **Webhook secret**: Generate secure secret

### 2. Configure Permissions

Set the following permissions:
- **Repository permissions**:
  - Contents: Read
  - Metadata: Read
  - Pull requests: Write
  - Statuses: Write
- **Subscribe to events**:
  - Pull request
  - Pull request review
  - Status

### 3. Generate Private Key

1. Click "Generate a private key"
2. Save the `.pem` file securely
3. Place in `/etc/governance/github-app.pem`
4. Set proper permissions: `chmod 600 /etc/governance/github-app.pem`

### 4. Install App

1. Go to the app settings
2. Click "Install App"
3. Select repositories to install on
4. Note the App ID for configuration

## Nostr Configuration

### 1. Generate Server Key

```bash
# Generate Nostr private key
nostr-keygen > /etc/governance/server.nsec
chmod 600 /etc/governance/server.nsec

# Get public key
nostr-keygen --pubkey < /etc/governance/server.nsec
```

### 2. Configure Relays

Add relay URLs to configuration:
- `wss://relay.damus.io` (primary)
- `wss://nos.lol` (secondary)
- `wss://relay.nostr.band` (tertiary)

### 3. Test Nostr Connection

```bash
# Test relay connectivity
nostr-cli --relay wss://relay.damus.io --pubkey <your-pubkey>
```

## OpenTimestamps Configuration

### 1. Choose OTS Server

- **Public**: `https://alice.btc.calendar.opentimestamps.org`
- **Self-hosted**: Set up your own OTS server (optional)

### 2. Configure Directories

```bash
# Create OTS directories
sudo mkdir -p /var/lib/governance/registries
sudo mkdir -p /var/lib/governance/ots-proofs
sudo chown governance:governance /var/lib/governance/registries
sudo chown governance:governance /var/lib/governance/ots-proofs
```

### 3. Test OTS Connection

```bash
# Test OTS server
curl -X POST https://alice.btc.calendar.opentimestamps.org/stamp \
  -H "Content-Type: application/octet-stream" \
  --data-binary @test.txt
```

## Server Authorization Setup

### 1. Create Server Registry

```bash
# Create server authorization directory
sudo mkdir -p /etc/governance/servers
sudo chown governance:governance /etc/governance/servers
```

### 2. Add Initial Server

```bash
# Add this server to authorized registry
governance-app server add \
  --server-id governance-01 \
  --operator-name "BTCDecoded Foundation" \
  --jurisdiction "United States" \
  --contact "admin@btcdecoded.org" \
  --nostr-npub "npub1..." \
  --ssh-fingerprint "SHA256:..."
```

### 3. Verify Server Authorization

```bash
# Verify server is authorized
governance-app server verify --server-id governance-01
```

## Application Deployment

### 1. Build Application

```bash
# Clone repository
git clone https://github.com/btcdecoded/governance-system.git
cd governance-system/governance-app

# Build for production
cargo build --release

# Install binary
sudo cp target/release/governance-app /usr/local/bin/
sudo chmod +x /usr/local/bin/governance-app
```

### 2. Create System User

```bash
# Create governance user
sudo useradd -r -s /bin/false governance
sudo mkdir -p /var/lib/governance
sudo chown governance:governance /var/lib/governance
```

### 3. Create Systemd Service

Create `/etc/systemd/system/governance-app.service`:

```ini
[Unit]
Description=BTCDecoded Governance App
After=network.target postgresql.service

[Service]
Type=simple
User=governance
Group=governance
WorkingDirectory=/var/lib/governance
ExecStart=/usr/local/bin/governance-app
Restart=always
RestartSec=5
Environment=RUST_LOG=info
EnvironmentFile=/etc/governance/.env

[Install]
WantedBy=multi-user.target
```

### 4. Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable governance-app

# Start service
sudo systemctl start governance-app

# Check status
sudo systemctl status governance-app
```

## Monitoring and Health Checks

### 1. Health Check Endpoint

The app provides a health check endpoint:
- **URL**: `http://localhost:8080/health`
- **Method**: GET
- **Response**: JSON with status information

### 2. Status Endpoint

Detailed status information:
- **URL**: `http://localhost:8080/status`
- **Method**: GET
- **Response**: JSON with detailed system status

### 3. Log Monitoring

```bash
# View application logs
sudo journalctl -u governance-app -f

# View specific log levels
sudo journalctl -u governance-app -p err

# View logs from specific time
sudo journalctl -u governance-app --since "2024-01-15 10:00:00"
```

### 4. Database Monitoring

```bash
# Check database connection
psql -h localhost -U governance_user -d governance -c "SELECT 1;"

# Check database size
psql -h localhost -U governance_user -d governance -c "SELECT pg_size_pretty(pg_database_size('governance'));"
```

## Backup and Recovery

### 1. Database Backup

```bash
# Create backup script
sudo nano /usr/local/bin/backup-governance.sh
```

```bash
#!/bin/bash
BACKUP_DIR="/var/backups/governance"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p $BACKUP_DIR

# Backup PostgreSQL
pg_dump -h localhost -U governance_user governance > $BACKUP_DIR/governance_$DATE.sql

# Backup configuration
cp /etc/governance/.env $BACKUP_DIR/env_$DATE
cp /etc/governance/config.toml $BACKUP_DIR/config_$DATE.toml

# Backup Nostr keys
cp /etc/governance/server.nsec $BACKUP_DIR/server_$DATE.nsec

# Clean old backups (keep 30 days)
find $BACKUP_DIR -name "*.sql" -mtime +30 -delete
find $BACKUP_DIR -name "env_*" -mtime +30 -delete
find $BACKUP_DIR -name "config_*.toml" -mtime +30 -delete
find $BACKUP_DIR -name "server_*.nsec" -mtime +30 -delete
```

```bash
# Make executable
sudo chmod +x /usr/local/bin/backup-governance.sh

# Add to crontab
echo "0 2 * * * /usr/local/bin/backup-governance.sh" | sudo crontab -
```

### 2. Recovery Procedures

```bash
# Restore database
psql -h localhost -U governance_user -d governance < /var/backups/governance/governance_20240115_020000.sql

# Restore configuration
sudo cp /var/backups/governance/env_20240115_020000 /etc/governance/.env
sudo cp /var/backups/governance/config_20240115_020000.toml /etc/governance/config.toml

# Restore Nostr keys
sudo cp /var/backups/governance/server_20240115_020000.nsec /etc/governance/server.nsec
sudo chmod 600 /etc/governance/server.nsec
```

## Security Considerations

### 1. File Permissions

```bash
# Set proper permissions
sudo chmod 600 /etc/governance/.env
sudo chmod 600 /etc/governance/server.nsec
sudo chmod 600 /etc/governance/github-app.pem
sudo chmod 755 /var/lib/governance
sudo chmod 755 /var/log/governance
```

### 2. Firewall Configuration

```bash
# Configure UFW
sudo ufw allow 22/tcp
sudo ufw allow 8080/tcp
sudo ufw enable
```

### 3. SSL/TLS

Use a reverse proxy (nginx) with SSL certificates:

```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;
    
    ssl_certificate /etc/ssl/certs/governance.crt;
    ssl_certificate_key /etc/ssl/private/governance.key;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   - Check database URL in configuration
   - Verify database is running
   - Check user permissions

2. **GitHub App Not Working**
   - Verify App ID and private key
   - Check webhook URL is accessible
   - Verify webhook secret

3. **Nostr Connection Failed**
   - Check relay URLs are accessible
   - Verify server key is valid
   - Check network connectivity

4. **OTS Stamping Failed**
   - Check OTS server is accessible
   - Verify network connectivity
   - Check server configuration

### Log Analysis

```bash
# Check for errors
sudo journalctl -u governance-app -p err --since "1 hour ago"

# Check for specific errors
sudo journalctl -u governance-app | grep "ERROR"

# Check database errors
sudo journalctl -u governance-app | grep "database"
```

## References

- [Configuration Reference](docs/CONFIGURATION.md)
- [Security Guide](SECURITY.md)
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md)
- [Main Governance Documentation](../governance/README.md)