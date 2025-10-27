# Getting Started with Governance App

This guide will help you get started with the BTCDecoded governance application.

## Prerequisites

Before you begin, ensure you have:

- **Rust 1.70+** installed ([rustup.rs](https://rustup.rs/))
- **Git** installed
- **GitHub account** with appropriate permissions
- **SQLite 3** installed
- **Basic understanding** of Bitcoin governance concepts

## Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/BTCDecoded/governance-app.git
cd governance-app
```

### 2. Install Dependencies

```bash
cargo build
```

### 3. Set Up Configuration

```bash
cp config/app.toml.example config/app.toml
# Edit config/app.toml with your settings
```

### 4. Run Database Migrations

```bash
cargo run --bin governance-app -- migrate
```

### 5. Start the Application

```bash
cargo run --bin governance-app
```

## Configuration

### Basic Configuration

Edit `config/app.toml`:

```toml
[database]
url = "sqlite:governance.db"

[github]
app_id = "your_app_id"
private_key_path = "path/to/private_key.pem"
webhook_secret = "your_webhook_secret"

[server]
host = "0.0.0.0"
port = 3000
```

### Environment Variables

You can also use environment variables:

```bash
export DATABASE_URL="sqlite:governance.db"
export GITHUB_APP_ID="your_app_id"
export GITHUB_PRIVATE_KEY_PATH="path/to/private_key.pem"
export GITHUB_WEBHOOK_SECRET="your_webhook_secret"
export SERVER_HOST="0.0.0.0"
export SERVER_PORT="3000"
```

## GitHub App Setup

### 1. Create GitHub App

1. Go to GitHub Settings → Developer settings → GitHub Apps
2. Click "New GitHub App"
3. Fill in the required information
4. Note the App ID and generate a private key

### 2. Configure Webhook

1. Set webhook URL to `https://your-domain.com/webhook`
2. Select events: `pull_request`, `issue_comment`
3. Generate webhook secret

### 3. Install App

1. Install the app on your repositories
2. Grant necessary permissions
3. Configure access settings

## First Steps

### 1. Verify Installation

Check that the application is running:

```bash
curl http://localhost:3000/health
```

### 2. Test Webhook

Create a test pull request to verify webhook integration.

### 3. Check Logs

Monitor the application logs:

```bash
tail -f logs/governance-app.log
```

## Common Issues

### Database Connection

If you get database connection errors:

1. Check that SQLite is installed
2. Verify the database URL in configuration
3. Ensure the database file is writable

### GitHub API Errors

If you get GitHub API errors:

1. Verify your App ID and private key
2. Check that the app is installed on the repository
3. Ensure webhook secret matches

### Permission Errors

If you get permission errors:

1. Check file permissions on the database
2. Verify the application has write access
3. Check GitHub app permissions

## Next Steps

Once you have the basic setup working:

1. **Configure Governance Rules**: Set up your governance configuration
2. **Add Maintainers**: Configure maintainer keys and permissions
3. **Set Up Economic Nodes**: Configure economic node registry
4. **Test Workflows**: Test different governance scenarios
5. **Production Deployment**: Deploy to production environment

## Getting Help

If you encounter issues:

1. Check [Troubleshooting Guide](./TROUBLESHOOTING.md)
2. Review [Configuration Reference](./CONFIGURATION.md)
3. Check the [API Reference](./API_REFERENCE.md)
4. Review [Main Governance Documentation](../../governance/README.md)

## Related Documentation

- [Configuration Reference](./CONFIGURATION.md) - Detailed configuration options
- [API Reference](./API_REFERENCE.md) - Complete API documentation
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues and solutions
- [Development Guide](./DEVELOPMENT.md) - Development and contribution
- [Main Governance Documentation](../../governance/README.md) - System overview




