# BTCDecoded Governance App

GitHub App for cryptographic governance enforcement across BTCDecoded repositories.

## Overview

This Rust-based GitHub App enforces the BTCDecoded governance system by:

- Validating signature requirements by layer (6-of-7 for constitutional, 4-of-5 for implementation, etc.)
- Enforcing review periods (180 days for constitutional, 90 days for implementation, 365 days for consensus changes)
- Managing three-tiered emergency response system
- Detecting consensus rule changes and applying advisory user signaling guidance
- Verifying cross-layer dependencies
- Posting status checks to GitHub
- Blocking merges until governance requirements are met

## Key Features

### Layered Governance

- **Constitutional (Layers 1-2)**: 6-of-7 signatures, 180-day review (365 for consensus)
- **Implementation (Layer 3)**: 4-of-5 signatures, 90-day review
- **Application (Layer 4)**: 3-of-5 signatures, 60-day review
- **Extension (Layer 5)**: 2-of-3 signatures, 14-day review

### Emergency Tier System

Three-tiered response for critical issues:

- **Tier 1 (Critical)**: 0-day review, 4-of-7 signatures, 7-day max duration
  - Network-threatening: inflation bugs, consensus forks, P2P DoS
  - No extensions, post-mortem + security audit required
  
- **Tier 2 (Urgent)**: 7-day review, 5-of-7 signatures, 30-day max duration
  - Serious security: memory corruption, privacy leaks, crashes
  - 1 extension allowed, post-mortem required
  
- **Tier 3 (Elevated)**: 30-day review, 6-of-7 signatures, 90-day max duration
  - Important priorities: bug fixes, competitive response
  - 2 extensions allowed, post-mortem required

All emergency tiers require 5-of-7 emergency keyholders to activate.

### Consensus Change Detection

The app detects consensus rule changes in:
- `consensus-rules/**`
- `validation/**`
- `block-acceptance/**`
- `proofs/**`

When detected, applies:
- 365-day review period (1 year)
- 6-of-7 maintainer requirement
- Requires BIP specification, test vectors, security audit, equivalence proof
- Adds user signaling guidance (advisory, not enforced)

## Architecture

The app consists of several key modules:

- **Webhooks**: Handle GitHub webhook events (PRs, reviews, comments, pushes)
- **Validation**: Verify signatures, review periods, and cross-layer rules
- **Enforcement**: Generate status checks and block merges
- **Database**: Store PR tracking, maintainer keys, and audit logs
- **Crypto**: Handle secp256k1 signature verification
- **GitHub**: Interface with GitHub API

## Setup

### Prerequisites

- PostgreSQL database
- GitHub App credentials
- Rust 1.70+

### Configuration

1. Copy `env.example` to `.env` and configure:
   ```bash
   cp env.example .env
   ```

2. Set up PostgreSQL database:
   ```sql
   CREATE DATABASE governance;
   ```

3. Configure GitHub App:
   - Create GitHub App in your organization
   - Download private key
   - Set webhook URL to your deployment
   - Configure webhook events: `pull_request`, `pull_request_review`, `issue_comment`, `push`

### Running

```bash
# Development
cargo run

# Production
cargo build --release
./target/release/governance-app
```

## Configuration

The app reads configuration from:

1. Environment variables (see `env.example`)
2. `config/app.toml` for default values
3. Database for governance rules (cached from governance repo)

## Database Schema

The app uses PostgreSQL with the following key tables:

- `pull_requests`: Track PRs and their governance status
- `maintainers`: Store maintainer public keys by layer
- `emergency_keyholders`: Emergency keyholders for crisis situations
- `emergency_tiers`: Active emergency tier tracking
- `emergency_activation_votes`: Keyholder votes for emergency activation
- `emergency_extensions`: Extension requests and approvals
- `emergency_audit_log`: Immutable emergency event log
- `governance_events`: Immutable audit log for all governance actions
- `cross_layer_rules`: Cross-repository validation rules

## API Endpoints

- `GET /health`: Health check
- `POST /webhooks/github`: GitHub webhook handler

## Validation Modules

The app includes specialized validation for:

- **Signatures**: Cryptographic verification using secp256k1
- **Review Periods**: Time-based requirements with emergency adjustments
- **Thresholds**: N-of-M signature requirements by layer
- **Cross-Layer**: Bidirectional synchronization rules
- **Emergency**: Tier activation, expiration, and extension logic

## Enforcement Modules

Status checks posted to GitHub PRs:

- Review period status (days elapsed, days remaining)
- Signature collection (N-of-M, who signed, who pending)
- Emergency tier status (if active)
- Cross-layer dependency validation
- Merge blocking until requirements met

## Emergency Response Workflow

1. **Discovery**: Vulnerability found and reported
2. **Activation**: 5-of-7 emergency keyholders sign activation request
3. **Tier Applied**: App adjusts review period and signature threshold
4. **Fix Merged**: Expedited process with appropriate safeguards
5. **Expiration**: Automatic after max duration
6. **Post-Mortem**: Required documentation and audit

See [Emergency Response Guide](../governance/examples/emergency-response.md) for detailed workflows.

## Consensus Change Workflow

1. **Detection**: App detects consensus-affecting changes via path patterns
2. **Requirements**: 365-day review, 6-of-7 signatures, BIP/audit/proof required
3. **Repository Approval**: Code enters BTCDecoded repositories
4. **Release**: Published as optional upgrade
5. **User Signaling**: Advisory guidance for network adoption (75% nodes, 90% hashpower)
6. **User Decision**: Network adopts (or not) based on voluntary coordination

**Key Distinction:** App enforces repository approval (binding), provides guidance for network adoption (advisory).

See [Consensus Change Workflow](../governance/examples/consensus-change-workflow.md) for detailed process.

## Governance Scope

**What This App Controls (Binding):**
- Repository merge access
- Maintainer signature requirements
- Review period enforcement
- GitHub status check blocking

**What This App Doesn't Control (Advisory):**
- Bitcoin network consensus rules
- User adoption decisions
- Node operator choices
- Protocol activation

See [SCOPE.md](../governance/SCOPE.md) for detailed clarification.

## Security

- All consensus-critical dependencies pinned to exact versions
- secp256k1 signature verification (Bitcoin-compatible)
- Immutable audit logs for all governance events
- Emergency tier safeguards (automatic expiration, post-mortem requirements)
- No consensus rule modifications allowed in application layers
- Transparent logging of all governance decisions

## Monitoring

The app provides:

- Health check endpoint (`/health`)
- Emergency expiration monitoring (automatic deactivation)
- Post-mortem deadline tracking
- Security audit requirement tracking
- Governance event audit trail

## Development

```bash
# Run tests
cargo test

# Run with coverage
cargo tarpaulin --out Html

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## Deployment

Recommended deployment:
- Docker container or systemd service
- PostgreSQL database (backed up regularly)
- Reverse proxy (nginx/caddy) for TLS
- Monitoring and alerting for emergency expirations
- Regular security updates

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Related Documentation

- [Governance Configuration](../governance/README.md)
- [Full Governance Process](../governance/GOVERNANCE.md)
- [Scope Clarification](../governance/SCOPE.md)
- [Consensus Change Workflow](../governance/examples/consensus-change-workflow.md)
- [Emergency Response Guide](../governance/examples/emergency-response.md)

## License

MIT License - see LICENSE file for details.



