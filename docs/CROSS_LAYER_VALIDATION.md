# Cross-Layer Validation Documentation

## Overview

This document provides comprehensive documentation for the cross-layer validation system implemented in the governance-app. The system ensures cryptographic synchronization between the Orange Paper (Layer 1) and Consensus Proof (Layer 2) repositories.

## System Architecture

### Components

1. **Content Hash Verification** (`content_hash.rs`)
2. **Version Pinning Validation** (`version_pinning.rs`)
3. **Equivalence Proof Validation** (`equivalence_proof.rs`)
4. **Cross-Layer Status Checks** (`cross_layer_status.rs`)
5. **GitHub Integration** (`cross_layer.rs`)

### Data Flow

```
GitHub PR → Cross-Layer Validator → Status Check Generator → GitHub Status API
    ↓
Content Hash Verification
    ↓
Version Pinning Validation
    ↓
Equivalence Proof Validation
    ↓
Combined Status Check
```

## Content Hash Verification

### Purpose

Ensures file correspondence between Orange Paper and Consensus Proof through cryptographic hash verification.

### Key Functions

#### `compute_file_hash(content: &[u8]) -> String`

Computes SHA256 hash of file content.

```rust
let content = "Hello, world!".as_bytes();
let hash = ContentHashValidator::compute_file_hash(content);
// Returns: "sha256:c0535e4be2b79ffd93291305436bf889314e4a3faec05ecffcbb7df31ad9e51a"
```

#### `compute_directory_hash(files: &HashMap<String, String>) -> String`

Computes Merkle tree hash of directory structure.

```rust
let mut files = HashMap::new();
files.insert("file1.txt".to_string(), "content1".to_string());
files.insert("file2.txt".to_string(), "content2".to_string());
let hash = ContentHashValidator::compute_directory_hash(&files);
```

#### `verify_correspondence(...) -> Result<bool, GovernanceError>`

Verifies correspondence between Orange Paper and Consensus Proof files.

```rust
let is_synced = validator.verify_correspondence(
    "consensus-rules/block-validation.md",
    "Block validation rules",
    &consensus_proof_files,
)?;
```

### File Correspondence Mapping

The system maintains a mapping between Orange Paper files and their corresponding Consensus Proof files:

```rust
pub struct FileCorrespondence {
    pub orange_paper_file: String,
    pub consensus_proof_file: String,
    pub correspondence_type: CorrespondenceType,
}

pub enum CorrespondenceType {
    Direct,      // 1:1 mapping
    OneToMany,   // 1:N mapping
    ManyToOne,   // N:1 mapping
    Custom,      // Custom mapping logic
}
```

### Usage Example

```rust
use governance_app::validation::content_hash::ContentHashValidator;

let mut validator = ContentHashValidator::new();
let correspondence_mappings = ContentHashValidator::generate_correspondence_map();
validator.load_correspondence_mappings(correspondence_mappings);

let changed_files = vec!["consensus-rules/block-validation.md".to_string()];
let orange_files = HashMap::new();
let consensus_proof_files = HashMap::new();

let sync_report = validator.check_bidirectional_sync(
    &orange_files,
    &consensus_proof_files,
    &changed_files,
)?;
```

## Version Pinning Validation

### Purpose

Ensures Consensus Proof implementations reference specific, cryptographically verified Orange Paper versions.

### Key Functions

#### `parse_version_references(file_path: &str, content: &str) -> Vec<VersionReference>`

Parses version references from file content.

```rust
let content = r#"
// @orange-paper-version: v1.2.3
// @orange-paper-commit: abc123def456
// @orange-paper-hash: sha256:fedcba...
pub fn validate_block() -> bool { true }
"#;

let references = validator.parse_version_references("src/validation.rs", content);
```

#### `verify_version_reference(reference: &VersionReference) -> Result<(), GovernanceError>`

Verifies a single version reference against the loaded manifest.

