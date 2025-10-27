# Security Guide

## Overview

This guide covers security configuration, best practices, and incident response procedures for the BTCDecoded Governance App.

## Security Principles

1. **Defense in Depth**: Multiple layers of security
2. **Least Privilege**: Minimal necessary permissions
3. **Zero Trust**: Verify everything, trust nothing
4. **Audit Everything**: Complete audit trail
5. **Fail Secure**: Secure by default

## Key Management

### Maintainer Keys

**Key Generation**:
```bash
# Generate maintainer keypair
governance-app key generate --maintainer-id maintainer_1 --output /etc/governance/keys/

# Verify key generation
governance-app key verify --maintainer-id maintainer_1
```

**Key Storage**:
- **Development**: Encrypted configuration files
- **Production**: Hardware security modules (HSMs)
- **Backup**: Encrypted backups in secure locations
- **Rotation**: Every 6 months for routine, 3 months for emergency

**Key Rotation**:
```bash
# Generate new keypair
governance-app key generate --maintainer-id maintainer_1 --output /etc/governance/keys/new/

# Update maintainer registry
governance-app maintainer update-key --maintainer-id maintainer_1 --new-key /etc/governance/keys/new/maintainer_1.pub

# Verify new key works
governance-app key verify --maintainer-id maintainer_1

# Revoke old key
governance-app key revoke --maintainer-id maintainer_1 --old-key /etc/governance/keys/old/maintainer_1.pub
```

### Server Keys

**Nostr Keys**:
```bash
# Generate Nostr keypair
nostr-keygen > /etc/governance/server.nsec
chmod 600 /etc/governance/server.nsec

# Get public key
nostr-keygen --pubkey < /etc/governance/server.nsec
```

**SSH Keys**:
```bash
# Generate SSH keypair
ssh-keygen -t ed25519 -f /etc/governance/ssh_key -C "governance-01"

# Get fingerprint
ssh-keygen -lf /etc/governance/ssh_key.pub
```

### Emergency Keys

**Emergency Keyholders**:
- **Purpose**: Emergency response and key recovery
- **Threshold**: 2-of-3 for emergency actions
- **Storage**: Air-gapped hardware security modules
- **Rotation**: Every 3 months
- **Access**: Multi-person authorization required

## Database Security

### PostgreSQL Security

**Connection Security**:
```sql
-- Create dedicated user
CREATE USER governance_user WITH PASSWORD 'secure_password';

-- Grant minimal permissions
GRANT CONNECT ON DATABASE governance TO governance_user;
GRANT USAGE ON SCHEMA public TO governance_user;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO governance_user;

-- Enable SSL
ALTER SYSTEM SET ssl = on;
ALTER SYSTEM SET ssl_cert_file = '/etc/ssl/certs/governance.crt';
ALTER SYSTEM SET ssl_key_file = '/etc/ssl/private/governance.key';
```

**Encryption at Rest**:
```bash
# Enable transparent data encryption
sudo apt install postgresql-13-tde

# Configure encryption
echo "tde_key_id = 1" >> /etc/postgresql/13/main/postgresql.conf
```

**Access Control**:
```bash
# Configure pg_hba.conf
sudo nano /etc/postgresql/13/main/pg_hba.conf
```

```
# Local connections
local   governance    governance_user    md5

# Host connections (SSL required)
hostssl governance    governance_user    127.0.0.1/32    md5
hostssl governance    governance_user    ::1/128         md5
```

### SQLite Security

**File Permissions**:
```bash
# Set restrictive permissions
chmod 600 /var/lib/governance/governance.db
chown governance:governance /var/lib/governance/governance.db
```

**Encryption**:
```bash
# Use SQLCipher for encryption
sudo apt install sqlcipher
```

## Network Security

### Firewall Configuration

**UFW Setup**:
```bash
# Enable UFW
sudo ufw enable

# Allow SSH
sudo ufw allow 22/tcp

# Allow HTTPS
sudo ufw allow 443/tcp

# Allow HTTP (redirect to HTTPS)
sudo ufw allow 80/tcp

# Deny all other traffic
sudo ufw default deny incoming
sudo ufw default allow outgoing
```

**iptables Rules**:
```bash
# Create iptables rules
sudo iptables -A INPUT -i lo -j ACCEPT
sudo iptables -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 22 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 443 -j ACCEPT
sudo iptables -A INPUT -j DROP
```

### VPN Configuration

**WireGuard Setup**:
```bash
# Install WireGuard
sudo apt install wireguard

# Generate keys
wg genkey | tee privatekey | wg pubkey > publickey

# Configure WireGuard
sudo nano /etc/wireguard/wg0.conf
```

```
[Interface]
PrivateKey = <private_key>
Address = 10.0.0.2/24
ListenPort = 51820

[Peer]
PublicKey = <peer_public_key>
Endpoint = <peer_ip>:51820
AllowedIPs = 10.0.0.0/24
```

### TLS/SSL Configuration

**Certificate Generation**:
```bash
# Generate private key
openssl genrsa -out /etc/ssl/private/governance.key 4096

# Generate certificate signing request
openssl req -new -key /etc/ssl/private/governance.key -out /etc/ssl/certs/governance.csr

# Generate self-signed certificate (development)
openssl x509 -req -days 365 -in /etc/ssl/certs/governance.csr -signkey /etc/ssl/private/governance.key -out /etc/ssl/certs/governance.crt
```

