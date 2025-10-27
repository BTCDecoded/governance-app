# Server Authorization

## Overview

The BTCDecoded governance system implements a server authorization registry that explicitly authorizes which servers can run critical Bitcoin jobs and operations. This provides an additional layer of security by ensuring only trusted, verified servers can perform governance operations.

## Purpose

Server authorization serves as a **security boundary mechanism** by:
- Explicitly authorizing which servers can run governance operations
- Preventing unauthorized servers from performing critical operations
- Providing audit trail of server operations
- Enabling rapid response to compromised servers

## Architecture

### Server Registry

**Registry Structure**:
```json
{
  "version": "2024-01",
  "timestamp": "2024-01-15T10:30:00Z",
  "servers": [
    {
      "server_id": "governance-01",
      "operator": {
        "name": "Alice Smith",
        "jurisdiction": "United States",
        "contact": "alice@example.com",
        "organization": "BTCDecoded Foundation"
      },
      "keys": {
        "nostr_npub": "npub1abc123...",
        "ssh_fingerprint": "SHA256:xyz789..."
      },
      "infrastructure": {
        "vpn_ip": "10.0.0.2",
        "github_runner": true,
        "ots_enabled": true,
        "location": "US-East-1"
      },
      "status": "active",
      "added_at": "2024-01-01T00:00:00Z",
      "last_verified": "2024-01-15T10:30:00Z"
    }
  ],
  "approval_requirements": {
    "add_server": "4-of-5 maintainers",
    "remove_server": "3-of-5 maintainers",
    "compromise_server": "2-of-3 emergency keyholders"
  }
}
```

### Server Information

**Required Information**:
- **Server ID**: Unique identifier for the server
- **Operator Details**: Name, jurisdiction, contact information
- **Cryptographic Keys**: Nostr public key, SSH fingerprint
- **Infrastructure**: VPN IP, capabilities, location
- **Status**: Active, retiring, inactive, compromised

**Optional Information**:
- **Organization**: Company or organization name
- **Maintenance Window**: Preferred maintenance times
- **Backup Contacts**: Alternative contact information
- **Special Capabilities**: Unique server capabilities

## Authorization Process

### Adding Servers

**Application Process**:
1. **Submit Application**: Operator submits server information
2. **Verification**: Maintainers verify operator identity
3. **Technical Review**: Verify server capabilities and security
4. **Approval**: Requires 4-of-5 maintainer signatures
5. **Onboarding**: Set up monitoring and access controls

**CLI Commands**:
```bash
# Add new server
governance-app server add \
  --server-id governance-02 \
  --operator-name "Bob Johnson" \
  --jurisdiction "Switzerland" \
  --contact "bob@example.com" \
  --nostr-npub "npub1def456..." \
  --ssh-fingerprint "SHA256:abc123..." \
  --vpn-ip "10.0.0.3" \
  --github-runner \
  --ots-enabled

# Verify server addition
governance-app server verify --server-id governance-02
```

**Programmatic Addition**:
```rust
use crate::authorization::server::{AuthorizedServer, OperatorInfo, ServerKeys, InfrastructureInfo};

async fn add_server(
    server_id: String,
    operator: OperatorInfo,
    keys: ServerKeys,
    infrastructure: InfrastructureInfo,
) -> Result<(), Error> {
    let server = AuthorizedServer::new(server_id, operator, keys, infrastructure);
    
    // Validate server configuration
    validate_server_config(&server)?;
    
    // Add to database
    database.add_authorized_server(&server).await?;
    
    // Publish to Nostr
    publish_server_added_event(&server).await?;
    
    Ok(())
}
```

### Removing Servers

**Graceful Removal**:
1. **Initiation**: Maintainer or operator requests removal
2. **Graceful Shutdown**: 30-day notice for planned removal
3. **Data Migration**: Transfer data to other servers
4. **Approval**: Requires 3-of-5 maintainer signatures
5. **Decommissioning**: Remove from registry and monitoring

**CLI Commands**:
```bash
# Remove server gracefully
governance-app server remove \
  --server-id governance-02 \
  --reason "Planned decommissioning" \
  --grace-period 30

# Force remove server
governance-app server remove \
  --server-id governance-02 \
  --reason "Security incident" \
  --force
```

**Emergency Revocation**:
```bash
# Emergency revocation
governance-app server compromise \
  --server-id governance-02 \
  --reason "Suspected key compromise" \
  --emergency
```

### Server Status Management

**Status Types**:
- **Active**: Server is operational and authorized
- **Retiring**: Server is being decommissioned
- **Inactive**: Server is temporarily offline
- **Compromised**: Server has been compromised

