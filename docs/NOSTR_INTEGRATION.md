# Nostr Integration

## Overview

The BTCDecoded governance system uses Nostr (Notes and Other Stuff Transmitted by Relays) for real-time transparency and communication. This provides a decentralized, censorship-resistant way to publish governance events and status updates.

## Purpose

Nostr integration serves as a **transparency mechanism** by:
- Publishing real-time governance status updates
- Providing public verification of server operations
- Enabling decentralized monitoring of governance events
- Creating an immutable public record of governance actions

## Architecture

### Event Types

**Governance Status Events (Kind 30078)**:
- Published hourly by each authorized server
- Contains server health, binary/config hashes, audit log status
- Tagged with `d:governance-status` for easy filtering
- Signed by server's Nostr private key

**Server Health Events (Kind 30079)**:
- Published when server status changes
- Contains uptime, last merge, operational metrics
- Tagged with `d:server-health` for filtering
- Signed by server's Nostr private key

**Audit Log Head Events (Kind 30080)**:
- Published when audit log head changes
- Contains current audit log head hash and entry count
- Tagged with `d:audit-head` for filtering
- Signed by server's Nostr private key

### Event Format

**Governance Status Event**:
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
    ["btcdecoded", "governance-infrastructure"],
    ["t", "bitcoin"],
    ["t", "governance"]
  ],
  "content": "{\"server_id\":\"governance-01\",\"timestamp\":\"2024-01-15T10:30:00Z\",\"hashes\":{\"binary\":\"sha256:abc123...\",\"config\":\"sha256:def456...\"},\"health\":{\"uptime_hours\":720,\"last_merge_pr\":123,\"last_merge\":\"2024-01-14T15:30:00Z\",\"merges_today\":3},\"next_ots_anchor\":\"2024-02-01T00:00:00Z\",\"audit_log_head\":\"sha256:ghi789...\",\"audit_log_length\":1500}",
  "sig": "signature"
}
```

## Configuration

### Server Configuration

**Environment Variables**:
```bash
# Enable Nostr integration
NOSTR_ENABLED=true

# Server private key (nsec format)
NOSTR_SERVER_NSEC_PATH=/etc/governance/server.nsec

# Relay URLs (comma-separated)
NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.nostr.band

# Publishing interval (seconds)
NOSTR_PUBLISH_INTERVAL_SECS=3600
```

**Configuration File**:
```toml
[nostr]
enabled = true
server_nsec_path = "/etc/governance/server.nsec"
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.band"
]
publish_interval_secs = 3600
```

### Key Management

**Generate Server Key**:
```bash
# Generate Nostr keypair
nostr-keygen > /etc/governance/server.nsec
chmod 600 /etc/governance/server.nsec

# Get public key
nostr-keygen --pubkey < /etc/governance/server.nsec
```

**Add to Server Registry**:
```bash
# Add server to authorized registry
governance-app server add \
  --server-id governance-01 \
  --nostr-npub "npub1..." \
  --operator-name "BTCDecoded Foundation" \
  --jurisdiction "United States"
```

## Relay Selection

### Recommended Relays

**Primary Relays**:
- `wss://relay.damus.io` - High availability, good performance
- `wss://nos.lol` - Reliable, good uptime
- `wss://relay.nostr.band` - Good performance, reliable

**Secondary Relays**:
- `wss://relay.snort.social` - Popular, good uptime
- `wss://relay.nostr.wine` - Good performance
- `wss://relay.nostr.bg` - Reliable, good uptime

### Relay Selection Criteria

1. **Uptime**: Choose relays with high uptime
2. **Performance**: Fast response times
3. **Reliability**: Consistent operation
4. **Geographic Distribution**: Spread across regions
5. **Censorship Resistance**: Multiple independent operators

### Relay Monitoring

**Check Relay Status**:
```bash
# Test relay connectivity
nostr-cli --relay wss://relay.damus.io --pubkey <your-pubkey>

# Monitor relay performance
governance-app nostr monitor --relay wss://relay.damus.io
```