```rust
let reference = VersionReference {
    file_path: "src/validation.rs:3".to_string(),
    orange_paper_version: "v1.2.3".to_string(),
    orange_paper_commit: "abc123def456".to_string(),
    orange_paper_hash: "sha256:fedcba...".to_string(),
};

validator.verify_version_reference(&reference)?;
```

### Version Reference Format

Version references must follow this format in code comments:

```rust
// @orange-paper-version: v1.2.3
// @orange-paper-commit: abc123def456
// @orange-paper-hash: sha256:fedcba...
```

### Version Manifest

The system uses a cryptographically signed version manifest:

```yaml
repository: orange-paper
created_at: "2023-10-27T10:00:00Z"
versions:
  - version: v1.0.0
    commit_sha: a1b2c3d4e5f6789012345678901234567890abcd
    content_hash: sha256:1234567890abcdef...
    created_at: "2023-10-26T10:00:00Z"
    signatures:
      - maintainer_id: maintainer1
        signature: test_signature_1
        public_key: test_public_key_1
        signed_at: "2023-10-26T10:05:00Z"
      # ... 6 signatures total
    ots_timestamp: bitcoin:test_timestamp_v1.0.0
    is_stable: true
    is_latest: true
latest_version: v1.0.0
manifest_hash: sha256:test_manifest_hash
```

### Usage Example

```rust
use governance_app::validation::version_pinning::VersionPinningValidator;

let mut validator = VersionPinningValidator::default();
let manifest = load_version_manifest()?;
validator.load_version_manifest(manifest)?;

let references = validator.parse_version_references("src/validation.rs", content);
for reference in references {
    validator.verify_version_reference(&reference)?;
}
```

## Equivalence Proof Validation

### Purpose

Validates that Consensus Proof implementations mathematically match Orange Paper specifications through test vector validation.

### Key Functions

#### `generate_consensus_test_vectors() -> Vec<EquivalenceTestVector>`

Generates test vectors for common consensus operations.

```rust
let test_vectors = EquivalenceProofValidator::generate_consensus_test_vectors();
```

#### `verify_equivalence_proof(test_id: &str) -> Result<VerificationResult, GovernanceError>`

Verifies a single equivalence proof.

```rust
let result = validator.verify_equivalence_proof("block_validation_001")?;
```

### Test Vector Structure

```rust
pub struct EquivalenceTestVector {
    pub test_id: String,
    pub description: String,
    pub orange_paper_spec: String,
    pub consensus_proof_impl: String,
    pub expected_result: String,
    pub test_data: HashMap<String, String>,
    pub proof_metadata: ProofMetadata,
}
```

### Verification Rules

```rust
pub struct VerificationRules {
    pub require_behavioral_equivalence: bool,
    pub require_performance_equivalence: bool,
    pub require_security_equivalence: bool,
    pub max_performance_variance: f64,
    pub security_property_checks: Vec<String>,
}
```

### Usage Example

```rust
use governance_app::validation::equivalence_proof::EquivalenceProofValidator;

let mut validator = EquivalenceProofValidator::new();
let test_vectors = EquivalenceProofValidator::generate_consensus_test_vectors();
validator.load_test_vectors(test_vectors);

let result = validator.verify_equivalence_proof("block_validation_001")?;
println!("Verification result: {:?}", result.overall_status);
```

## Cross-Layer Status Checks

### Purpose

Generates comprehensive GitHub status checks for cross-layer validation.

### Key Functions

#### `generate_cross_layer_status(...) -> Result<CrossLayerStatusCheck, GovernanceError>`

Generates comprehensive status check for a PR.

```rust
let status_check = status_checker.generate_cross_layer_status(
    "owner",
    "repo",
    123,
    &changed_files,
).await?;
```

### Status Check Structure

```rust
pub struct CrossLayerStatusCheck {
    pub state: StatusState,
    pub description: String,
    pub target_url: Option<String>,
    pub context: String,
    pub details: CrossLayerStatusDetails,
}
```

