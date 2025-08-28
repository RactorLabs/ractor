---
sidebar_position: 1
title: CLI Usage Guide
---

# Using the Raworc CLI

The Raworc CLI provides complete command-line access to all functionality for managing AI agent deployments, spaces, sessions, and runtime operations. This guide covers everything you need to know about using the CLI effectively.

## Prerequisites

- **Node.js 16+**: For the Raworc CLI
- **Docker**: Docker Engine 20.10+ and Docker Compose v2+
- **Anthropic API Key**: Required - get one at [console.anthropic.com](https://console.anthropic.com)

## Installation

```bash
npm install -g @raworc/cli
```

## 1. Service Management

Start and manage Raworc services:

```bash
# Start all services
raworc start
raworc start --pull             # Pull latest images first

# Stop services
raworc stop
raworc stop --cleanup           # Also clean up session containers

# Check service status
raworc api health
```

## 2. Authentication

Authenticate with the Raworc server:

```bash
# Start services first
raworc start

# Authenticate with default credentials (localhost)
raworc auth login --user admin --pass admin

# Authenticate with remote server
raworc auth login --server https://your-raworc-server.com --user admin --pass admin

# Token-based auth (recommended for production)
raworc auth login --token sk-ant-your-token

# Token auth with remote server
raworc auth login --server https://your-raworc-server.com --token sk-ant-your-token

# Check authentication status
raworc auth

# Check server health
raworc api health
```

## 3. Space Setup

Set up spaces and secrets for your projects:

### Space Operations

```bash
# List all spaces (default space always exists)
raworc api spaces

# Get specific space details
raworc api spaces/default

# Create new space (admin only)
raworc api spaces -m post -b '{
  "name": "staging",
  "description": "Staging environment",
  "settings": {
    "environment": "staging"
  }
}'
```

### Essential: Set up API Keys

```bash
# Add your Anthropic API key (required for AI functionality)
raworc api spaces/default/secrets -m post -b '{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-your-actual-key",
  "description": "Claude API key"
}'

# Add other API keys as needed
raworc api spaces/default/secrets -m post -b '{
  "key_name": "OPENAI_API_KEY",
  "value": "sk-your-openai-key",
  "description": "OpenAI API key"
}'
```

### Manage Secrets

```bash
# List secrets (metadata only)
raworc api spaces/default/secrets

# List secrets with values (requires permissions)
raworc api "spaces/default/secrets?show_values=true"

# Update secret value
raworc api spaces/default/secrets/API_KEY -m put -b '{"value":"new-secret-value"}'

# Delete secret
raworc api spaces/default/secrets/API_KEY -m delete
```

## 4. Agent Management

Deploy custom agents to enhance functionality:

```bash
# List agents in space
raworc api spaces/default/agents

# Add a demo agent
raworc api spaces/default/agents -m post -b '{
  "name": "data-analyzer",
  "description": "Data analysis specialist", 
  "purpose": "analyze data, create visualizations, statistical analysis",
  "source_repo": "Raworc/raworc-agent-python-demo",
  "source_branch": "main"
}'

# Trigger space build (required after adding agents)
raworc api spaces/default/build -m post

# Check build status
raworc api spaces/default/build/latest
```

## 5. Session Management

Create and manage AI agent sessions:

### Interactive Sessions

```bash
# Start interactive session (easiest way)
raworc session

# Start session in specific space
raworc session --space production

# In session interface:
# - Type messages directly: "Hello, help me write Python code"
# - Use /status to show session info
# - Use /quit to exit session
```

### API-based Session Management

```bash
# Create new session
raworc api sessions -m post -b '{"space":"default"}'

# List all sessions
raworc api sessions

# Get specific session details
raworc api sessions/{session-id}

# Send message to agent
raworc api sessions/{session-id}/messages -m post -b '{"content":"Generate a Python script to calculate fibonacci numbers"}'

# View messages
raworc api sessions/{session-id}/messages

# Get latest messages (limit to last 10)
raworc api "sessions/{session-id}/messages?limit=10"

# Close session (saves resources)
raworc api sessions/{session-id}/close -m post

# Restore session
raworc api sessions/{session-id}/restore -m post

# Terminate session
raworc api sessions/{session-id} -m delete
```

## Advanced Operations

### Space Management

```bash
# Update space (admin only)
raworc api spaces/staging -m put -b '{"description":"Updated staging space"}'

# Delete space (admin only)
raworc api spaces/staging -m delete
```

### Multi-Environment Setup

```bash
# Create production space
raworc api spaces -m post -b '{
  "name": "production",
  "description": "Production environment"
}'

# Set production secrets
raworc api spaces/production/secrets -m post -b '{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-production-key"
}'

# Use production space
raworc session --space production
```

## Error Handling

Common error responses and solutions:

```json
// 401 Unauthorized - Re-authenticate
{
  "error": {
    "code": "UNAUTHORIZED",
    "message": "Invalid or expired token"
  }
}

// 400 Bad Request - Missing API key
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "ANTHROPIC_API_KEY secret is required"
  }
}

// 404 Not Found - Resource doesn't exist
{
  "error": {
    "code": "NOT_FOUND", 
    "message": "Session not found"
  }
}
```

## CLI Tips and Best Practices

### General Usage
- Use `raworc session` for interactive development and testing
- Use `raworc api` for automation and scripting
- Methods are case-insensitive: `-m post` or `-m POST` both work
- POST requests automatically get empty `{}` body if none specified

### Authentication
- CLI stores tokens securely in `~/.raworc/`
- Default server: `http://localhost:9000` (for local development)
- Use `--server` flag to connect to remote Raworc instances
- Check auth status anytime with `raworc auth`
- Re-authenticate if tokens expire: `raworc auth login`

### Session Management
- Sessions persist until explicitly deleted
- Interactive sessions auto-cleanup on exit (`/quit`)
- Use close/restore for long-running sessions to save resources

### Performance Tips
- Use `raworc start --pull` to ensure latest images
- Pause unused sessions to save system resources
- Use `raworc stop --cleanup` for complete cleanup

## Next Steps

- [Complete API Reference](/docs/api/rest-api) - Full REST API documentation  
- [Bring Your Own Agent](/docs/guides/bring-your-own-agent) - Deploy custom agents
- [Architecture Overview](/docs/concepts/architecture) - Understanding system design
- [Troubleshooting](/docs/guides/troubleshooting) - Solve common issues