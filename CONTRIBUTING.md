# Contributing to BTCDecoded Governance App

Thank you for your interest in contributing to the BTCDecoded Governance App!

## Development Setup

### Prerequisites

- Rust 1.70+
- PostgreSQL 13+
- Git

### Getting Started

1. Clone the repository:
   ```bash
   git clone https://github.com/BTCDecoded/governance-app.git
   cd governance-app
   ```

2. Set up environment:
   ```bash
   cp env.example .env
   # Edit .env with your configuration
   ```

3. Set up database:
   ```bash
   createdb governance
   ```

4. Run tests:
   ```bash
   cargo test
   ```

## Code Style

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Follow Rust naming conventions
- Document public APIs with `///` comments

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

```bash
cargo test --test integration_tests
```

### Coverage

```bash
cargo tarpaulin --out Html
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

### PR Requirements

- All tests must pass
- Code must be formatted with `cargo fmt`
- No clippy warnings
- Documentation updated for public APIs
- Security considerations addressed

## Governance

This project follows the BTCDecoded governance system:

- **Layer 3 (Implementation)**: 4-of-5 maintainers, 90 days
- Changes require maintainer signatures
- Review period must be met
- Cross-layer dependencies validated

## Security

- Never commit private keys or secrets
- Use environment variables for configuration
- Follow secure coding practices
- Report security issues to security@btcdecoded.org

## Questions?

- GitHub Issues for bug reports and feature requests
- GitHub Discussions for questions
- security@btcdecoded.org for security issues

## License

By contributing, you agree that your contributions will be licensed under the MIT License.