### Status States

- `Success`: All checks passed
- `Failure`: One or more checks failed
- `Pending`: Checks are in progress
- `Error`: System error occurred

### Usage Example

```rust
use governance_app::github::cross_layer_status::CrossLayerStatusChecker;

let mut checker = CrossLayerStatusChecker::new(github_client);
let status = checker.generate_cross_layer_status(
    "owner",
    "repo",
    123,
    &changed_files,
).await?;

println!("Status: {:?}", status.state);
println!("Description: {}", status.description);
```

## GitHub Integration

### Purpose

Integrates cross-layer validation into GitHub PR workflow.

### Key Functions

#### `post_cross_layer_status_check(...) -> Result<(), GovernanceError>`

Posts cross-layer status check to GitHub.

```rust
CrossLayerValidator::post_cross_layer_status_check(
    github_token,
    "owner",
    "repo",
    123,
    &changed_files,
).await?;
```

### Status Check Messages

#### Success
```
✅ Cross-Layer Sync: All 3 files are synchronized
✅ Version Pinning: All 2 references are valid
✅ Equivalence Proof: All 5 tests passed
```

#### Failure
```
❌ Cross-Layer Sync: Missing Consensus Proof updates for 1 files: consensus-rules/block-validation.md
❌ Version Pinning: 1 invalid references found
❌ Equivalence Proof: 2 tests failed
```

### Usage Example

```rust
use governance_app::validation::cross_layer::CrossLayerValidator;

// Post status check to GitHub
CrossLayerValidator::post_cross_layer_status_check(
    github_token,
    "owner",
    "repo",
    123,
    &changed_files,
).await?;
```

## Configuration

### Cross-Layer Rules

Configure cross-layer dependencies in `governance/config/cross-layer-rules.yml`:

```yaml
rules:
  - name: consensus_proof_sync
    description: "Orange Paper and Consensus Proof must stay synchronized"
    source_repo: orange-paper
    source_pattern: consensus-rules/**
    target_repo: consensus-proof
    target_pattern: proofs/**
    validation: corresponding_file_exists
    bidirectional: true
    blocking: true
```

### Repository Configuration

Configure repository-specific rules in `governance/config/repos/`:

```yaml
# orange-paper.yml
layer: 1
governance_level: constitutional
signature_threshold: 6-of-7
review_period_days: 180
synchronized_with:
  - consensus-proof
cross_layer_rules:
  - if_changed: consensus-rules/**
    then_require_update: consensus-proof/proofs/**
    validation: equivalence_proof_exists
    error_message: "Consensus rule changes require corresponding proof updates"
```

## Testing

### Unit Tests

Each module includes comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_verification() {
        // Test content hash computation
        let content = "Hello, world!".as_bytes();
        let hash = ContentHashValidator::compute_file_hash(content);
        assert_eq!(hash, "sha256:c0535e4be2b79ffd93291305436bf889314e4a3faec05ecffcbb7df31ad9e51a");
    }

    #[test]
    fn test_version_pinning_validation() {
        // Test version reference parsing
        let content = "// @orange-paper-version: v1.2.3";
        let references = validator.parse_version_references("test.rs", content);
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].orange_paper_version, "v1.2.3");
    }

    #[test]
    fn test_equivalence_proof_validation() {
        // Test equivalence proof verification
        let result = validator.verify_equivalence_proof("block_validation_001").unwrap();
        assert_eq!(result.overall_status, VerificationStatus::Verified);
    }
}
```

### Integration Tests

Test the complete cross-layer validation workflow:

```rust
#[tokio::test]
async fn test_cross_layer_status_generation() {
    let github_client = GitHubClient::new("test_token".to_string());
    let mut checker = CrossLayerStatusChecker::new(github_client);
    
    let changed_files = vec![
        "consensus-rules/block-validation.md".to_string(),
        "proofs/block-validation.rs".to_string(),
    ];

    let status = checker.generate_cross_layer_status(
        "test_owner",
        "test_repo",
        123,
        &changed_files,
    ).await.unwrap();
    
    assert_eq!(status.context, "cross-layer-sync");
    assert!(status.target_url.is_some());
}
```

### Standalone Tests

Run standalone tests for each component:

```bash
# Test content hash verification
cargo run --bin test-content-hash

