---
sidebar_position: 2
title: REST API Reference
---

# REST API Reference

Complete reference for all Raworc REST API endpoints.

## Overview

- **Base URL**: `http://localhost:9000/api/v0`
- **Authentication**: Bearer token (JWT) required for most endpoints
- **Content-Type**: application/json

## Version & Health

### GET /version

Get API version and health information.

**Authentication**: Not required

**Response**: `200 OK`
```json
{
  "version": "0.3.0",
  "api": "v0"
}
```


## Authentication

### POST /operators/{name}/login

Authenticate with operator credentials and receive a JWT token.

**Authentication**: Not required

**Parameters**:
- `name` (path) - Operator name

**Request Body**:
```json
{
  "pass": "admin"
}
```

**Response**: `200 OK`
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "token_type": "Bearer",
  "expires_at": "2024-01-15T11:30:00Z"
}
```

**Errors**:
- `400 Bad Request` - Missing required fields
- `401 Unauthorized` - Invalid credentials

### GET /auth

Get information about the authenticated user and token status.

**Authentication**: Required

**Response**: `200 OK`
```json
{
  "user": "admin",
  "type": "Operator"
}
```

### POST /auth/token

Create a JWT token for any principal. Admin-only endpoint for creating tokens for users or operators.

**Authentication**: Required (Admin only)

**Request Body**:

Create token for a User:
```json
{
  "principal": "user@example.com",
  "principal_type": "User"
}
```

Create token for a Operator:
```json
{
  "principal": "api-service",
  "principal_type": "Operator"
}
```

**Fields**:
- `principal` - The user identifier for the principal
- `principal_type` - Must be "User" or "Operator"

**Response**: `200 OK`
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "token_type": "Bearer",
  "expires_at": "2025-01-15T11:30:00Z"
}
```

**Errors**:
- `400 Bad Request` - Invalid principal_type
- `403 Forbidden` - Not admin

## Operators

### GET /operators

List all operators.

**Authentication**: Required

**Response**: `200 OK`
```json
[
  {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "user": "api-user",
    "scope": null,
    "description": "API access user",
    "active": true,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z",
    "last_login_at": "2025-01-01T12:00:00Z"
  }
]
```

### POST /operators

Create a new operator.

**Authentication**: Required

**Request Body**:
```json
{
  "user": "api-user",
  "pass": "secure-password",
  "scope": null,
  "description": "API access user"
}
```

**Response**: `200 OK`
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "user": "api-user",
  "scope": null,
  "description": "API access user",
  "active": true,
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z",
  "last_login_at": null
}
```

### GET /operators/\{name\}

Get a specific operator by name.

**Authentication**: Required

**Parameters**:
- `name` (path) - Operator name

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Operator not found

### PUT /operators/\{name\}

Update a operator.

**Authentication**: Required

**Parameters**:
- `name` (path) - Operator name

**Request Body**:
```json
{
  "description": "Updated description",
  "active": true
}
```

**Response**: `200 OK`
(Returns updated operator)

### DELETE /operators/\{name\}

Delete a operator.

**Authentication**: Required

**Parameters**:
- `name` (path) - Operator name

**Response**: `200 OK`

### PUT /operators/\{name\}/password

Update operator password.

**Authentication**: Required

**Parameters**:
- `name` (path) - Operator name

**Request Body**:
```json
{
  "current_password": "old-password",
  "new_password": "new-secure-password"
}
```

**Response**: `200 OK`

## Sessions

### GET /sessions

List Host sessions.

**Authentication**: Required

**Response**: `200 OK`
```json
[
  {
    "id": "61549530-3095-4cbf-b379-cd32416f626d",
    "state": "IDLE",
    "created_at": "2025-01-20T10:00:00Z",
    "started_at": "2025-01-20T10:01:00Z",
    "last_activity_at": "2025-01-20T10:05:00Z",
    "created_by": "admin"
  }
]
```

### POST /sessions

Create a new Host session.

**Authentication**: Required

**Request Body**:
```json
{
  "metadata": {},
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Host specialized in data analysis.",
  "setup": "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy"
}
```

**Fields**:
- `metadata` (optional) - Additional metadata object (defaults to {})
- `secrets` (optional) - Environment variables/secrets for the session
- `instructions` (optional) - Instructions for the Host
- `setup` (optional) - Setup script to run in the container

**Response**: `200 OK`
```json
{
  "id": "61549530-3095-4cbf-b379-cd32416f626d",
  "created_by": "admin",
  "state": "INIT",
  "container_id": null,
  "persistent_volume_id": null,
  "parent_session_id": null,
  "created_at": "2025-01-20T10:00:00Z",
  "started_at": null,
  "last_activity_at": null,
  "terminated_at": null,
  "termination_reason": null,
  "metadata": {}
}
```

### GET /sessions/\{name\}

Get a specific session by ID.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`
(Same format as POST response)

