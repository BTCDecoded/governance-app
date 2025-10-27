# OpenTimestamps Integration

## Overview

The BTCDecoded governance system uses OpenTimestamps (OTS) to anchor governance registries to the Bitcoin blockchain. This provides cryptographic proof that governance state existed at specific points in time, creating an immutable historical record.

## Purpose

OpenTimestamps integration serves as a **temporal proof mechanism** by:
- Anchoring governance registries to Bitcoin blockchain
- Providing cryptographic proof of governance state
- Creating immutable historical records
- Enabling verification of governance timeline

## Architecture

### Monthly Registry Anchoring

**Anchoring Schedule**:
- **Frequency**: Monthly on the 1st day of each month
- **Content**: Complete governance registry snapshot
- **Proof**: OpenTimestamps proof anchored to Bitcoin
- **Storage**: Local proof files and public registry

**Registry Structure**:
```json
{
  "version": "2024-01",
  "timestamp": "2024-01-15T10:30:00Z",
  "previous_registry_hash": "sha256:abc123...",
  "maintainers": [
    {
      "id": "maintainer_1",
      "name": "Alice Smith",
      "public_key": "ed25519_public_key_hex",
      "jurisdiction": "United States",
      "status": "active"
    }
  ],
  "authorized_servers": [
    {
      "server_id": "governance-01",
      "operator": {
        "name": "BTCDecoded Foundation",
        "jurisdiction": "United States"
      },
      "keys": {
        "nostr_npub": "npub1...",
        "ssh_fingerprint": "SHA256:..."
      },
      "status": "active"
    }
  ],
  "audit_logs": {
    "2024-01": {
      "head_hash": "sha256:def456...",
      "entry_count": 1500,
      "merkle_root": "sha256:ghi789..."
    }
  },
  "multisig_config": {
    "required_signatures": 3,
    "total_maintainers": 5
  }
}
```

### Proof Generation

**OTS Proof Format**:
- **Format**: Binary OpenTimestamps proof
- **Extension**: `.json.ots` (e.g., `2024-01.json.ots`)
- **Content**: Cryptographic proof of registry existence
- **Verification**: Can be verified against Bitcoin blockchain

## Configuration

### Server Configuration

**Environment Variables**:
```bash
# Enable OTS integration
OTS_ENABLED=true

# OTS aggregator URL
OTS_AGGREGATOR_URL=https://alice.btc.calendar.opentimestamps.org

# Monthly anchor day (1-31)
OTS_MONTHLY_ANCHOR_DAY=1

# Registry storage path
OTS_REGISTRY_PATH=/var/lib/governance/registries

# Proof storage path
OTS_PROOFS_PATH=/var/lib/governance/ots-proofs
```

**Configuration File**:
```toml
[ots]
enabled = true
aggregator_url = "https://alice.btc.calendar.opentimestamps.org"
monthly_anchor_day = 1
registry_path = "/var/lib/governance/registries"
proofs_path = "/var/lib/governance/ots-proofs"
```

### Directory Structure

**Registry Directory**:
```
/var/lib/governance/registries/
├── 2024-01.json
├── 2024-02.json
├── 2024-03.json
└── ...
```

**Proof Directory**:
```
/var/lib/governance/ots-proofs/
├── 2024-01.json.ots
├── 2024-02.json.ots
├── 2024-03.json.ots
└── ...
```

## Registry Generation

### Monthly Registry Creation

**Registry Generation Process**:
1. **Data Collection**: Gather all governance data
2. **Hash Calculation**: Calculate registry hash
3. **OTS Submission**: Submit to OpenTimestamps
4. **Proof Storage**: Store OTS proof locally
5. **Registry Storage**: Store registry locally
6. **Public Publication**: Make registry publicly available

**Programmatic Generation**:
```rust
use crate::ots::{OtsClient, RegistryAnchorer};

async fn generate_monthly_registry() -> Result<(), Error> {
    let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
    let anchorer = RegistryAnchorer::new(
        ots_client,
        database,
        "/var/lib/governance/registries".to_string(),
        "/var/lib/governance/ots-proofs".to_string(),
    );
    
    // Generate and anchor registry
    anchorer.anchor_registry().await?;
    
    Ok(())
}
```

### Registry Content

**Maintainer Registry**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Maintainer {
    pub id: String,
    pub name: String,
    pub public_key: String,
    pub jurisdiction: String,
    pub contact: Option<String>,
    pub status: String,
    pub added_at: DateTime<Utc>,
    pub last_verified: Option<DateTime<Utc>>,
}
```

**Authorized Server Registry**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedServer {
    pub server_id: String,
    pub operator: OperatorInfo,
    pub keys: ServerKeys,
    pub infrastructure: InfrastructureInfo,
    pub status: String,
    pub added_at: DateTime<Utc>,
    pub last_verified: Option<DateTime<Utc>>,
}
```

