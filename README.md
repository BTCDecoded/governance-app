# BTCDecoded Governance App

Rust-based GitHub App for enforcing cryptographic governance rules across all BTCDecoded repositories.

## ⚠️ DEPLOYMENT WARNING

**This application is currently UNRELEASED and UNTESTED in production.**

### Current Status: Phase 1 (Infrastructure Building)

- ✅ **Core Infrastructure**: All major components implemented
- ✅ **Database Schema**: Complete with migrations
- ✅ **Economic Node System**: Registry and veto mechanism
- ✅ **GitHub Integration**: Status checks and merge blocking
- ✅ **Comprehensive Testing**: Full test suite implemented
- ⚠️ **NOT ACTIVATED**: Governance rules are not enforced
- 🔧 **Test Keys Only**: No real cryptographic enforcement

### What This Means

- **Production Quality**: The codebase is well-structured and production-quality
- **Not Battle-Tested**: Has not been tested in real-world production scenarios
- **Rapid Development**: System is in active AI-assisted development
- **Expect Changes**: APIs, interfaces, and behavior may change frequently
- **Use at Your Own Risk**: This is experimental software

## Quick Navigation

### For New Users
- [Getting Started Guide](docs/GETTING_STARTED.md) - Quick setup and first steps
- [Configuration Reference](docs/CONFIGURATION.md) - How to configure the application
- [Verification Guide](docs/VERIFICATION.md) - How to verify system integrity
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md) - Common issues and solutions

### For Developers
- [API Reference](docs/API_REFERENCE.md) - Complete API documentation
- [Development Guide](docs/DEVELOPMENT.md) - Development and contribution guidelines
- [Configuration Integration](docs/CONFIG_INTEGRATION.md) - How app uses governance configs
- [Nostr Integration](docs/NOSTR_INTEGRATION.md) - Real-time transparency system
- [OTS Integration](docs/OTS_INTEGRATION.md) - Bitcoin blockchain anchoring
- [Audit Log System](docs/AUDIT_LOG_SYSTEM.md) - Tamper-evident logging
- [Server Authorization](docs/SERVER_AUTHORIZATION.md) - Server authorization system

### For Administrators
- [Deployment Guide](DEPLOYMENT.md) - Production deployment guide
- [Security Guide](SECURITY.md) - Security configuration and best practices
- [Main Governance Documentation](../governance/README.md) - System overview

### For Auditors
- [Audit Materials](../audit-materials/README.md) - Security and audit information
- [Architecture Documentation](../audit-materials/01-technical/ARCHITECTURE.md) - System design
- [Security Analysis](../audit-materials/02-security/README.md) - Security details

## Architecture

### Core Components

- **Database Layer**: SQLite (development) / PostgreSQL (production) with migrations for governance data
- **Economic Nodes**: Registry and veto signal collection
- **Governance Fork**: Export and adoption tracking
- **GitHub Integration**: Status checks and merge blocking
- **Validation**: Tier classification and requirement checking
- **Enforcement**: Merge blocking and status updates
- **Nostr Integration**: Real-time transparency and communication
- **OpenTimestamps**: Bitcoin blockchain anchoring
- **Audit Log System**: Tamper-evident logging with hash chains
- **Server Authorization**: Authorized server registry and verification

### Key Features

- **Tier Classification**: Automatic PR tier detection using governance config files
- **Signature Collection**: Maintainer signature verification
- **Economic Node Veto**: Mining pool and exchange veto signals
- **Status Checks**: Detailed GitHub status reporting
- **Merge Blocking**: Prevents merging until requirements met
- **Audit Logging**: Complete governance event tracking with hash chains
- **Nostr Publishing**: Real-time status updates via Nostr protocol
- **Bitcoin Anchoring**: Monthly registry anchoring via OpenTimestamps
- **Server Authorization**: Explicit authorization of governance servers
- **Configuration Integration**: Loads and validates governance repository configs

## Development Setup

### Prerequisites

- Rust 1.70+
- SQLite3
- Git

### Installation

```bash
# Clone the repository
git clone https://github.com/btcdecoded/governance-system.git
cd governance-system/governance-app

# Install dependencies
cargo build

# Run tests
cargo test

# Build verification tools
cargo build --bin verify-audit-log
```

