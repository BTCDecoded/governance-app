# Verification Guide

## Overview

This guide explains how to verify the integrity and authenticity of the BTCDecoded governance system, including Nostr events, OpenTimestamps proofs, audit logs, and server authorization.

## Nostr Event Verification

### Subscribing to Status Updates

**Using Nostr Client**:
```bash
# Install nostr-cli
cargo install nostr-cli

# Subscribe to governance status events
nostr-cli --relay wss://relay.damus.io --filter '{"kinds":[30078],"#d":["governance-status"]}'
```

**Using Python**:
```python
import asyncio
from nostr_sdk import Client, Keys, Filter, Kind

async def subscribe_to_governance_status():
    keys = Keys.generate()
    client = Client(keys)
    
    # Connect to relay
    await client.add_relay("wss://relay.damus.io")
    await client.connect()
    
    # Subscribe to governance status events
    filter = Filter().kind(Kind(30078)).custom_tag("d", ["governance-status"])
    
    async for event in client.get_events_of([filter]):
        print(f"Received governance status: {event.content}")
        # Verify signature and content
        if event.verify():
            print("Event signature verified")
        else:
            print("Event signature verification failed")

# Run the subscription
asyncio.run(subscribe_to_governance_status())
```

### Verifying Event Signatures

**Manual Verification**:
```bash
# Extract event data
echo '{"id":"event_id","pubkey":"pubkey","created_at":1234567890,"kind":30078,"tags":[["d","governance-status"]],"content":"status_json","sig":"signature"}' | jq .

# Verify signature using nostr-cli
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
    
    // Verify tags
    let d_tag = event.tags.iter().find(|tag| tag.as_vec()[0] == "d");
    if d_tag.is_none() || d_tag.unwrap().as_vec()[1] != "governance-status" {
        return false;
    }
    
    true
}
```

## OpenTimestamps Verification

### Verifying OTS Proofs

**Manual Verification**:
```bash
# Install ots-cli
pip install opentimestamps-client

# Verify a proof
ots verify /var/lib/governance/ots-proofs/2024-01.json.ots

# Get Bitcoin block height
ots info /var/lib/governance/ots-proofs/2024-01.json.ots
```

**Programmatic Verification**:
```rust
use opentimestamps::client::OtsClient;

async fn verify_ots_proof(proof_path: &str, data_path: &str) -> Result<bool, Error> {
    let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
    
    let data = std::fs::read(data_path)?;
    let proof = std::fs::read(proof_path)?;
    
    match ots_client.verify(&data, &proof).await? {
        VerificationResult::Confirmed(block_height) => {
            println!("Proof confirmed in Bitcoin block {}", block_height);
            Ok(true)
        }
        VerificationResult::Pending => {
            println!("Proof is pending confirmation");
            Ok(false)
        }
        VerificationResult::Invalid => {
            println!("Proof is invalid");
            Ok(false)
        }
    }
}
```

### Verifying Registry Integrity

**Download and Verify Registry**:
```bash
# Download latest registry
curl -o registry.json https://btcdecoded.org/governance/registries/2024-01.json

# Verify registry signature
governance-app registry verify --registry registry.json

# Verify OTS proof
ots verify registry.json.ots
```

**Programmatic Verification**:
```rust
use serde_json::Value;

async fn verify_registry(registry_path: &str, proof_path: &str) -> Result<bool, Error> {
    // Load registry
    let registry_data = std::fs::read(registry_path)?;
    let registry: Value = serde_json::from_slice(&registry_data)?;
    
    // Verify OTS proof
    let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
    let proof_data = std::fs::read(proof_path)?;
    
    match ots_client.verify(&registry_data, &proof_data).await? {
        VerificationResult::Confirmed(_) => {
            println!("Registry verified and anchored to Bitcoin");
            Ok(true)
        }
        _ => {
            println!("Registry verification failed");
            Ok(false)
        }
    }
}
```

## Audit Log Verification

### Verifying Hash Chain

**Using CLI Tool**:
```bash
# Verify audit log integrity
governance-app audit verify --log-path /var/lib/governance/audit-log.jsonl

# Verify specific entry
governance-app audit verify-entry --log-path /var/lib/governance/audit-log.jsonl --entry-id "job_123"
```