### PUT /sessions/\{name\}

Update session details.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Request Body**:
```json
{
  "metadata": {
    "updated_by": "admin",
    "purpose": "production deployment"
  }
}
```

**Response**: `200 OK`
(Returns updated session)

### PUT /sessions/\{name\}/state

Update session state.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Request Body**:
```json
{
  "state": "closed"
}
```

**Response**: `200 OK`

### POST /sessions/\{name\}/close

Close a Host session (saves resources by stopping the container while preserving state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{name\}/restore

Restore a closed Host session (restarts the container with preserved state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{name\}/remix

Create a new Host session based on an existing session with selective content copying.

**Authentication**: Required

**Parameters**:
- `id` (path) - Parent session ID

**Request Body**:
```json
{
  "metadata": {
    "remixed_from": "61549530-3095-4cbf-b379-cd32416f626d",
    "remix_timestamp": "2025-01-20T10:00:00Z"
  },
  "data": true,
  "code": false
}
```

**Fields**:
- `metadata` (optional) - Additional metadata for the new session
- `data` (optional) - Copy data files from parent session (default: true)
- `code` (optional) - Copy code files from parent session (default: true)

**Response**: `200 OK`
```json
{
  "id": "new-session-id",
  "state": "INIT",
  "parent_session_id": "61549530-3095-4cbf-b379-cd32416f626d",
  "created_at": "2025-01-20T10:00:00Z",
  "created_by": "admin",
  "metadata": {
    "remixed_from": "61549530-3095-4cbf-b379-cd32416f626d",
    "remix_timestamp": "2025-01-20T10:00:00Z"
  }
}
```

### DELETE /sessions/\{name\}

Terminate a Host session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

## Session Messages

### GET /sessions/\{name\}/messages

List messages in a Host session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Query Parameters**:
- `limit` (optional) - Maximum number of messages to return

**Response**: `200 OK`
```json
[
  {
    "id": "msg-uuid",
    "session_id": "session-uuid",
    "role": "user",
    "content": "Generate a Python script to calculate fibonacci numbers",
    "created_at": "2025-01-20T10:00:00Z"
  },
  {
    "id": "msg-uuid-2", 
    "session_id": "session-uuid",
    "role": "host",
    "content": "I'll create a Python script for calculating fibonacci numbers...",
    "created_at": "2025-01-20T10:01:00Z"
  }
]
```

### POST /sessions/\{name\}/messages

Send a message to a Host session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Request Body**:
```json
{
  "content": "Generate a Python script to calculate fibonacci numbers"
}
```

**Response**: `200 OK`
```json
{
  "id": "msg-uuid",
  "session_id": "session-uuid", 
  "role": "user",
  "content": "Generate a Python script to calculate fibonacci numbers",
  "created_at": "2025-01-20T10:00:00Z"
}
```

### GET /sessions/\{name\}/messages/count

Get message count for Host session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`
```json
{
  "count": 25
}
```

### DELETE /sessions/\{name\}/messages

Clear all Host session messages.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

## Error Responses

All errors follow a consistent format:

```json
{
  "error": {
    "message": "Resource not found"
  }
}
```

### Common HTTP Status Codes

| Code | Description |
|------|-------------|
| `400` | Bad Request - Invalid request data |
| `401` | Unauthorized - Missing or invalid token |
| `403` | Forbidden - Insufficient permissions |
| `404` | Not Found - Resource not found |
| `409` | Conflict - Resource already exists |
| `500` | Internal Server Error |

