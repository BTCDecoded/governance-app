# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in the BTCDecoded Governance App, please report it to security@btcdecoded.org.

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Timeline

- **Initial Response**: Within 24 hours
- **Status Update**: Within 72 hours
- **Resolution**: Within 7 days (for critical issues)

## Security Considerations

### Cryptographic Security

- All signature verification uses secp256k1 (Bitcoin-compatible)
- Public keys are validated before use
- Signature formats are strictly enforced

### Database Security

- All database queries use parameterized statements
- No direct string concatenation in SQL
- Audit logs are immutable

### Network Security

- Webhook signatures are verified
- HTTPS required for all external communication
- Rate limiting on webhook endpoints

### Access Control

- Maintainer keys are stored securely
- Emergency keyholders have separate access controls
- All governance actions are logged

## Security Boundaries

The governance app operates at the **application layer** and does not:

- Modify consensus rules
- Access private keys (only verifies signatures)
- Store sensitive data beyond public keys
- Make consensus decisions (only enforces existing rules)

## Dependencies

All consensus-critical dependencies are pinned to exact versions:

```toml
secp256k1 = "=0.28.2"
bitcoin = "=0.31.0"
sha2 = "=0.10.9"
```

## Contact

For security-related questions, contact security@btcdecoded.org.