**Programmatic Verification**:
```rust
use crate::audit::{AuditLogger, verify_audit_log};

async fn verify_audit_log_integrity(log_path: &str) -> Result<(), Error> {
    let logger = AuditLogger::new(log_path.to_string())?;
    
    // Load all entries
    let entries = logger.load_all_entries().await?;
    
    // Verify hash chain
    verify_audit_log(&entries)?;
    
    println!("Audit log integrity verified");
    Ok(())
}
```

### Verifying Merkle Root

**Calculate Merkle Root**:
```bash
# Calculate Merkle root for audit log
governance-app audit merkle-root --log-path /var/lib/governance/audit-log.jsonl

# Verify against published registry
governance-app audit verify-merkle --log-path /var/lib/governance/audit-log.jsonl --registry registry.json
```

**Programmatic Verification**:
```rust
use crate::audit::{build_merkle_tree, verify_merkle_root};

async fn verify_merkle_root(log_path: &str, claimed_root: &str) -> Result<bool, Error> {
    let logger = AuditLogger::new(log_path.to_string())?;
    let entries = logger.load_all_entries().await?;
    
    // Build Merkle tree
    let merkle_tree = build_merkle_tree(&entries)?;
    
    // Verify root
    let is_valid = verify_merkle_root(&entries, claimed_root)?;
    
    if is_valid {
        println!("Merkle root verified: {}", merkle_tree.hash);
    } else {
        println!("Merkle root verification failed");
    }
    
    Ok(is_valid)
}
```

## Server Authorization Verification

### Verifying Server Authorization

**Using CLI Tool**:
```bash
# Verify server is authorized
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

### Verifying Server Operations

**Check Server Operations**:
```bash
# List recent operations by server
governance-app server operations --server-id governance-01 --limit 10

# Verify specific operation
governance-app server verify-operation --operation-id "op_123"
```

**Programmatic Verification**:
```rust
use crate::audit::AuditLogger;

async fn verify_server_operations(server_id: &str, log_path: &str) -> Result<(), Error> {
    let logger = AuditLogger::new(log_path.to_string())?;
    let entries = logger.load_all_entries().await?;
    
    // Filter operations by server
    let server_operations: Vec<_> = entries
        .iter()
        .filter(|entry| entry.server_id == server_id)
        .collect();
    
    println!("Found {} operations for server {}", server_operations.len(), server_id);
    
    // Verify each operation
    for operation in server_operations {
        if !operation.verify_hash() {
            return Err(Error::InvalidOperation(operation.job_id.clone()));
        }
    }
    
    println!("All server operations verified");
    Ok(())
}
```

## Public Verification Scripts

### Complete System Verification

**verify-governance.sh**:
```bash
#!/bin/bash

set -e

echo "Verifying BTCDecoded Governance System..."

# Configuration
GOVERNANCE_URL="https://btcdecoded.org/governance"
REGISTRY_URL="$GOVERNANCE_URL/registries"
AUDIT_LOG_URL="$GOVERNANCE_URL/audit-logs"
NOSTR_RELAY="wss://relay.damus.io"

# Download latest registry
echo "Downloading latest registry..."
REGISTRY_FILE=$(mktemp)
PROOF_FILE=$(mktemp)
curl -s "$REGISTRY_URL/latest.json" -o "$REGISTRY_FILE"
curl -s "$REGISTRY_URL/latest.json.ots" -o "$PROOF_FILE"

# Verify registry
echo "Verifying registry..."
if ots verify "$PROOF_FILE"; then
    echo "✓ Registry anchored to Bitcoin"
else
    echo "✗ Registry verification failed"
    exit 1
fi

# Verify audit log
echo "Verifying audit log..."
AUDIT_LOG_FILE=$(mktemp)
curl -s "$AUDIT_LOG_URL/latest.jsonl" -o "$AUDIT_LOG_FILE"

if governance-app audit verify --log-path "$AUDIT_LOG_FILE"; then
    echo "✓ Audit log integrity verified"
else
    echo "✗ Audit log verification failed"
    exit 1
fi

