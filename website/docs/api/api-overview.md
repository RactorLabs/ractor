---
sidebar_position: 1
title: API Overview
---

# REST API Overview

Raworc provides a comprehensive REST API for Computer Use automation. The API enables programmatic control over Host sessions, dedicated computers, and enterprise operations.

## Base Information

- **Base URL**: `http://localhost:9000/api/v0`
- **Protocol**: HTTP/HTTPS
- **Format**: JSON
- **Authentication**: Bearer token (JWT)

## Authentication

All API endpoints (except `/version` and `/auth/login`) require authentication using a JWT bearer token.

## Sessions

Raworc uses sessions to provide Computer Use automation. Each session includes a dedicated computer with a Host for automating manual work. Sessions support:

- **Named Sessions**: Use names instead of UUIDs for easier identification
- **Session Publishing**: Share sessions publicly with configurable permissions
- **Auto-Timeouts**: Automatic resource management with idle-based timeouts
- **Auto-Restore**: Seamless session restoration when messaging closed sessions
- **Cross-User Access**: Admin privileges and published session access

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

### Host Sessions

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/sessions`](./rest-api-reference#get-sessions) | GET | List Host sessions |
| [`/sessions`](./rest-api-reference#post-sessions) | POST | Create new Host session |
| [`/sessions/{id}`](./rest-api-reference#get-sessionsid) | GET | Get specific session |
| [`/sessions/{id}`](./rest-api-reference#put-sessionsid) | PUT | Update session details |
| [`/sessions/{id}/state`](./rest-api-reference#put-sessionsidstate) | PUT | Update session state |
| [`/sessions/{id}/close`](./rest-api-reference#post-sessionsidclose) | POST | Close Host session |
| [`/sessions/{id}/restore`](./rest-api-reference#post-sessionsidrestore) | POST | Restore Host session |
| [`/sessions/{id}/remix`](./rest-api-reference#post-sessionsidremix) | POST | Fork Host session |
| [`/sessions/{id}/publish`](./rest-api-reference#post-sessionsidpublish) | POST | Publish session |
| [`/sessions/{id}/unpublish`](./rest-api-reference#post-sessionsidunpublish) | POST | Unpublish session |
| [`/sessions/{id}/busy`](./rest-api-reference#post-sessionsidbusy) | POST | Mark session busy |
| [`/sessions/{id}/idle`](./rest-api-reference#post-sessionsididle) | POST | Mark session idle |
| [`/sessions/{id}`](./rest-api-reference#delete-sessionsid) | DELETE | Delete session |

### Public Sessions

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/published/sessions`](./rest-api-reference#get-publishedsessions) | GET | List published sessions |
| [`/published/sessions/{id}`](./rest-api-reference#get-publishedsessionsid) | GET | Get published session |

### Host Communication

| Endpoint | Method | Description |
|----------|--------|--------------|
| [`/sessions/{id}/messages`](./rest-api-reference#get-sessionsidmessages) | GET | List Host messages |
| [`/sessions/{id}/messages`](./rest-api-reference#post-sessionsidmessages) | POST | Send message to Host |
| [`/sessions/{id}/messages/count`](./rest-api-reference#get-sessionsidmessagescount) | GET | Get message count |
| [`/sessions/{id}/messages`](./rest-api-reference#delete-sessionsidmessages) | DELETE | Clear all messages |

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
GET /sessions?state=idle
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