# Test version pinning
cargo run --bin test-version-pinning

# Test equivalence proof validation
cargo run --bin test-equivalence-proof

# Test cross-layer integration
cargo run --bin test-cross-layer-integration
```

## Error Handling

### Common Errors

#### Content Hash Mismatch
```
Error: Content hash mismatch for file consensus-rules/block-validation.md
```

**Solution**: Update the corresponding Consensus Proof file to match the Orange Paper changes.

#### Version Reference Invalid
```
Error: Referenced version v1.1.0 not found in manifest
```

**Solution**: Update version references to point to valid Orange Paper versions.

#### Equivalence Test Failure
```
Error: Equivalence test failed for block_validation_001
```

**Solution**: Fix implementation to match specification requirements.

### Error Recovery

1. **Fix the underlying issue** (update files, fix references, etc.)
2. **Re-run validation** to verify the fix
3. **Check status checks** for updated results
4. **Retry merge** once all checks pass

## Performance Optimization

### Caching

- GitHub API responses are cached to reduce API calls
- File hashes are cached to avoid recomputation
- Version manifests are cached in memory

### Parallel Processing

- Multiple validations run in parallel
- File operations are batched
- Status checks are generated concurrently

### Incremental Checking

- Only changed files are validated
- Dependencies are checked incrementally
- Status updates are incremental

## Monitoring

### Metrics

Track key performance indicators:

- Cross-layer sync success rate
- Version pinning compliance rate
- Equivalence proof pass rate
- Average validation time
- GitHub API usage

### Logging

Comprehensive logging for debugging:

```rust
use tracing::{info, warn, error};

info!("Checking content hash synchronization for {} files", changed_files.len());
warn!("Missing corresponding Consensus Proof file: {}", file_path);
error!("Content hash verification failed: {}", error);
```

### Alerting

Set up alerts for:

- Cross-layer sync failures
- Version pinning violations
- Equivalence proof failures
- System performance degradation
- Security incidents

## Troubleshooting

### Common Issues

1. **File Not Found**: Ensure correspondence mapping is correct
2. **Version Mismatch**: Check version manifest and references
3. **Test Failure**: Verify implementation matches specification
4. **API Rate Limit**: Implement proper rate limiting and caching
5. **Permission Denied**: Check GitHub token permissions

### Debug Mode

Enable debug logging:

```bash
RUST_LOG=debug cargo run --bin governance-app
```

### Manual Verification

Use standalone test binaries for manual verification:

```bash
# Test specific functionality
cargo run --bin test-content-hash
cargo run --bin test-version-pinning
cargo run --bin test-equivalence-proof
cargo run --bin test-cross-layer-integration
```

## Security Considerations

### Cryptographic Security

- SHA256 provides strong collision resistance
- 6-of-7 multisig prevents signature forgery
- Version manifest prevents version spoofing
- Test vectors prevent implementation drift

### Access Control

- GitHub tokens have minimal required permissions
- Database access is restricted
- File system access is sandboxed
- Network access is limited to required endpoints

### Audit Trail

- All validation decisions are logged
- Cryptographic proofs are generated
- Status checks are immutable
- Error conditions are tracked

## Conclusion

The cross-layer validation system provides a robust, secure, and scalable solution for maintaining synchronization between Bitcoin's specification and implementation layers. Through comprehensive testing, monitoring, and error handling, it ensures the integrity and security of the Bitcoin governance model.

