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

## Health & Status

### GET /health

Comprehensive health check including database connectivity, Docker daemon status, and system resource monitoring.

**Authentication**: Not required

**Response**: `200 OK`
```text
OK
```

### GET /version

Get API version and build information.

**Authentication**: Not required

**Response**: `200 OK`
```json
{
  "version": "0.3.0",
  "api": "v0"
}
```


## Authentication

### POST /auth/login

Authenticate with service account credentials and receive a JWT token.

**Authentication**: Not required

**Request Body**:
```json
{
  "user": "admin", 
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
  "type": "ServiceAccount"
}
```

### POST /auth/token

Create a JWT token for any principal. Admin-only endpoint for creating tokens for users or service accounts.

**Authentication**: Required (Admin only)

**Request Body**:

Create token for a User:
```json
{
  "principal": "user@example.com",
  "principal_type": "User"
}
```

Create token for a ServiceAccount:
```json
{
  "principal": "api-service",
  "principal_type": "ServiceAccount"
}
```

**Fields**:
- `principal` - The username or identifier for the principal
- `principal_type` - Must be "User" or "ServiceAccount"

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

## Service Accounts

### GET /service-accounts

List all service accounts.

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

### POST /service-accounts

Create a new service account.

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

### GET /service-accounts/\{id\}

Get a specific service account by ID.

**Authentication**: Required

**Parameters**:
- `id` (path) - Service account ID

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Service account not found

### PUT /service-accounts/\{id\}

Update a service account.

**Authentication**: Required

**Parameters**:
- `id` (path) - Service account ID

**Request Body**:
```json
{
  "description": "Updated description",
  "active": true
}
```

**Response**: `200 OK`
(Returns updated service account)

### DELETE /service-accounts/\{id\}

Delete a service account.

**Authentication**: Required

**Parameters**:
- `id` (path) - Service account ID

**Response**: `200 OK`

### PUT /service-accounts/\{id\}/password

Update service account password.

**Authentication**: Required

**Parameters**:
- `id` (path) - Service account ID

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

### GET /sessions/\{id\}

Get a specific session by ID.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`
(Same format as POST response)

### PUT /sessions/\{id\}

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

### PUT /sessions/\{id\}/state

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

### POST /sessions/\{id\}/close

Close a Host session (saves resources by stopping the container while preserving state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{id\}/restore

Restore a closed Host session (restarts the container with preserved state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{id\}/remix

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

### DELETE /sessions/\{id\}

Terminate a Host session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

## Session Messages

### GET /sessions/\{id\}/messages

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

### POST /sessions/\{id\}/messages

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

### GET /sessions/\{id\}/messages/count

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

### DELETE /sessions/\{id\}/messages

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