### Configuration

Create a `.env` file in the governance-app directory:

```bash
# Database
DATABASE_URL=sqlite:governance.db

# GitHub App (Test Keys Only)
GITHUB_APP_ID=12345
GITHUB_PRIVATE_KEY_PATH=/path/to/test-private-key.pem
GITHUB_WEBHOOK_SECRET=test-webhook-secret

# Governance Configuration
GOVERNANCE_CONFIG_PATH=../governance
```

### Running Locally

```bash
# Development mode
cargo run

# With logging
RUST_LOG=debug cargo run
```

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suites

```bash
# Economic node tests
cargo test --test economic_nodes_test

# Governance fork tests
cargo test --test governance_fork_test

# GitHub integration tests
cargo test --test github_integration_test

# End-to-end tests
cargo test --test e2e_test
```

### Test Coverage

The test suite includes:

- **Unit Tests**: Individual component functionality
- **Integration Tests**: Component interaction testing
- **End-to-End Tests**: Complete workflow testing
- **Error Handling Tests**: Edge cases and error conditions
- **Performance Tests**: System performance validation

## API Endpoints

### Webhook Endpoints

- `POST /webhooks/github` - GitHub webhook handler
- `POST /webhooks/pull-request` - Pull request events
- `POST /webhooks/issue-comment` - Comment events

### Governance Endpoints

- `GET /governance/status` - System status
- `GET /governance/events` - Governance events
- `POST /governance/sign` - Signature submission
- `GET /governance/nodes` - Economic node registry

### Fork Endpoints

- `GET /fork/export` - Export governance configuration
- `GET /fork/adoption` - Adoption metrics
- `POST /fork/decision` - Fork decision submission

## Database Schema

### Core Tables

- `pull_requests` - PR tracking and status
- `signatures` - Maintainer signatures
- `economic_nodes` - Economic node registry
- `veto_signals` - Veto signal collection
- `governance_events` - Audit log
- `governance_rulesets` - Fork rulesets
- `fork_decisions` - Node fork decisions

### Migrations

```bash
# Run migrations
cargo run --bin migrate

# Create new migration
cargo run --bin migrate -- create migration_name
```

## GitHub Integration

### Status Checks

The app posts detailed status checks to GitHub PRs:

- **Review Period**: Time remaining and requirements
- **Signatures**: Current signature count and required signatures
- **Economic Veto**: Veto status and threshold information
- **Combined Status**: Overall governance status

### Merge Blocking

- **Automatic Blocking**: PRs are blocked until requirements met
- **Status Updates**: Real-time status updates as requirements are met
- **Merge Enablement**: Automatic merge enablement when all requirements satisfied

### Webhook Events

- **Pull Request**: Opened, updated, closed events
- **Issue Comment**: Signature collection via comments
- **Push**: Branch updates and force pushes

## Verification Tools

### Audit Log Verification

```bash
# Verify audit log integrity
cargo run --bin verify-audit-log -- --log-path /var/lib/governance/audit-log.jsonl

# Verify with expected Merkle root
cargo run --bin verify-audit-log -- --log-path /var/lib/governance/audit-log.jsonl --merkle-root "sha256:abc123..."

# Verbose output
cargo run --bin verify-audit-log -- --log-path /var/lib/governance/audit-log.jsonl --verbose
```

### Server Verification

```bash
# Verify server authorization
./scripts/verify-server.sh governance-01

# Verify with Nostr public key
./scripts/verify-server.sh governance-01 npub1abc123...
```

### Integration Verification

```bash
# Verify complete system integration
./scripts/verify-integration.sh
```

### Nostr Event Verification

```bash
# Subscribe to governance events
nostr-cli --relay wss://relay.damus.io --filter '{"kinds":[30078],"#d":["governance-status"]}'

# Verify specific server events
nostr-cli --relay wss://relay.damus.io --filter '{"kinds":[30078],"#server":["governance-01"]}'
```

### OTS Proof Verification

```bash
# Verify OTS proof
ots verify /var/lib/governance/ots-proofs/2024-01.json.ots

# Get Bitcoin block height
ots info /var/lib/governance/ots-proofs/2024-01.json.ots
```

