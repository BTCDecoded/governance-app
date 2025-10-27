# API Reference

This document provides a comprehensive reference for the BTCDecoded governance application API.

## API Overview

The governance application provides a REST API for managing governance operations, economic nodes, and GitHub integration.

### Base URL

- **Development**: `http://localhost:3000`
- **Production**: `https://governance.btcdecoded.org`

### Authentication

All API endpoints require authentication via GitHub App tokens or maintainer signatures.

## Endpoints

### Health Check

#### GET /health

Check application health status.

**Response:**
```json
{
  "status": "healthy",
  "timestamp": "2025-01-01T00:00:00Z",
  "version": "0.1.0",
  "database": "connected",
  "github": "connected"
}
```

### Pull Request Management

#### GET /api/pull-requests

List all pull requests.

**Query Parameters:**
- `repo` (optional) - Filter by repository
- `status` (optional) - Filter by status
- `limit` (optional) - Limit results (default: 100)
- `offset` (optional) - Offset for pagination

**Response:**
```json
{
  "pull_requests": [
    {
      "id": 1,
      "repo_name": "BTCDecoded/orange-paper",
      "pr_number": 123,
      "head_sha": "abc123",
      "layer": 1,
      "tier": 2,
      "status": "pending",
      "created_at": "2025-01-01T00:00:00Z",
      "updated_at": "2025-01-01T00:00:00Z"
    }
  ],
  "total": 1,
  "limit": 100,
  "offset": 0
}
```

#### GET /api/pull-requests/{id}

Get specific pull request details.

**Response:**
```json
{
  "id": 1,
  "repo_name": "BTCDecoded/orange-paper",
  "pr_number": 123,
  "head_sha": "abc123",
  "layer": 1,
  "tier": 2,
  "status": "pending",
  "signatures": [
    {
      "maintainer": "alice",
      "signature": "signature_hash",
      "timestamp": "2025-01-01T00:00:00Z"
    }
  ],
  "veto_signals": [
    {
      "node_id": 1,
      "signal_type": "veto",
      "weight": 10.5,
      "timestamp": "2025-01-01T00:00:00Z"
    }
  ],
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z"
}
```

#### POST /api/pull-requests/{id}/sign

Add maintainer signature to pull request.

**Request Body:**
```json
{
  "maintainer": "alice",
  "signature": "signature_hash"
}
```

**Response:**
```json
{
  "status": "success",
  "message": "Signature added successfully"
}
```

### Economic Node Management

#### GET /api/economic-nodes

List all economic nodes.

**Query Parameters:**
- `status` (optional) - Filter by status
- `type` (optional) - Filter by node type
- `limit` (optional) - Limit results (default: 100)
- `offset` (optional) - Offset for pagination

**Response:**
```json
{
  "economic_nodes": [
    {
      "id": 1,
      "node_type": "mining_pool",
      "entity_name": "Example Pool",
      "public_key": "public_key_hash",
      "weight": 10.5,
      "status": "active",
      "registered_at": "2025-01-01T00:00:00Z"
    }
  ],
  "total": 1,
  "limit": 100,
  "offset": 0
}
```

#### POST /api/economic-nodes

Register new economic node.

**Request Body:**
```json
{
  "node_type": "mining_pool",
  "entity_name": "Example Pool",
  "public_key": "public_key_hash",
  "qualification_data": {
    "hash_power_percent": 5.0,
    "btc_holdings": 1000.0
  }
}
```

**Response:**
```json
{
  "id": 1,
  "status": "success",
  "message": "Economic node registered successfully"
}
```

#### POST /api/economic-nodes/{id}/veto

Submit veto signal for pull request.

**Request Body:**
```json
{
  "pr_id": 1,
  "signal_type": "veto",
  "signature": "signature_hash",
  "rationale": "Security concerns"
}
```

**Response:**
```json
{
  "status": "success",
  "message": "Veto signal submitted successfully"
}
```

### Governance Fork Management

#### GET /api/governance-fork/rulesets

List all governance rulesets.

**Response:**
```json
{
  "rulesets": [
    {
      "id": 1,
      "ruleset_id": "mainnet-v1.0.0",
      "version": "1.0.0",
      "config_hash": "hash123",
      "status": "active",
      "created_at": "2025-01-01T00:00:00Z"
    }
  ]
}
```

#### POST /api/governance-fork/export

Export governance configuration.

**Request Body:**
```json
{
  "ruleset_id": "mainnet-v1.0.0",
  "exported_by": "alice",
  "source_repo": "BTCDecoded/governance",
  "commit_hash": "abc123"
}
```

**Response:**
```json
{
  "export_id": "export_123",
  "ruleset_id": "mainnet-v1.0.0",
  "config_hash": "hash123",
  "exported_at": "2025-01-01T00:00:00Z",
  "download_url": "https://governance.btcdecoded.org/exports/export_123"
}
```

#### POST /api/governance-fork/decisions

Submit fork decision.