**Audit Log Summary**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogSummary {
    pub head_hash: String,
    pub entry_count: u64,
    pub merkle_root: String,
    pub first_entry: DateTime<Utc>,
    pub last_entry: DateTime<Utc>,
}
```

## Proof Verification

### Manual Verification

**Using ots-cli**:
```bash
# Install ots-cli
pip install opentimestamps-client

# Verify a proof
ots verify /var/lib/governance/ots-proofs/2024-01.json.ots

# Get Bitcoin block height
ots info /var/lib/governance/ots-proofs/2024-01.json.ots
```

**Using ots-tools**:
```bash
# Install ots-tools
cargo install ots-tools

# Verify proof
ots-tools verify /var/lib/governance/ots-proofs/2024-01.json.ots

# Get verification details
ots-tools info /var/lib/governance/ots-proofs/2024-01.json.ots
```

### Programmatic Verification

**Verify OTS Proof**:
```rust
use crate::ots::client::{OtsClient, VerificationResult};

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

**Verify Registry Chain**:
```rust
use crate::ots::verify::verify_registry_chain;

async fn verify_registry_chain(registry_dir: &str) -> Result<(), Error> {
    let chain_verification = verify_registry_chain(registry_dir).await?;
    
    for result in chain_verification {
        println!("Registry verification result: {}", result);
    }
    
    Ok(())
}
```

## Registry Chain Verification

### Chain Integrity

**Hash Chain Verification**:
```rust
use sha2::{Digest, Sha256};

fn verify_registry_chain(registries: &[GovernanceRegistry]) -> Result<(), Error> {
    for i in 1..registries.len() {
        let current = &registries[i];
        let previous = &registries[i-1];
        
        // Calculate previous registry hash
        let prev_data = serde_json::to_vec(previous)?;
        let mut hasher = Sha256::new();
        hasher.update(&prev_data);
        let expected_hash = format!("sha256:{}", hex::encode(hasher.finalize()));
        
        // Verify hash chain
        if current.previous_registry_hash != expected_hash {
            return Err(Error::BrokenRegistryChain);
        }
    }
    
    Ok(())
}
```

### Merkle Tree Verification

**Audit Log Merkle Trees**:
```rust
use crate::audit::merkle::{build_merkle_tree, verify_merkle_root};

async fn verify_audit_log_merkle(log_path: &str, claimed_root: &str) -> Result<bool, Error> {
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

## Public Verification

### Registry Download

**Download Latest Registry**:
```bash
# Download registry
curl -o registry.json https://btcdecoded.org/governance/registries/2024-01.json

# Download proof
curl -o registry.json.ots https://btcdecoded.org/governance/ots-proofs/2024-01.json.ots

# Verify proof
ots verify registry.json.ots
```

**Programmatic Download**:
```rust
use reqwest;

async fn download_and_verify_registry(month: &str) -> Result<(), Error> {
    let registry_url = format!("https://btcdecoded.org/governance/registries/{}.json", month);
    let proof_url = format!("https://btcdecoded.org/governance/ots-proofs/{}.json.ots", month);
    
    // Download registry
    let registry_response = reqwest::get(&registry_url).await?;
    let registry_data = registry_response.bytes().await?;
    
    // Download proof
    let proof_response = reqwest::get(&proof_url).await?;
    let proof_data = proof_response.bytes().await?;
    
    // Verify proof
    let ots_client = OtsClient::new("https://alice.btc.calendar.opentimestamps.org".to_string());
    match ots_client.verify(&registry_data, &proof_data).await? {
        VerificationResult::Confirmed(block_height) => {
            println!("Registry {} verified in Bitcoin block {}", month, block_height);
        }
        _ => {
            println!("Registry {} verification failed", month);
        }
    }
    
    Ok(())
}
```

### Verification Script

**Complete Verification Script**:
```bash
#!/bin/bash

set -e

MONTH=${1:-$(date +%Y-%m)}
REGISTRY_URL="https://btcdecoded.org/governance/registries"
PROOF_URL="https://btcdecoded.org/governance/ots-proofs"

echo "Verifying governance registry for $MONTH..."

# Download registry and proof
REGISTRY_FILE=$(mktemp)
PROOF_FILE=$(mktemp)

curl -s "$REGISTRY_URL/$MONTH.json" -o "$REGISTRY_FILE"
curl -s "$PROOF_URL/$MONTH.json.ots" -o "$PROOF_FILE"

# Verify OTS proof
if ots verify "$PROOF_FILE"; then
    echo "✓ Registry anchored to Bitcoin"
else
    echo "✗ Registry verification failed"
    exit 1
fi

# Verify registry structure
if governance-app registry verify --registry "$REGISTRY_FILE"; then
    echo "✓ Registry structure verified"
else
    echo "✗ Registry structure verification failed"
    exit 1
fi

# Verify audit log merkle root
AUDIT_ROOT=$(jq -r '.audit_logs."'$MONTH'".merkle_root' "$REGISTRY_FILE")
if governance-app audit verify-merkle --log-path /var/lib/governance/audit-log.jsonl --merkle-root "$AUDIT_ROOT"; then
    echo "✓ Audit log merkle root verified"