# Verify Nostr events
echo "Verifying Nostr events..."
if nostr-cli --relay "$NOSTR_RELAY" --filter '{"kinds":[30078],"#d":["governance-status"]}' --limit 1; then
    echo "✓ Nostr events verified"
else
    echo "✗ Nostr event verification failed"
    exit 1
fi

echo "All verifications passed!"
```

### Server Verification Script

**verify-server.sh**:
```bash
#!/bin/bash

set -e

SERVER_ID=${1:-"governance-01"}
NOSTR_NPUB=${2:-""}

echo "Verifying server: $SERVER_ID"

# Verify server authorization
if governance-app server verify --server-id "$SERVER_ID"; then
    echo "✓ Server is authorized"
else
    echo "✗ Server is not authorized"
    exit 1
fi

# Verify server operations
if governance-app server operations --server-id "$SERVER_ID" --limit 5; then
    echo "✓ Server operations verified"
else
    echo "✗ Server operations verification failed"
    exit 1
fi

# Verify Nostr events (if NPUB provided)
if [ -n "$NOSTR_NPUB" ]; then
    if nostr-cli --relay wss://relay.damus.io --filter "{\"kinds\":[30078],\"#server\":[\"$SERVER_ID\"]}" --limit 1; then
        echo "✓ Server Nostr events verified"
    else
        echo "✗ Server Nostr events verification failed"
        exit 1
    fi
fi

echo "Server verification completed successfully!"
```

## Dashboard Setup

### Web Dashboard

**Simple HTML Dashboard**:
```html
<!DOCTYPE html>
<html>
<head>
    <title>BTCDecoded Governance Verification</title>
    <script src="https://unpkg.com/nostr-tools@latest/dist/nostr-tools.js"></script>
</head>
<body>
    <h1>Governance System Status</h1>
    
    <div id="nostr-status">
        <h2>Nostr Events</h2>
        <div id="nostr-events"></div>
    </div>
    
    <div id="audit-status">
        <h2>Audit Log</h2>
        <div id="audit-info"></div>
    </div>
    
    <div id="server-status">
        <h2>Server Authorization</h2>
        <div id="servers"></div>
    </div>

    <script>
        // Connect to Nostr relay
        const relay = new NostrRelay('wss://relay.damus.io');
        
        // Subscribe to governance events
        relay.subscribe({
            kinds: [30078],
            '#d': ['governance-status']
        }, (event) => {
            document.getElementById('nostr-events').innerHTML += 
                `<div>${event.content}</div>`;
        });
        
        // Load audit log info
        fetch('/api/audit/status')
            .then(response => response.json())
            .then(data => {
                document.getElementById('audit-info').innerHTML = 
                    `<div>Entries: ${data.entry_count}</div>
                     <div>Head Hash: ${data.head_hash}</div>`;
            });
        
        // Load server status
        fetch('/api/servers')
            .then(response => response.json())
            .then(data => {
                data.servers.forEach(server => {
                    document.getElementById('servers').innerHTML += 
                        `<div>${server.server_id}: ${server.status}</div>`;
                });
            });
    </script>
</body>
</html>
```

## Troubleshooting

### Common Issues

1. **Nostr Connection Failed**
   - Check relay URL is accessible
   - Verify network connectivity
   - Try different relay

2. **OTS Verification Failed**
   - Check OTS server is accessible
   - Verify proof file is valid
   - Check network connectivity

3. **Audit Log Verification Failed**
   - Check log file exists and is readable
   - Verify hash chain integrity
   - Check for corrupted entries

4. **Server Authorization Failed**
   - Check server is in authorized registry
   - Verify server status is active
   - Check Nostr public key matches

### Debug Commands

```bash
# Enable debug logging
export RUST_LOG=debug
governance-app verify --debug

# Check specific component
governance-app nostr test --relay wss://relay.damus.io
governance-app ots test --server https://alice.btc.calendar.opentimestamps.org
governance-app audit test --log-path /var/lib/governance/audit-log.jsonl
```

## References

- [Nostr Integration](NOSTR_INTEGRATION.md)
- [OTS Integration](OTS_INTEGRATION.md)
- [Audit Log System](AUDIT_LOG_SYSTEM.md)
- [Server Authorization](SERVER_AUTHORIZATION.md)
- [Configuration Reference](CONFIGURATION.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