**Status Updates**:
```bash
# Update server status
governance-app server status \
  --server-id governance-02 \
  --status inactive \
  --reason "Maintenance window"

# Reactivate server
governance-app server status \
  --server-id governance-02 \
  --status active \
  --reason "Maintenance completed"
```

## Verification Process

### Server Verification

**Before Operations**:
1. **Registry Lookup**: Check server in authorized registry
2. **Key Verification**: Verify Nostr public key matches
3. **Status Check**: Ensure server status is "active"
4. **Signature Verification**: Verify operation signatures
5. **Timestamp Validation**: Check operation timestamps

**CLI Verification**:
```bash
# Verify server authorization
governance-app server verify --server-id governance-01

# List all authorized servers
governance-app server list

# Check server status
governance-app server status --server-id governance-01
```

**Programmatic Verification**:
```rust
use crate::authorization::verification::verify_server_authorization;

async fn verify_server(server_id: &str, nostr_npub: &str) -> Result<bool, Error> {
    // Load governance registry
    let registry = load_governance_registry().await?;
    
    // Verify server authorization
    let result = verify_server_authorization(server_id, nostr_npub, &registry)?;
    
    match result.status {
        ServerVerificationStatus::Authorized(_) => {
            println!("Server {} is authorized", server_id);
            Ok(true)
        }
        ServerVerificationStatus::Unauthorized(reason) => {
            println!("Server {} is unauthorized: {}", server_id, reason);
            Ok(false)
        }
        ServerVerificationStatus::Compromised(_) => {
            println!("Server {} is compromised", server_id);
            Ok(false)
        }
        _ => {
            println!("Server {} verification failed", server_id);
            Ok(false)
        }
    }
}
```

### Operation Authorization

**Operation Requirements**:
- **Server ID**: Identifies which server performed operation
- **Operation Type**: Type of operation performed
- **Timestamp**: When operation was performed
- **Signature**: Server's cryptographic signature
- **Authorization Proof**: Reference to server registry

**Operation Format**:
```json
{
  "operation_id": "op_1705320000_123",
  "operation_type": "pr_approval",
  "server_id": "governance-01",
  "timestamp": "2024-01-15T10:30:00Z",
  "signature": "ed25519_signature_hex",
  "authorization_proof": {
    "registry_version": "2024-01",
    "server_status": "active",
    "verification_timestamp": "2024-01-15T10:30:00Z"
  },
  "operation_data": {
    "pr_number": 123,
    "maintainer_id": "maintainer_1",
    "action": "approved"
  }
}
```

## Integration with Governance

### Nostr Events

**Server Authorization Events**:
```json
{
  "id": "event_id",
  "pubkey": "server_npub",
  "created_at": 1705320000,
  "kind": 30078,
  "tags": [
    ["d", "governance-status"],
    ["server", "governance-01"],
    ["authorized_by", "registry-2024-01"],
    ["btcdecoded", "governance-infrastructure"]
  ],
  "content": "{\"server_id\":\"governance-01\",\"status\":\"active\",...}",
  "sig": "signature"
}
```

**Event Publishing**:
```rust
use crate::nostr::{NostrClient, StatusPublisher};

async fn publish_server_status(
    client: &NostrClient,
    server_id: &str,
    status: &str,
) -> Result<(), Error> {
    let event = EventBuilder::new(
        Kind::Custom(30078),
        serde_json::json!({
            "server_id": server_id,
            "status": status,
            "timestamp": Utc::now()
        }).to_string(),
        vec![
            Tag::Generic(TagKind::Custom("d".into()), vec!["server-status".to_string()]),
            Tag::Generic(TagKind::Custom("server".into()), vec![server_id.to_string()]),
        ],
    ).to_event(&client.keys)?;
    
    client.publish_event(event).await?;
    Ok(())
}
```

### Audit Logs

**Server Operation Logging**:
```rust
use crate::audit::{AuditLogger, execute_with_audit};

async fn log_server_operation(
    logger: &mut AuditLogger,
    server_id: &str,
    operation_type: &str,
    operation_data: &[u8],
) -> Result<(), Error> {
    execute_with_audit(
        logger,
        operation_type,
        server_id,
        operation_data,
        || {
            // Perform operation
            Ok(serde_json::json!({"status": "completed"}))
        },
    ).await?;
    
    Ok(())
}
```

### Status Checks

**GitHub Status Integration**:
```rust
use crate::github::StatusCheckGenerator;

async fn create_server_status_check(
    pr_number: i32,
    server_id: &str,
    server_status: &str,
) -> Result<(), Error> {
    let status_check = StatusCheckGenerator::new()
        .with_server_authorization(server_id, server_status)
        .with_verification_timestamp(Utc::now())
        .build();
    
    github_client.create_status_check(pr_number, status_check).await?;
    Ok(())
}
```

