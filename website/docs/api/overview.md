---
sidebar_position: 1
title: API Overview
---

# REST API Overview

Raworc provides a comprehensive REST API for accelerating agent development from prototype to production. The API enables programmatic control over containerized sessions, computer-use capabilities, multi-agent orchestration, and enterprise operations.

## Base Information

- **Base URL**: `http://your-server:9000/api/v0`
- **Protocol**: HTTP/HTTPS
- **Format**: JSON
- **Authentication**: Bearer token (JWT)

## Authentication

All API endpoints (except `/health`, `/version`, and `/auth/login`) require authentication using a JWT bearer token.

## Workspaces

Raworc uses spaces to organize resources and provide isolation. Sessions and secrets belong to spaces, while users and roles are global. Access is controlled through role bindings that specify which users have which roles in which spaces.

### Obtaining a Token

```bash
POST /api/v0/auth/login
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

### Using the Token

Include the token in the Authorization header:
```
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGc...
```

## API Endpoints

### Core Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/health`](./rest-api#get-health) | GET | Health check |
| [`/version`](./rest-api#get-version) | GET | API version info |
| [`/auth/login`](./rest-api#post-authlogin) | POST | Authenticate and get token |
| [`/auth/me`](./rest-api#get-authme) | GET | Get current user info |

### Service Accounts

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/service-accounts`](./rest-api#get-service-accounts) | GET | List all service accounts |
| [`/service-accounts`](./rest-api#post-service-accounts) | POST | Create new service account |
| [`/service-accounts/{id}`](./rest-api#get-service-accountsid) | GET | Get specific service account |
| [`/service-accounts/{id}`](./rest-api#put-service-accountsid) | PUT | Update service account |
| [`/service-accounts/{id}`](./rest-api#delete-service-accountsid) | DELETE | Delete service account |
| [`/service-accounts/{id}/password`](./rest-api#put-service-accountsidpassword) | PUT | Update service account password |

### Roles

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/roles`](./rest-api#get-roles) | GET | List all roles |
| [`/roles`](./rest-api#post-roles) | POST | Create new role |
| [`/roles/{id}`](./rest-api#get-rolesid) | GET | Get specific role |
| [`/roles/{id}`](./rest-api#delete-rolesid) | DELETE | Delete role |

### Role Bindings

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/role-bindings`](./rest-api#get-role-bindings) | GET | List all role bindings |
| [`/role-bindings`](./rest-api#post-role-bindings) | POST | Create new role binding |
| [`/role-bindings/{id}`](./rest-api#get-role-bindingsid) | GET | Get specific role binding |
| [`/role-bindings/{id}`](./rest-api#delete-role-bindingsid) | DELETE | Delete role binding |

### Secrets

| Endpoint | Method | Description |
|----------|--------|-------------|
| [`/secrets`](./rest-api#get-secrets) | GET | List secrets |
| [`/secrets`](./rest-api#post-secrets) | POST | Create new secret |
| [`/secrets/{key}`](./rest-api#get-secretskey) | GET | Get specific secret |
| [`/secrets/{key}`](./rest-api#put-secretskey) | PUT | Update secret |
| [`/secrets/{key}`](./rest-api#delete-secretskey) | DELETE | Delete secret |

### Computer Sessions

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/sessions`](./rest-api#get-sessions) | GET | List computer sessions |
| [`/sessions`](./rest-api#post-sessions) | POST | Create new computer session |
| [`/sessions/{id}`](./rest-api#get-sessionsid) | GET | Get specific session |
| [`/sessions/{id}`](./rest-api#put-sessionsid) | PUT | Update session details |
| [`/sessions/{id}/state`](./rest-api#put-sessionsidstate) | PUT | Update session state |
| [`/sessions/{id}/close`](./rest-api#post-sessionsidclose) | POST | Close computer session |
| [`/sessions/{id}/restore`](./rest-api#post-sessionsidrestore) | POST | Restore computer session |
| [`/sessions/{id}/remix`](./rest-api#post-sessionsidremix) | POST | Fork computer session |
| [`/sessions/{id}`](./rest-api#delete-sessionsid) | DELETE | Delete session |

### Task Communication

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/sessions/{id}/messages`](./rest-api#get-sessionsidmessages) | GET | List task messages |
| [`/sessions/{id}/messages`](./rest-api#post-sessionsidmessages) | POST | Send task to Computer Use agent |
| [`/sessions/{id}/messages/count`](./rest-api#get-sessionsidmessagescount) | GET | Get message count |
| [`/sessions/{id}/messages`](./rest-api#delete-sessionsidmessages) | DELETE | Clear all messages |


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
GET /sessions?limit=20&offset=0
GET /service-accounts?limit=50&offset=100
GET /spaces?limit=10&offset=0
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
GET /sessions?workspace_name=my-project  # Sessions for workspace
GET /sessions?state=IDLE
GET /spaces?active=true
GET /spaces/{name}/secrets?show_values=true
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

- [REST API Reference](rest-api.md) - HTTP REST API documentation (updated with correct base URLs)

## Next Steps

- Explore the [REST API Reference](rest-api.md) for detailed endpoint documentation  
- Review [RBAC Permissions](/docs/guides/security-rbac) for API access control