else
    echo "✗ Audit log merkle root verification failed"
    exit 1
fi

echo "All verifications passed!"
```

## Self-Hosted OTS Server

### Setup (Optional)

**Install OTS Server**:
```bash
# Clone OTS server
git clone https://github.com/opentimestamps/opentimestamps-server.git
cd opentimestamps-server

# Install dependencies
pip install -r requirements.txt

# Configure server
cp config.example.py config.py
nano config.py
```

**Configuration**:
```python
# config.py
CALENDAR_SERVER = {
    'host': '0.0.0.0',
    'port': 8080,
    'db_path': '/var/lib/ots/calendar.db',
    'bitcoin_node': {
        'host': 'localhost',
        'port': 8332,
        'user': 'bitcoin',
        'password': 'password'
    }
}
```

**Start Server**:
```bash
# Start OTS server
python calendar-server.py

# Test server
curl -X POST http://localhost:8080/stamp \
  -H "Content-Type: application/octet-stream" \
  --data-binary @test.txt
```

### Integration

**Use Self-Hosted Server**:
```bash
# Update configuration
export OTS_AGGREGATOR_URL=http://localhost:8080
```

**Configuration File**:
```toml
[ots]
enabled = true
aggregator_url = "http://localhost:8080"
monthly_anchor_day = 1
registry_path = "/var/lib/governance/registries"
proofs_path = "/var/lib/governance/ots-proofs"
```

## Monitoring and Maintenance

### Registry Monitoring

**Check Registry Status**:
```bash
# List all registries
ls -la /var/lib/governance/registries/

# Check latest registry
governance-app registry status --latest

# Verify all registries
governance-app registry verify-all
```

**Programmatic Monitoring**:
```rust
use crate::ots::anchor::RegistryAnchorer;

async fn monitor_registry_status() -> Result<(), Error> {
    let anchorer = RegistryAnchorer::new(/* ... */);
    
    // Check if monthly anchoring is due
    let now = Utc::now();
    if now.day() == 1 {
        // Perform monthly anchoring
        anchorer.anchor_registry().await?;
    }
    
    // Check registry health
    let registries = anchorer.list_registries().await?;
    for registry in registries {
        if !anchorer.verify_registry(&registry).await? {
            println!("Registry {} verification failed", registry.version);
        }
    }
    
    Ok(())
}
```

### Proof Maintenance

**Clean Old Proofs**:
```bash
# Keep only last 12 months of proofs
find /var/lib/governance/ots-proofs/ -name "*.ots" -mtime +365 -delete
find /var/lib/governance/registries/ -name "*.json" -mtime +365 -delete
```

**Backup Proofs**:
```bash
# Backup proofs to remote storage
rsync -av /var/lib/governance/ots-proofs/ backup-server:/backups/governance/ots-proofs/
rsync -av /var/lib/governance/registries/ backup-server:/backups/governance/registries/
```

## Troubleshooting

### Common Issues

1. **OTS Server Unavailable**
   - Check OTS server is running
   - Verify network connectivity
   - Try different OTS server

2. **Proof Verification Failed**
   - Check proof file is valid
   - Verify data matches proof
   - Check OTS server status

3. **Registry Generation Failed**
   - Check database connectivity
   - Verify file permissions
   - Check disk space

### Debug Commands

```bash
# Test OTS server connectivity
governance-app ots test --server https://alice.btc.calendar.opentimestamps.org

# Generate test proof
governance-app ots stamp --data test.txt --output test.txt.ots

# Verify test proof
governance-app ots verify --data test.txt --proof test.txt.ots
```

### Log Analysis

```bash
# Check OTS logs
sudo journalctl -u governance-app | grep ots

# Check registry generation
sudo journalctl -u governance-app | grep "registry.*generated"

# Check proof verification
sudo journalctl -u governance-app | grep "proof.*verified"
```

## Best Practices

### Registry Management

1. **Regular Anchoring**: Anchor registries monthly
2. **Backup Proofs**: Keep backups of all proofs
3. **Verify Regularly**: Verify proofs regularly
4. **Monitor Health**: Monitor OTS server health

### Proof Management

1. **Store Securely**: Store proofs in secure locations
2. **Verify Integrity**: Verify proof integrity regularly
3. **Keep History**: Maintain historical proof chain
4. **Public Access**: Make proofs publicly accessible

### Security

1. **Server Security**: Secure OTS server if self-hosting
2. **Proof Integrity**: Verify proof integrity
3. **Access Control**: Control access to proof files
4. **Audit Trail**: Maintain audit trail of proof operations

## References

- [OpenTimestamps Protocol](https://opentimestamps.org/)
- [OpenTimestamps Server](https://github.com/opentimestamps/opentimestamps-server)
- [Verification Guide](VERIFICATION.md)
- [Configuration Reference](CONFIGURATION.md)
- [Troubleshooting Guide](TROUBLESHOOTING.md)
