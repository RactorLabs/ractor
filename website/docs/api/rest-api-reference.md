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

## CLI API Access

The Raworc CLI provides convenient access to all API endpoints:

```bash
# Direct API calls
raworc api <endpoint>

# Examples
raworc api version                   # GET /version
raworc api agents                    # GET /agents
raworc api agents -m POST            # POST /agents
raworc api agents/abc123            # GET /agents/abc123
raworc api agents/abc123 -m DELETE  # DELETE /agents/abc123

# With request body
raworc api agents -m POST -b '{"instructions":"Hello"}'

# With query parameters  
raworc api "agents/abc123/messages?limit=10"

# Response formatting
raworc api agents --pretty           # Pretty print JSON
raworc api agents --headers          # Show response headers
raworc api agents --status           # Show HTTP status
```

**CLI Options:**
- `-m, --method <method>` - HTTP method (GET, POST, PUT, DELETE, PATCH)
- `-b, --body <body>` - JSON request body
- `-H, --headers` - Show response headers
- `-p, --pretty` - Pretty print JSON (default: true)
- `-s, --status` - Show HTTP status code

**Authentication:**
The CLI automatically includes your stored authentication token. Authenticate first:
```bash
raworc login -u admin -p admin    # Generate token
raworc auth -t <jwt-token>        # Authenticate CLI
```

## Version & Health

### GET /version

Get API version and health information.

**Authentication**: Not required

**Response**: `200 OK`
```json
{
  "version": "0.3.7",
  "api": "v0"
}
```


## Authentication

### POST /operators/`{name}`/login

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
  "type": "User"
}
```

Create token for a Operator:
```json
{
  "principal": "api-service",
  "type": "Operator"
}
```

**Fields**:
- `principal` - The user identifier for the principal
- `type` - Must be "User" or "Operator"

**Response**: `200 OK`
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "token_type": "Bearer",
  "expires_at": "2025-01-15T11:30:00Z"
}
```

**Errors**:
- `400 Bad Request` - Invalid type
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

### PUT /operators/`{name}`/password

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

## Agents

### GET /agents

List computer use agents. Regular users see only their own agents, admin users see all agents.

**Authentication**: Required

**Query Parameters**:
- `state` (optional) - Filter agents by state (init, idle, busy, slept, deleted)

**Response**: `200 OK`
```json
[
  {
    "id": "61549530-3095-4cbf-b379-cd32416f626d",
    "created_by": "admin",
    "name": "my-agent",
    "state": "idle",
    "container_id": "container-id",
    "persistent_volume_id": "volume-id",
    "parent_agent_name": null,
    "created_at": "2025-01-20T10:00:00Z",
    "last_activity_at": "2025-01-20T10:05:00Z",
    "metadata": {},
    "is_published": false,
    "published_at": null,
    "published_by": null,
    "publish_permissions": {},
    "timeout_seconds": 300,
    "auto_sleep_at": "2025-01-20T10:10:00Z"
  }
]
```

### POST /agents

Create a new computer use agent.

**Authentication**: Required

**Request Body**:
```json
{
  "name": "my-agent",
  "metadata": {},
  "secrets": {
    "DATABASE_URL": "mysql://user:pass@host/db",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful AI agent specialized in data analysis.",
  "setup": "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy",
  "timeout_seconds": 300
}
```

**Fields**:
- `name` (optional) - Unique name for the agent
- `metadata` (optional) - Additional metadata object (defaults to {})
- `secrets` (optional) - Environment variables/secrets for the agent
- `instructions` (optional) - Instructions for the AI agent
- `setup` (optional) - Setup script to run in the container
- `timeout_seconds` (optional) - Agent timeout in seconds (default: 300)

**Response**: `200 OK`
```json
{
  "id": "61549530-3095-4cbf-b379-cd32416f626d",
  "created_by": "admin",
  "name": "my-agent",
  "state": "init",
  "container_id": null,
  "persistent_volume_id": null,
  "parent_agent_name": null,
  "created_at": "2025-01-20T10:00:00Z",
  "started_at": null,
  "last_activity_at": null,
  "terminated_at": null,
  "termination_reason": null,
  "metadata": {},
  "is_published": false,
  "published_at": null,
  "published_by": null,
  "publish_permissions": {},
  "timeout_seconds": 300,
  "auto_sleep_at": null
}
```

### GET /agents/\{name\}