**Nginx SSL Configuration**:
```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;
    
    ssl_certificate /etc/ssl/certs/governance.crt;
    ssl_certificate_key /etc/ssl/private/governance.key;
    
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-RSA-AES256-GCM-SHA384:DHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## Application Security

### Input Validation

**GitHub Webhook Validation**:
```rust
// Verify webhook signature
fn verify_webhook_signature(payload: &[u8], signature: &str, secret: &str) -> bool {
    let expected_signature = hmac_sha256(secret, payload);
    let provided_signature = hex::decode(signature.trim_start_matches("sha256=")).unwrap();
    constant_time_eq(&expected_signature, &provided_signature)
}
```

**SQL Injection Prevention**:
```rust
// Use parameterized queries
let result = sqlx::query!(
    "SELECT * FROM maintainers WHERE id = $1",
    maintainer_id
)
.fetch_one(&pool)
.await?;
```

### Rate Limiting

**API Rate Limiting**:
```rust
use tower::ServiceBuilder;
use tower_governance::rate_limit::RateLimitLayer;

let app = Router::new()
    .route("/webhooks/github", post(handle_webhook))
    .layer(
        ServiceBuilder::new()
            .layer(RateLimitLayer::new(100, Duration::from_secs(60)))
    );
```

### Authentication

**API Authentication**:
```rust
// Require authentication for sensitive endpoints
async fn require_auth(State(auth): State<AuthState>) -> Result<(), StatusCode> {
    if auth.is_authenticated() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
```

## Audit Log Protection

### Log Integrity

**Hash Chain Verification**:
```rust
// Verify audit log integrity
fn verify_audit_log(entries: &[AuditLogEntry]) -> Result<(), Error> {
    for (i, entry) in entries.iter().enumerate() {
        if i == 0 {
            // Genesis entry
            if entry.previous_log_hash != GENESIS_HASH {
                return Err(Error::InvalidGenesis);
            }
        } else {
            // Verify chain
            let prev_hash = &entries[i-1].this_log_hash;
            if entry.previous_log_hash != *prev_hash {
                return Err(Error::BrokenChain);
            }
        }
        
        // Verify entry hash
        if !entry.verify_hash() {
            return Err(Error::InvalidHash);
        }
    }
    Ok(())
}
```

### Log Encryption

**Encrypt Sensitive Data**:
```rust
// Encrypt sensitive fields in audit logs
fn encrypt_sensitive_data(data: &str, key: &[u8]) -> Result<String, Error> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, data.as_bytes())?;
    Ok(format!("{}:{}", hex::encode(nonce), hex::encode(ciphertext)))
}
```

### Log Rotation

**Secure Log Rotation**:
```bash
# Configure logrotate
sudo nano /etc/logrotate.d/governance
```

```
/var/log/governance/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 640 governance governance
    postrotate
        systemctl reload governance-app
    endscript
}
```

## Incident Response

### Incident Classification

**Severity Levels**:
- **Critical**: System compromise, data breach
- **High**: Service disruption, security vulnerability
- **Medium**: Performance issues, minor security issues
- **Low**: Minor bugs, cosmetic issues

### Response Procedures

**Critical Incidents**:
1. **Immediate Response** (0-15 minutes):
   - Isolate affected systems
   - Notify emergency keyholders
   - Document initial assessment

2. **Containment** (15-60 minutes):
   - Block malicious traffic
   - Preserve evidence
   - Notify stakeholders

3. **Recovery** (1-24 hours):
   - Restore from clean backups
   - Verify system integrity
   - Monitor for re-compromise

4. **Post-Incident** (1-7 days):
   - Conduct root cause analysis
   - Update security measures
   - Document lessons learned

### Emergency Contacts

**Emergency Keyholders**:
- **Primary**: emergency-1@btcdecoded.org
- **Secondary**: emergency-2@btcdecoded.org
- **Tertiary**: emergency-3@btcdecoded.org

**Maintainers**:
- **Lead**: maintainer-1@btcdecoded.org
- **Security**: maintainer-2@btcdecoded.org
- **Operations**: maintainer-3@btcdecoded.org

## Security Monitoring

### Log Monitoring

**Security Event Detection**:
```bash
# Monitor for failed authentication attempts
sudo journalctl -u governance-app | grep "authentication failed"

# Monitor for suspicious activity
sudo journalctl -u governance-app | grep "suspicious"

# Monitor for errors
sudo journalctl -u governance-app -p err
```

### Intrusion Detection

**Fail2Ban Configuration**:
```bash
# Install Fail2Ban
sudo apt install fail2ban

# Configure for governance app
sudo nano /etc/fail2ban/jail.d/governance.conf
```

```
[governance-app]
enabled = true
port = 8080
filter = governance-app
logpath = /var/log/governance/access.log
maxretry = 5
bantime = 3600
```

### Vulnerability Scanning

**Regular Security Scans**:
```bash
# Install security tools
sudo apt install nmap nikto

# Scan for vulnerabilities
nmap -sV -sC -O localhost
nikto -h localhost:8080
```

## Security Checklist

### Pre-Deployment

- [ ] All keys generated securely
- [ ] Database encrypted at rest
- [ ] Network firewall configured
- [ ] TLS certificates installed
- [ ] Access controls configured
- [ ] Monitoring enabled
- [ ] Backup procedures tested
- [ ] Incident response plan ready

### Post-Deployment

- [ ] Security monitoring active
- [ ] Regular security scans scheduled
- [ ] Key rotation schedule established
- [ ] Access logs reviewed
- [ ] Vulnerability assessments completed
- [ ] Incident response tested
- [ ] Security training completed
- [ ] Documentation updated

### Ongoing

- [ ] Daily log review
- [ ] Weekly security scans
- [ ] Monthly key rotation
- [ ] Quarterly security audits
- [ ] Annual penetration testing
- [ ] Continuous monitoring
- [ ] Regular updates
- [ ] Security training

## References

- [Deployment Guide](DEPLOYMENT.md)
- [Configuration Reference](docs/CONFIGURATION.md)
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md)
- [Audit Materials](../audit-materials/README.md)
- [Main Governance Documentation](../governance/README.md)