**Request Body:**
```json
{
  "node_id": 1,
  "ruleset_id": "mainnet-v1.0.0",
  "decision_type": "adopt",
  "signature": "signature_hash",
  "rationale": "Supports new governance rules"
}
```

**Response:**
```json
{
  "status": "success",
  "message": "Fork decision submitted successfully"
}
```

### Key Management

#### GET /api/keys

List all cryptographic keys.

**Query Parameters:**
- `type` (optional) - Filter by key type
- `status` (optional) - Filter by status
- `limit` (optional) - Limit results (default: 100)
- `offset` (optional) - Offset for pagination

**Response:**
```json
{
  "keys": [
    {
      "id": 1,
      "key_id": "maintainer-123",
      "key_type": "maintainer",
      "public_key": "public_key_hash",
      "status": "active",
      "created_at": "2025-01-01T00:00:00Z",
      "expires_at": "2026-01-01T00:00:00Z"
    }
  ],
  "total": 1,
  "limit": 100,
  "offset": 0
}
```

#### POST /api/keys

Generate new cryptographic key.

**Request Body:**
```json
{
  "key_type": "maintainer",
  "owner": "alice",
  "expiration_days": 365
}
```

**Response:**
```json
{
  "key_id": "maintainer-123",
  "public_key": "public_key_hash",
  "status": "success",
  "message": "Key generated successfully"
}
```

#### POST /api/keys/{id}/rotate

Rotate existing key.

**Request Body:**
```json
{
  "new_key_type": "maintainer",
  "new_key_owner": "alice",
  "expiration_days": 365
}
```

**Response:**
```json
{
  "new_key_id": "maintainer-124",
  "new_public_key": "new_public_key_hash",
  "status": "success",
  "message": "Key rotated successfully"
}
```

### Statistics and Monitoring

#### GET /api/statistics

Get system statistics.

**Response:**
```json
{
  "pull_requests": {
    "total": 100,
    "pending": 10,
    "approved": 80,
    "rejected": 10
  },
  "economic_nodes": {
    "total": 50,
    "active": 45,
    "inactive": 5
  },
  "keys": {
    "total": 200,
    "active": 180,
    "expired": 20
  },
  "governance_events": {
    "total": 1000,
    "last_24h": 50
  }
}
```

#### GET /api/statistics/veto

Get veto statistics for pull request.

**Query Parameters:**
- `pr_id` (required) - Pull request ID

**Response:**
```json
{
  "pr_id": 1,
  "veto_active": false,
  "mining_veto_percent": 15.0,
  "economic_veto_percent": 25.0,
  "total_signals": 10,
  "veto_count": 2,
  "support_count": 6,
  "abstain_count": 2
}
```

## Error Responses

All endpoints may return error responses in the following format:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request parameters",
    "details": {
      "field": "signature",
      "reason": "Invalid signature format"
    }
  }
}
```

### Error Codes

- `VALIDATION_ERROR` - Request validation failed
- `AUTHENTICATION_ERROR` - Authentication failed
- `AUTHORIZATION_ERROR` - Authorization failed
- `NOT_FOUND` - Resource not found
- `CONFLICT` - Resource conflict
- `INTERNAL_ERROR` - Internal server error
- `RATE_LIMITED` - Rate limit exceeded
- `SERVICE_UNAVAILABLE` - Service temporarily unavailable

## Rate Limiting

API endpoints are rate limited:

- **General endpoints**: 100 requests per minute
- **Authentication endpoints**: 10 requests per minute
- **Key management endpoints**: 5 requests per minute

Rate limit headers are included in responses:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1640995200
```

## Authentication

### GitHub App Authentication

For GitHub App operations, include the GitHub App token in the Authorization header:

```
Authorization: Bearer <github_app_token>
```

### Maintainer Authentication

For maintainer operations, include the maintainer signature in the request body or header:

```
X-Maintainer-Signature: <signature>
X-Maintainer-Public-Key: <public_key>
```

## Webhooks

### GitHub Webhooks

The application receives webhooks from GitHub for pull request and comment events.

**Webhook URL**: `POST /webhook/github`

**Headers:**
```
X-GitHub-Event: pull_request
X-Hub-Signature-256: sha256=<signature>
```

**Event Types:**
- `pull_request` - Pull request events
- `issue_comment` - Comment events

## SDK and Client Libraries

### Rust Client

```rust
use governance_app_client::GovernanceClient;

let client = GovernanceClient::new("http://localhost:3000");
let pr = client.get_pull_request(1).await?;
```

### JavaScript Client

```javascript
import { GovernanceClient } from '@btcdecoded/governance-client';

const client = new GovernanceClient('http://localhost:3000');
const pr = await client.getPullRequest(1);
```

## Related Documentation

- [Getting Started Guide](./GETTING_STARTED.md) - Quick setup
- [Configuration Reference](./CONFIGURATION.md) - Configuration options
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues
- [Development Guide](./DEVELOPMENT.md) - Development setup
- [Main Governance Documentation](../../governance/README.md) - System overview




