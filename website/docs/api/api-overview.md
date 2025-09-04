---
sidebar_position: 1
title: API Overview
---

# REST API Overview

Raworc provides a comprehensive REST API for Computer Use automation. The API enables programmatic control over computer use agents, dedicated computers, and enterprise operations.

## Base Information

- **Base URL**: `http://localhost:9000/api/v0`
- **Protocol**: HTTP/HTTPS
- **Format**: JSON
- **Authentication**: Bearer token (JWT)

## Authentication

All API endpoints (except `/version` and `/auth/login`) require authentication using a JWT bearer token.

## Agents

Raworc uses agents to provide Computer Use automation. Each agent includes a dedicated computer with an AI assistant for automating manual work. Agents support:

- **Named Agents**: Use names instead of UUIDs for easier identification
- **Agent Publishing**: Share agents publicly with configurable permissions
- **Auto-Timeouts**: Automatic resource management with idle-based timeouts
- **Auto-Wake**: Seamless agent restoration when messaging sleeping agents
- **Cross-User Access**: Admin privileges and published agent access

### Obtaining a Token

**Operator Authentication (Primary Method):**

```bash
POST /api/v0/operators/{name}/login
Content-Type: application/json

{
  "user": "admin",
  "pass": "your-password"
}
```

Response:

```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "token_type": "Bearer",
  "expires_at": "2025-01-02T12:00:00Z"
}
```

**Token Creation (Admin Only):**

Admins can create tokens for any principal:

```bash
POST /api/v0/auth/token
Authorization: Bearer <admin-token>
Content-Type: application/json

{
  "principal": "user@example.com",
  "type": "User"
}
```

### Using the Token

Include the token in the Authorization header:

```
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGc...
```

## API Endpoints

### Core Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/version`](./rest-api-reference#get-version) | GET | API version and health info |
| [`/auth/login`](./rest-api-reference#post-authlogin) | POST | Authenticate and get token |
| [`/auth`](./rest-api-reference#get-auth) | GET | Get current user info |
| [`/auth/token`](./rest-api-reference#post-authtoken) | POST | Create token for any principal (admin only) |

### Operators

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/operators`](./rest-api-reference#get-operators) | GET | List all operators |
| [`/operators`](./rest-api-reference#post-operators) | POST | Create new operator |
| [`/operators/{name}`](./rest-api-reference#get-operatorsname) | GET | Get specific operator |
| [`/operators/{name}`](./rest-api-reference#put-operatorsname) | PUT | Update operator |
| [`/operators/{name}`](./rest-api-reference#delete-operatorsname) | DELETE | Delete operator |
| [`/operators/{name}/password`](./rest-api-reference#put-operatorsnamepassword) | PUT | Update operator password |

### Computer Use Agents

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/agents`](./rest-api-reference#get-agents) | GET | List computer use agents |
| [`/agents`](./rest-api-reference#post-agents) | POST | Create new agent |
| [`/agents/{id}`](./rest-api-reference#get-agentsid) | GET | Get specific agent |
| [`/agents/{id}`](./rest-api-reference#put-agentsid) | PUT | Update agent details |
| [`/agents/{id}/state`](./rest-api-reference#put-agentsidstate) | PUT | Update agent state |
| [`/agents/{id}/sleep`](./rest-api-reference#post-agentsidsleep) | POST | Sleep agent |
| [`/agents/{id}/wake`](./rest-api-reference#post-agentsidwake) | POST | Wake agent |
| [`/agents/{id}/remix`](./rest-api-reference#post-agentsidremix) | POST | Fork agent |
| [`/agents/{id}/publish`](./rest-api-reference#post-agentsidpublish) | POST | Publish agent |
| [`/agents/{id}/unpublish`](./rest-api-reference#post-agentsidunpublish) | POST | Unpublish agent |
| [`/agents/{id}/busy`](./rest-api-reference#post-agentsidbusy) | POST | Mark agent busy |
| [`/agents/{id}/idle`](./rest-api-reference#post-agentsididle) | POST | Mark agent idle |
| [`/agents/{id}`](./rest-api-reference#delete-agentsid) | DELETE | Delete agent |

### Public Agents

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/published/agents`](./rest-api-reference#get-publishedagents) | GET | List published agents |
| [`/published/agents/{id}`](./rest-api-reference#get-publishedagentsid) | GET | Get published agent |

### Agent Communication

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/agents/{id}/messages`](./rest-api-reference#get-agentsidmessages) | GET | List agent messages |
| [`/agents/{id}/messages`](./rest-api-reference#post-agentsidmessages) | POST | Send message to agent |
| [`/agents/{id}/messages/count`](./rest-api-reference#get-agentsidmessagescount) | GET | Get message count |
| [`/agents/{id}/messages`](./rest-api-reference#delete-agentsidmessages) | DELETE | Clear all messages |

## Request Format

### Headers

Required headers for authenticated requests:

```
Authorization: Bearer <token>
Content-Type: application/json
```

### Request Body

All POST and PUT requests accept JSON:

```json
{
  "field1": "value1",
  "field2": "value2"
}
```

## Response Format

### Success Response

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "example",
  "created_at": "2025-01-01T00:00:00Z"
}
```

### Error Response

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Resource not found"
  }
}
```

## Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 201 | Created |
| 204 | No Content (successful deletion) |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 409 | Conflict |
| 422 | Unprocessable Entity |
| 500 | Internal Server Error |

## Rate Limiting

Currently, Raworc does not enforce rate limiting, but this may change in future versions. Best practices:

- Cache responses when possible
- Use pagination for list operations
- Implement exponential backoff for retries

## Pagination

List endpoints support pagination:

```
GET /agents?limit=20&offset=0
GET /operators?limit=50&offset=100
```

**Parameters**:

- `limit` - Maximum number of items to return (default: 100, max: 1000)
- `offset` - Number of items to skip (default: 0)

**Response Headers**:

```
X-Total-Count: 250
X-Page-Offset: 100
X-Page-Limit: 50
```

## Filtering

Some endpoints support filtering:

```
GET /agents?state=idle
```

## SDK Support

Official SDKs are planned for:

- Python
- JavaScript/TypeScript
- Go
- Rust

## Webhooks

Webhook support is planned for future releases to enable:

- Real-time notifications
- Event-driven workflows
- Third-party integrations

## API Versioning

The API uses URL versioning:

- Current version: `v0`
- Format: `/api/v{version}/endpoint`

Breaking changes will result in a new API version.

## Best Practices

1. **Use Specific Fields**: Only request/send needed fields
2. **Handle Errors**: Implement proper error handling
3. **Validate Input**: Validate data before sending
4. **Use HTTPS**: Always use HTTPS in production
5. **Token Management**: Refresh tokens before expiry
6. **Idempotency**: Make requests idempotent where possible

## Available API Documentation

- [REST API Reference](rest-api-reference.md) - HTTP REST API documentation

## Next Steps

- Explore the [REST API Reference](rest-api-reference.md) for detailed endpoint documentation  
- Review [RBAC System](/docs/concepts/authentication-users) for API access control