**Programmatic Monitoring**:
```rust
use nostr_sdk::prelude::*;

async fn monitor_relay(relay_url: &str) -> Result<(), Error> {
    let keys = Keys::generate();
    let client = Client::new(&keys);
    
    // Add relay
    client.add_relay(relay_url).await?;
    client.connect().await;
    
    // Check relay status
    let relays = client.relays().await;
    for (url, relay) in &relays {
        println!("Relay {}: {}", url, relay.is_connected());
    }
    
    Ok(())
}
```

## Event Publishing

### Status Publishing

**Hourly Status Updates**:
```rust
use crate::nostr::{NostrClient, StatusPublisher};

async fn publish_hourly_status() -> Result<(), Error> {
    let client = NostrClient::new(nsec, relay_urls).await?;
    let publisher = StatusPublisher::new(
        client,
        database,
        server_id,
        binary_path,
        config_path,
    );
    
    // Publish status
    publisher.publish_status().await?;
    
    Ok(())
}
```

**Event Content Structure**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceStatus {
    pub server_id: String,
    pub timestamp: DateTime<Utc>,
    pub hashes: Hashes,
    pub health: ServerHealth,
    pub next_ots_anchor: DateTime<Utc>,
    pub audit_log_head: Option<String>,
    pub audit_log_length: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hashes {
    pub binary: String,  // sha256:...
    pub config: String,  // sha256:...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHealth {
    pub uptime_hours: u64,
    pub last_merge_pr: Option<i32>,
    pub last_merge: Option<DateTime<Utc>>,
    pub merges_today: i64,
}
```

### Event Tagging

**Standard Tags**:
- `d:governance-status` - Event type identifier
- `server:governance-01` - Server identifier
- `authorized_by:registry-2024-01` - Authorization proof
- `btcdecoded:governance-infrastructure` - System identifier
- `t:bitcoin` - Bitcoin-related content
- `t:governance` - Governance-related content

**Custom Tags**:
- `next_ots:2024-02-01T00:00:00Z` - Next OTS anchoring date
- `audit_head:sha256:...` - Current audit log head
- `merges_today:3` - Number of merges today

## Event Verification

### Signature Verification

**Verify Event Signature**:
```bash
# Verify event signature
nostr-cli verify --event '{"id":"...","pubkey":"...","sig":"..."}'
```

**Programmatic Verification**:
```rust
use nostr_sdk::prelude::*;

fn verify_governance_event(event: &Event) -> bool {
    // Verify event signature
    if !event.verify() {
        return false;
    }
    
    // Verify event kind
    if event.kind != Kind::Custom(30078) {
        return false;
    }
    
    // Verify required tags
    let d_tag = event.tags.iter().find(|tag| tag.as_vec()[0] == "d");
    if d_tag.is_none() || d_tag.unwrap().as_vec()[1] != "governance-status" {
        return false;
    }
    
    true
}
```

### Content Verification

**Verify Event Content**:
```rust
use serde_json::Value;

fn verify_governance_content(content: &str) -> Result<(), Error> {
    let status: GovernanceStatus = serde_json::from_str(content)?;
    
    // Verify server ID
    if status.server_id.is_empty() {
        return Err(Error::InvalidServerId);
    }
    
    // Verify timestamp
    if status.timestamp > Utc::now() {
        return Err(Error::FutureTimestamp);
    }
    
    // Verify hashes
    if !status.hashes.binary.starts_with("sha256:") {
        return Err(Error::InvalidHash);
    }
    
    Ok(())
}
```

## Monitoring and Analytics

### Event Monitoring

**Subscribe to Events**:
```bash
# Subscribe to governance status events
nostr-cli --relay wss://relay.damus.io --filter '{"kinds":[30078],"#d":["governance-status"]}'

# Subscribe to specific server events
nostr-cli --relay wss://relay.damus.io --filter '{"kinds":[30078],"#server":["governance-01"]}'
```

**Programmatic Monitoring**:
```rust
use nostr_sdk::prelude::*;

async fn monitor_governance_events() -> Result<(), Error> {
    let keys = Keys::generate();
    let client = Client::new(&keys);
    
    // Connect to relays
    client.add_relay("wss://relay.damus.io").await?;
    client.connect().await;
    
    // Subscribe to governance events
    let filter = Filter::new()
        .kind(Kind::Custom(30078))
        .custom_tag("d", vec!["governance-status"]);
    
    let mut events = client.get_events_of(vec![filter]).await;
    while let Some(event) = events.next().await {
        println!("Received governance event: {:?}", event);
        
        // Verify and process event
        if verify_governance_event(&event) {
            process_governance_event(&event).await?;
        }
    }
    
    Ok(())
}
```

### Analytics Dashboard

**Simple Dashboard**:
```html
<!DOCTYPE html>
<html>
<head>
    <title>Governance Status Dashboard</title>
    <script src="https://unpkg.com/nostr-tools@latest/dist/nostr-tools.js"></script>
</head>
<body>
    <h1>Governance System Status</h1>
    
    <div id="servers">
        <h2>Authorized Servers</h2>
        <div id="server-list"></div>
    </div>
    
    <div id="events">
        <h2>Recent Events</h2>
        <div id="event-list"></div>
    </div>

    <script>
        // Connect to Nostr relay
        const relay = new NostrRelay('wss://relay.damus.io');
        
        // Subscribe to governance events
        relay.subscribe({
            kinds: [30078],
            '#d': ['governance-status']
        }, (event) => {
            const status = JSON.parse(event.content);
            addEventToList(event, status);
        });
        
        function addEventToList(event, status) {
            const eventDiv = document.createElement('div');
            eventDiv.innerHTML = `
                <div>
                    <strong>Server:</strong> ${status.server_id}<br>
                    <strong>Time:</strong> ${status.timestamp}<br>
                    <strong>Uptime:</strong> ${status.health.uptime_hours} hours<br>
                    <strong>Merges Today:</strong> ${status.health.merges_today}
                </div>
            `;
            document.getElementById('event-list').appendChild(eventDiv);
        }
    </script>
</body>
</html>
```

## Troubleshooting

### Common Issues

1. **Relay Connection Failed**
   - Check relay URL is correct
   - Verify network connectivity
   - Try different relay

2. **Event Publishing Failed**
   - Check server key is valid
   - Verify relay is accessible
   - Check event format

3. **Event Verification Failed**
   - Check event signature
   - Verify event content
   - Check event tags

### Debug Commands

```bash
# Test relay connectivity
governance-app nostr test --relay wss://relay.damus.io

# Publish test event
governance-app nostr publish-test --content "test event"

# Monitor relay performance
governance-app nostr monitor --relay wss://relay.damus.io --duration 60
```

### Log Analysis

```bash
# Check Nostr logs
sudo journalctl -u governance-app | grep nostr

# Check for relay errors
sudo journalctl -u governance-app | grep "relay.*error"

# Check event publishing
sudo journalctl -u governance-app | grep "published.*event"
```

## Best Practices

### Relay Management

1. **Use Multiple Relays**: Distribute events across multiple relays
2. **Monitor Relay Health**: Regularly check relay status
3. **Have Backup Relays**: Keep backup relays ready
4. **Test Regularly**: Test relay connectivity regularly

### Event Publishing

1. **Consistent Timing**: Publish events at regular intervals
2. **Error Handling**: Handle relay failures gracefully
3. **Retry Logic**: Implement retry logic for failed publishes
4. **Monitoring**: Monitor event publishing success

### Security

1. **Key Security**: Protect server private keys
2. **Event Verification**: Always verify received events
3. **Content Validation**: Validate event content
4. **Relay Trust**: Use trusted, reliable relays

## References

- [Nostr Protocol Specification](https://nostr.com/)
- [Nostr SDK Documentation](https://docs.rs/nostr-sdk/)
- [Verification Guide](VERIFICATION.md)
- [Configuration Reference](CONFIGURATION.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