## Monitoring and Compliance

### Server Monitoring

**Health Checks**:
```bash
# Check server health
governance-app server health --server-id governance-01

# Monitor server operations
governance-app server operations --server-id governance-01 --limit 10

# Check server compliance
governance-app server compliance --server-id governance-01
```

**Programmatic Monitoring**:
```rust
use crate::authorization::verification::get_server_statistics;

async fn monitor_server_health() -> Result<(), Error> {
    let stats = get_server_statistics(&registry)?;
    
    println!("Server Statistics:");
    println!("  Total: {}", stats.total);
    println!("  Active: {}", stats.active);
    println!("  Inactive: {}", stats.inactive);
    println!("  Compromised: {}", stats.compromised);
    println!("  Health: {:.1}%", stats.health_percentage());
    
    Ok(())
}
```

### Compliance Enforcement

**Automated Checks**:
```rust
use crate::authorization::verification::validate_server_config;

async fn enforce_server_compliance() -> Result<(), Error> {
    let servers = get_authorized_servers(&registry)?;
    
    for server in servers {
        // Validate server configuration
        if let Err(e) = validate_server_config(&server) {
            println!("Server {} compliance issue: {}", server.server_id, e);
        }
        
        // Check server status
        if !server.is_authorized() {
            println!("Server {} is not authorized", server.server_id);
        }
        
        // Check last verification
        if let Some(last_verified) = server.last_verified {
            let days_since_verification = (Utc::now() - last_verified).num_days();
            if days_since_verification > 30 {
                println!("Server {} needs verification ({} days old)", 
                    server.server_id, days_since_verification);
            }
        }
    }
    
    Ok(())
}
```

## Security Considerations

### Key Management

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

### Access Control

**VPN Access**:
```bash
# Configure WireGuard
sudo nano /etc/wireguard/wg0.conf
```

```
[Interface]
PrivateKey = <server_private_key>
Address = 10.0.0.2/24
ListenPort = 51820

[Peer]
PublicKey = <governance_public_key>
Endpoint = <governance_server_ip>:51820
AllowedIPs = 10.0.0.0/24
```

**Firewall Rules**:
```bash
# Allow only authorized IPs
sudo ufw allow from 10.0.0.0/24 to any port 8080
sudo ufw deny from any to any port 8080
```

### Incident Response

**Compromise Detection**:
```bash
# Detect compromised server
governance-app server compromise \
  --server-id governance-02 \
  --reason "Suspected key compromise" \
  --evidence "Unauthorized access detected"

# Isolate compromised server
governance-app server isolate --server-id governance-02
```

**Recovery Procedures**:
```bash
# Generate new keys for compromised server
governance-app server regenerate-keys --server-id governance-02

# Verify new keys
governance-app server verify --server-id governance-02

# Reactivate server
governance-app server status --server-id governance-02 --status active
```

## Troubleshooting

### Common Issues

1. **Server Not Authorized**
   - Check server is in authorized registry
   - Verify server status is active
   - Check Nostr public key matches

2. **Operation Authorization Failed**
   - Verify server signature
   - Check operation timestamp
   - Verify server status

3. **Key Verification Failed**
   - Check key format
   - Verify key matches registry
   - Regenerate keys if necessary

### Debug Commands

```bash
# Debug server authorization
governance-app server debug --server-id governance-01

# Check server operations
governance-app server operations --server-id governance-01 --limit 5

# Verify server keys
governance-app server verify-keys --server-id governance-01
```

### Log Analysis

```bash
# Check server authorization logs
sudo journalctl -u governance-app | grep "server.*authorization"

# Check server operations
sudo journalctl -u governance-app | grep "server.*operation"

# Check server status changes
sudo journalctl -u governance-app | grep "server.*status"
```

## Best Practices

### Server Management

1. **Regular Verification**: Verify server authorization regularly
2. **Key Rotation**: Rotate keys regularly
3. **Status Monitoring**: Monitor server status continuously
4. **Incident Response**: Have clear incident response procedures

### Security

1. **Key Security**: Protect server private keys
2. **Access Control**: Implement proper access controls
3. **Monitoring**: Monitor server operations
4. **Audit Trail**: Maintain complete audit trail

### Operations

1. **Documentation**: Document all server operations
2. **Backup**: Backup server configurations
3. **Testing**: Test server operations regularly
4. **Training**: Train operators on procedures

## References

- [Server Authorization Architecture](../governance/architecture/SERVER_AUTHORIZATION.md)
- [Verification Guide](VERIFICATION.md)
- [Nostr Integration](NOSTR_INTEGRATION.md)
- [Audit Log System](AUDIT_LOG_SYSTEM.md)
- [Configuration Reference](CONFIGURATION.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