## Economic Node System

### Node Types

- **Mining Pool**: Hashpower-based qualification
- **Exchange**: Volume and holdings-based qualification
- **Custodian**: Holdings-based qualification
- **Payment Processor**: Volume-based qualification
- **Major Holder**: Holdings-based qualification

### Veto Mechanism

- **Signal Collection**: Nodes can submit veto, support, or abstain signals
- **Threshold Calculation**: 30%+ hashpower or 40%+ economic activity
- **Weight Calculation**: Dynamic weight based on qualification data
- **Verification**: Cryptographic signature verification

## Governance Fork System

### Configuration Export

- **Complete Export**: All governance configuration in single YAML
- **Versioning**: Semantic versioning for rulesets
- **Hash Verification**: Cryptographic hash for integrity
- **Metadata**: Export metadata and provenance

### Adoption Tracking

- **Node Decisions**: Track node adoption decisions
- **Metrics Calculation**: Adoption percentages and statistics
- **Dashboard**: Real-time adoption metrics
- **History**: Adoption decision history

## Security Considerations

### Current Limitations

- **Test Keys Only**: All cryptographic operations use test keys
- **Not Audited**: Code has not undergone security audit
- **Experimental**: New cryptographic governance model
- **Untested**: Not tested in adversarial conditions

### Security Features

- **Signature Verification**: All signatures are cryptographically verified
- **Audit Logging**: Complete audit trail of all governance actions
- **Access Control**: Role-based access to governance functions
- **Rate Limiting**: Protection against abuse and spam

## Monitoring and Logging

### Logging

- **Structured Logging**: JSON-formatted logs
- **Log Levels**: Debug, info, warn, error
- **Context**: Rich context information in logs
- **Correlation**: Request correlation IDs

### Metrics

- **Performance Metrics**: Response times and throughput
- **Governance Metrics**: Signature collection rates
- **Error Metrics**: Error rates and types
- **System Metrics**: Resource usage and health

## Deployment

### Development Deployment

```bash
# Build for development
cargo build

# Run with development configuration
cargo run --bin governance-app
```

### Production Deployment (Phase 2+)

**⚠️ DO NOT DEPLOY IN PRODUCTION UNTIL PHASE 2 ACTIVATION**

When Phase 2 is activated, deployment will include:

- Production key management
- Security audit completion
- Battle testing in production
- Community validation

## Troubleshooting

### Common Issues

1. **Database Connection**: Check DATABASE_URL configuration
2. **GitHub API**: Verify GITHUB_APP_ID and private key
3. **Webhook Delivery**: Check webhook secret and endpoint
4. **Signature Verification**: Verify key format and permissions

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable trace logging
RUST_LOG=trace cargo run
```

### Health Checks

```bash
# Check system health
curl http://localhost:3000/health

# Check governance status
curl http://localhost:3000/governance/status
```

## Contributing

### Development Guidelines

1. **Read the Documentation**: Understand the system architecture
2. **Run Tests**: Ensure all tests pass before submitting
3. **Follow Conventions**: Use consistent coding style
4. **Document Changes**: Update documentation for new features
5. **Test Thoroughly**: Add tests for new functionality

### Code Review Process

1. **Create Pull Request**: Submit changes via GitHub
2. **Run Tests**: Ensure all tests pass
3. **Code Review**: Maintainers review the changes
4. **Merge**: Changes are merged after approval

## Support

### Getting Help

- **GitHub Issues**: Report bugs and feature requests
- **GitHub Discussions**: Ask questions and provide feedback
- **Documentation**: Check the documentation first
- **Community**: Join the development community

### Reporting Issues

1. **Check Existing Issues**: Search for similar issues
2. **Provide Details**: Include error messages and logs
3. **Reproduction Steps**: Describe how to reproduce the issue
4. **Environment**: Include system and version information

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## ⚠️ Final Warning

**This is experimental software in active development. Use at your own risk and do not deploy in production until Phase 2 activation.**

---

**Remember**: This system is designed to make Bitcoin governance more transparent, accountable, and resistant to capture. But it's still in development. Stay informed, provide feedback, and wait for the official release.