Get a specific agent by ID or name. Users can access their own agents and published agents. Admin users can access any agent.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`
(Same format as POST response)

### PUT /agents/\{name\}

Update agent details.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID

**Request Body**:
```json
{
  "name": "my-updated-agent",
  "metadata": {
    "updated_by": "admin",
    "purpose": "production deployment"
  },
  "timeout_seconds": 600
}
```

**Fields**:
- `name` (optional) - Update agent name (alphanumeric and hyphens only)
- `metadata` (optional) - Update agent metadata object
- `timeout_seconds` (optional) - Update agent timeout in seconds

**Response**: `200 OK`
(Returns updated agent)

### PUT /agents/`{name}`/state

Update agent state.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID

**Request Body**:
```json
{
  "state": "sleeping"
}
```

**Response**: `200 OK`

### POST /agents/`{name}`/sleep

Sleep an agent (saves resources by stopping the container while preserving state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID

**Response**: `200 OK`

### POST /agents/`{name}`/wake

Wake a sleeping agent (restarts the container with preserved state).

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Request Body** (optional):
```json
{
  "prompt": "Continue working on the analysis from where we left off"
}
```

**Fields**:
- `prompt` (optional) - Message to send after waking agent

**Response**: `200 OK`

### POST /agents/`{name}`/publish

Publish an agent for public access with configurable remix permissions.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Request Body**:
```json
{
  "data": true,
  "code": true,
  "secrets": false
}
```

**Fields**:
- `data` (optional) - Allow data files to be remixed (default: true)
- `code` (optional) - Allow code files to be remixed (default: true) 
- `secrets` (optional) - Allow secrets to be remixed (default: false)

**Response**: `200 OK`
```json
{
  "success": true,
  "message": "Agent published successfully"
}
```

### POST /agents/`{name}`/unpublish

Remove agent from public access.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`
```json
{
  "success": true,
  "message": "Agent unpublished successfully"
}
```

### POST /agents/`{name}`/busy

Mark agent as busy (prevents timeout).

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`

### POST /agents/`{name}`/idle

Mark agent as idle (enables timeout counting).

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`

### POST /agents/`{name}`/remix

Create a new agent based on an existing agent with selective content copying.

**Authentication**: Required

**Parameters**:
- `id` (path) - Parent agent ID or name

**Request Body**:
```json
{
  "name": "my-new-agent",
  "metadata": {
    "remixed_from": "61549530-3095-4cbf-b379-cd32416f626d",
    "remix_timestamp": "2025-01-20T10:00:00Z"
  },
  "data": true,
  "code": false,
  "secrets": true
}
```

**Fields**:
- `name` (optional) - Name for the new agent
- `metadata` (optional) - Additional metadata for the new agent
- `data` (optional) - Copy data files from parent agent (default: true)
- `code` (optional) - Copy code files from parent agent (default: true)
- `secrets` (optional) - Copy secrets from parent agent (default: true)

**Response**: `200 OK`
```json
{
  "id": "new-agent-name",
  "name": "my-new-agent",
  "state": "init",
  "parent_agent_name": "original-agent",
  "created_at": "2025-01-20T10:00:00Z",
  "created_by": "admin",
  "metadata": {
    "remixed_from": "61549530-3095-4cbf-b379-cd32416f626d",
    "remix_timestamp": "2025-01-20T10:00:00Z"
  }
}
```

### DELETE /agents/\{name\}

Terminate an agent.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`

## Public Agents

### GET /published/agents

List all published agents available for public access.

**Authentication**: Not required

**Response**: `200 OK`
```json
[
  {
    "id": "61549530-3095-4cbf-b379-cd32416f626d",
    "created_by": "admin",
    "name": "public-data-analysis",
    "state": "idle",
    "created_at": "2025-01-20T10:00:00Z",
    "published_at": "2025-01-20T10:30:00Z",
    "published_by": "admin",
    "publish_permissions": {
      "data": true,
      "code": true,
      "secrets": false
    },
    "metadata": {}
  }
]
```

### GET /published/agents/\{name\}

Get a specific published agent by ID or name.

**Authentication**: Not required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`
(Same format as published agents list)

## Agent Messages

### GET /agents/`{name}`/messages

List messages in an agent.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Query Parameters**:
- `limit` (optional) - Maximum number of messages to return

**Response**: `200 OK`
```json
[
  {
    "id": "msg-uuid",
    "agent_name": "agent-name",
    "role": "user",
    "content": "Generate a Python script to calculate fibonacci numbers",
    "created_at": "2025-01-20T10:00:00Z"
  },
  {
    "id": "msg-uuid-2", 
    "agent_name": "agent-name",
    "role": "assistant",
    "content": "I'll create a Python script for calculating fibonacci numbers...",
    "created_at": "2025-01-20T10:01:00Z"
  }
]
```

### POST /agents/`{name}`/messages

Send a message to an agent. If the agent is sleeping, it will automatically be woken before processing the message.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

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
  "agent_name": "agent-name", 
  "role": "user",
  "content": "Generate a Python script to calculate fibonacci numbers",
  "created_at": "2025-01-20T10:00:00Z"
}
```

**Note**: When sending a message to a sleeping agent, the API returns `200 OK` immediately and queues an auto-wake task. The agent will be woken and then process the message.

### GET /agents/`{name}`/messages/count

Get message count for agent.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

**Response**: `200 OK`
```json
{
  "count": 25
}
```

### DELETE /agents/`{name}`/messages

Clear all agent messages.

**Authentication**: Required

**Parameters**:
- `id` (path) - Agent ID or name

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
