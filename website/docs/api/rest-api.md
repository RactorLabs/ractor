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
  "version": "0.1.1",
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

### GET /auth/me

Get information about the authenticated user.

**Authentication**: Required

**Response**: `200 OK`
```json
{
  "user": "admin",
  "namespace": null,
  "type": "ServiceAccount"
}
```

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
    "space": null,
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
  "space": null,
  "description": "API access user"
}
```

**Response**: `200 OK`
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "user": "api-user",
  "space": null,
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
  "space": "production",
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

## Roles

### GET /roles

List all RBAC roles with their permission rules.

**Authentication**: Required (admin permissions)

**Response**: `200 OK`
```json
[
  {
    "name": "admin",
    "description": "Full administrative access to all resources",
    "rules": [
      {
        "resources": ["*"],
        "verbs": ["*"],
        "scope": "global"
      }
    ],
    "created_at": "2024-01-01T00:00:00Z"
  },
  {
    "name": "operator", 
    "description": "Session and secret management permissions",
    "rules": [
      {
        "resources": ["sessions", "agents", "secrets"],
        "verbs": ["create", "read", "update", "delete", "list"],
        "scope": "space"
      },
      {
        "resources": ["spaces"],
        "verbs": ["read", "list"], 
        "scope": "global"
      }
    ],
    "created_at": "2024-01-01T00:00:00Z"
  },
  {
    "name": "developer",
    "description": "Development access without secret modification",
    "rules": [
      {
        "resources": ["sessions", "agents"],
        "verbs": ["create", "read", "update", "list"],
        "scope": "space"
      },
      {
        "resources": ["secrets"],
        "verbs": ["read", "list"],
        "scope": "space"
      }
    ],
    "created_at": "2024-01-01T00:00:00Z"
  }
]
```

### POST /roles

Create a new role.

**Authentication**: Required

**Request Body**:
```json
{
  "id": "developer",
  "description": "Developer role",
  "rules": [
    {
      "apiGroups": [""],
      "resources": ["sessions", "messages"],
      "verbs": ["get", "list", "create"]
    }
  ]
}
```

**Response**: `200 OK`
(Returns created role)

### GET /roles/\{id\}

Get a specific role by ID.

**Authentication**: Required

**Parameters**:
- `id` (path) - Role ID

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Role not found

### DELETE /roles/\{id\}

Delete a role.

**Authentication**: Required

**Parameters**:
- `id` (path) - Role ID

**Response**: `200 OK`

## Role Bindings

### GET /role-bindings

List all role bindings.

**Authentication**: Required

**Response**: `200 OK`
```json
[
  {
    "id": "admin-binding",
    "subject": "admin",
    "role_ref": "admin",
    "space": null,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

### POST /role-bindings

Create a new role binding.

**Authentication**: Required

**Request Body**:
```json
{
  "subject": "api-user",
  "role_ref": "developer",
  "space": "staging"
}
```

**Response**: `200 OK`
(Returns created role binding)

### GET /role-bindings/\{id\}

Get a specific role binding by ID.

**Authentication**: Required

**Parameters**:
- `id` (path) - Role binding ID

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Role binding not found

### DELETE /role-bindings/\{id\}

Delete a role binding.

**Authentication**: Required

**Parameters**:
- `id` (path) - Role binding ID

**Response**: `200 OK`

## Spaces

### GET /spaces

List all spaces.

**Authentication**: Required

**Response**: `200 OK`
```json
[
  {
    "name": "default",
    "description": "Default space",
    "settings": {},
    "active": true,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

### POST /spaces

Create a new space (admin only).

**Authentication**: Required

**Request Body**:
```json
{
  "name": "staging",
  "description": "Staging environment",
  "settings": {
    "environment": "staging"
  }
}
```

**Response**: `200 OK`
```json
{
  "name": "staging",
  "description": "Staging environment", 
  "settings": {
    "environment": "staging"
  },
  "active": true,
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z"
}
```

### GET /spaces/\{name\}

Get a specific space by name.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Space not found

### PUT /spaces/\{name\}

Update a space (admin only).

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Request Body**:
```json
{
  "description": "Updated staging space"
}
```

**Response**: `200 OK`
(Returns updated space)

### DELETE /spaces/\{name\}

Delete a space (admin only, cannot delete 'default').

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`

## Space Secrets

### GET /spaces/\{name\}/secrets

List secrets in a space (metadata only).

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Query Parameters**:
- `show_values` (optional) - Include secret values (requires read-values permission)

**Response**: `200 OK`
```json
[
  {
    "key_name": "ANTHROPIC_API_KEY",
    "description": "Claude API key",
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

### POST /spaces/\{name\}/secrets

Create a secret in a space.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Request Body**:
```json
{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-your-actual-key",
  "description": "Claude API key"
}
```

**Fields**:
- `key_name` (required) - Secret key identifier
- `value` (required) - Secret value
- `description` (optional) - Human-readable description

**Response**: `200 OK`
```json
{
  "space": "default",
  "key_name": "ANTHROPIC_API_KEY",
  "description": "Claude API key",
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z",
  "created_by": "admin"
}
```

### GET /spaces/\{name\}/secrets/\{key\}

Get a specific secret.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `key` (path) - Secret key name

**Query Parameters**:
- `show_values` (optional) - Include secret value

**Response**: `200 OK`
```json
{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-your-actual-key",
  "description": "Claude API key",
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z"
}
```

### PUT /spaces/\{name\}/secrets/\{key\}

Update a secret value or description.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `key` (path) - Secret key name

**Request Body**:
```json
{
  "value": "new-secret-value",
  "description": "Updated description"
}
```

**Response**: `200 OK`
(Returns updated secret)

### DELETE /spaces/\{name\}/secrets/\{key\}

Delete a secret.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `key` (path) - Secret key name

**Response**: `200 OK`

## Space Agents

### GET /spaces/\{name\}/agents

List agents in a space.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`
```json
[
  {
    "name": "raworc-agent-python-demo",
    "description": "Python demo agent",
    "purpose": "Python agent that speaks English",
    "source_repo": "Raworc/raworc-agent-python-demo",
    "source_branch": "main",
    "status": "active"
  }
]
```

### POST /spaces/\{name\}/agents

Create an agent (triggers automatic space rebuild).

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Request Body**:
```json
{
  "name": "data-analyzer",
  "description": "Data analysis specialist",
  "purpose": "analyze data, create visualizations, statistical analysis", 
  "source_repo": "Raworc/raworc-agent-python-demo",
  "source_branch": "main"
}
```

**Response**: `200 OK`
(Returns created agent)

### GET /spaces/\{name\}/agents/\{agent_name\}

Get a specific agent.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Response**: `200 OK`
(Same format as POST response)

**Errors**:
- `404 Not Found` - Agent not found

### PUT /spaces/\{name\}/agents/\{agent_name\}

Update an agent.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Request Body**:
```json
{
  "description": "Updated data analysis specialist",
  "purpose": "enhanced data analysis and visualization"
}
```

**Response**: `200 OK`
(Returns updated agent)

### DELETE /spaces/\{name\}/agents/\{agent_name\}

Delete an agent.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Response**: `200 OK`

### PATCH /spaces/\{name\}/agents/\{agent_name\}/status

Update agent status.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Request Body**:
```json
{
  "status": "inactive"
}
```

**Response**: `200 OK`

### POST /spaces/\{name\}/agents/\{agent_name\}/deploy

Deploy an agent.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Response**: `200 OK`

### POST /spaces/\{name\}/agents/\{agent_name\}/stop

Stop an agent.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Response**: `200 OK`

### GET /spaces/\{name\}/agents/running

List running agents in a space.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`
```json
[
  {
    "name": "raworc-agent-python-demo",
    "status": "running",
    "started_at": "2025-01-01T12:00:00Z"
  }
]
```

### GET /spaces/\{name\}/agents/\{agent_name\}/logs

Get agent logs.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `agent_name` (path) - Agent name

**Query Parameters**:
- `limit` (optional) - Maximum number of log lines to return
- `follow` (optional) - Follow log output

**Response**: `200 OK`
```json
{
  "logs": [
    "2025-01-01T12:00:00Z [INFO] Agent started",
    "2025-01-01T12:01:00Z [INFO] Processing request"
  ]
}
```

## Space Builds

### POST /spaces/\{name\}/build

Manually trigger space build after adding agents.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`

### GET /spaces/\{name\}/build/latest

Check space build status.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name

**Response**: `200 OK`
```json
{
  "status": "completed",
  "started_at": "2025-01-01T00:00:00Z",
  "completed_at": "2025-01-01T00:05:00Z"
}
```

### GET /spaces/\{name\}/build/\{build_id\}

Get specific build status.

**Authentication**: Required

**Parameters**:
- `name` (path) - Space name
- `build_id` (path) - Build ID

**Response**: `200 OK`
```json
{
  "id": "build-550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "started_at": "2025-01-01T00:00:00Z",
  "completed_at": "2025-01-01T00:05:00Z",
  "logs": "Build completed successfully"
}
```

**Errors**:
- `404 Not Found` - Build not found

## Sessions

### GET /sessions

List sessions.

**Authentication**: Required

**Response**: `200 OK`
```json
[
  {
    "id": "61549530-3095-4cbf-b379-cd32416f626d",
    "space": "default",
    "state": "IDLE",
    "created_at": "2025-01-20T10:00:00Z",
    "started_at": "2025-01-20T10:01:00Z",
    "last_activity_at": "2025-01-20T10:05:00Z"
  }
]
```

### POST /sessions

Create a new session.

**Authentication**: Required

**Request Body**:
```json
{
  "space": "default",
  "metadata": {}
}
```

**Fields**:
- `space` (optional) - Space name (defaults to "default")
- `metadata` (optional) - Additional metadata object (defaults to {})

**Response**: `200 OK`
```json
{
  "id": "61549530-3095-4cbf-b379-cd32416f626d",
  "space": "default",
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
  "space": "production"
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
  "state": "PAUSED"
}
```

**Response**: `200 OK`

### POST /sessions/\{id\}/pause

Pause a session (saves resources).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{id\}/suspend

Suspend a session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{id\}/resume

Resume a paused session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

### POST /sessions/\{id\}/remix

Fork session (create child session).

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Request Body**:
```json
{
  "space": "development"
}
```

**Response**: `200 OK`
```json
{
  "id": "new-session-id",
  "space": "development",
  "state": "INIT",
  "parent_session": "61549530-3095-4cbf-b379-cd32416f626d",
  "created_at": "2025-01-20T10:00:00Z"
}
```

### DELETE /sessions/\{id\}

Terminate a session.

**Authentication**: Required

**Parameters**:
- `id` (path) - Session ID

**Response**: `200 OK`

## Session Messages

### GET /sessions/\{id\}/messages

List messages in a session.

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
    "role": "assistant",
    "content": "I'll create a Python script for calculating fibonacci numbers...",
    "created_at": "2025-01-20T10:01:00Z"
  }
]
```

### POST /sessions/\{id\}/messages

Send a message to a session.

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

Get message count for session.

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

Clear all session messages.

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

