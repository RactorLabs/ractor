---
sidebar_position: 1
title: CLI Usage Guide
---

# Using the Raworc CLI

The Raworc CLI provides complete command-line access to all functionality for managing Host sessions and runtime operations. This guide covers everything you need to know about using the CLI effectively.

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

## 3. Session Management

Create and manage Host sessions:

### Interactive Sessions (Recommended)

```bash
# Start Host session with API key (REQUIRED for new sessions)
raworc session --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'

# Note: ANTHROPIC_API_KEY is required for all new Host sessions

# Start Host session with instructions
raworc session --instructions "You are a helpful coding Host"

# Start Host session with setup script
raworc session --setup "pip install pandas numpy matplotlib"

# Full configuration
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"sk-ant-key","DATABASE_URL":"mysql://user:pass@host/db"}' \
  --instructions "You are a data analyst Host" \
  --setup "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy"

# Restore previous session
raworc session --restore abc123-def456-789

# Create remix from existing session
raworc session --remix abc123-def456-789
raworc session --remix abc123-def456-789 --data false    # Don't copy data files
raworc session --remix abc123-def456-789 --code false    # Don't copy code files

# In session interface:
# - Type messages directly: "Hello, help me write Python code"
# - Use /status to show session info
# - Use /quit to exit session
```

### API-based Session Management

```bash
# Create new session (requires ANTHROPIC_API_KEY)
raworc api sessions -m post -b '{"secrets":{"ANTHROPIC_API_KEY":"sk-ant-your-key"}}'

# Create session with configuration
raworc api sessions -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Host specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy"
}'

# List all sessions
raworc api sessions

# Get specific session details
raworc api sessions/{session-id}

# Send message to Host
raworc api sessions/{session-id}/messages -m post -b '{"content":"Generate a Python script to calculate fibonacci numbers"}'

# View messages
raworc api sessions/{session-id}/messages

# Get latest messages (limit to last 10)
raworc api "sessions/{session-id}/messages?limit=10"

# Close session (saves resources, preserves data)
raworc api sessions/{session-id}/close -m post

# Restore closed session
raworc api sessions/{session-id}/restore -m post

# Create remix from session
raworc api sessions/{session-id}/remix -m post -b '{
  "data": true,
  "code": true,
  "secrets": false
}'

# Terminate session permanently
raworc api sessions/{session-id} -m delete
```

## 4. Session Configuration Options

### Secrets Configuration

Pass environment variables and API keys to Host sessions:

```bash
# Single secret
raworc session --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'

# Multiple secrets
raworc session --secrets '{
  "ANTHROPIC_API_KEY": "sk-ant-your-key",
  "DATABASE_URL": "mysql://user:pass@host/db",
  "OPENAI_API_KEY": "sk-your-openai-key"
}'
```

### Instructions Configuration

Provide system instructions for the Host:

```bash
# Inline instructions
raworc session --instructions "You are a helpful coding Host specialized in Python"

# Instructions from file
raworc session --instructions ./my-instructions.md
```

### Setup Script Configuration

Run initialization commands in the Host session container:

```bash
# Inline setup
raworc session --setup "pip install pandas numpy matplotlib"

# Multi-line setup
raworc session --setup "#!/bin/bash
echo 'Setting up development environment'
apt-get update
apt-get install -y curl git
pip install pandas numpy matplotlib jupyter"

# Setup from file
raworc session --setup ./setup.sh
```

## 5. Session Remix (Advanced)

Create new sessions based on existing ones with selective copying:

```bash
# Default remix (copies everything)
raworc session --remix {source-session-id}

# Selective copying
raworc session --remix {source-session-id} --data false      # Skip data files
raworc session --remix {source-session-id} --code false      # Skip code files

# Combination
raworc session --remix {source-session-id} --data false --code false

# API version with selective copying
raworc api sessions/{source-session-id}/remix -m post -b '{
  "metadata": {
    "remixed_from": "{source-session-id}",
    "purpose": "experiment with new approach"
  },
  "data": true,
  "code": false
}'
```

### Remix Use Cases

- **Experimentation**: Try different approaches from same starting point
- **Template Sessions**: Create base sessions and remix for new projects
- **A/B Testing**: Compare different configurations from same baseline

## 6. System Maintenance

### Cleanup Operations

```bash
# Clean up all sessions
raworc cleanup --yes

# Complete system reset
raworc reset --yes

# Stop services with cleanup
raworc stop --cleanup
```

### Update Operations

```bash
# Pull latest CLI and images
raworc pull

# Only update CLI
raworc pull --cli-only

# Only pull Docker images
raworc pull --images-only
```

### Troubleshooting

```bash
# Check system health
raworc api health

# View service logs
docker logs raworc_server
docker logs raworc_operator
docker logs raworc_mysql

# Restart services
raworc start --restart
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
- Use close/restore for long-running Host sessions to save resources
- **Always include `ANTHROPIC_API_KEY` secret for Host functionality** - this is required for all new sessions

### Performance Tips
- Use `raworc start --pull` to ensure latest images
- Close unused sessions to save system resources
- Use `raworc stop --cleanup` for complete cleanup
- Use selective remix to avoid copying unnecessary data

### Security Best Practices
- Never commit secrets to version control
- Use file-based instructions/setup instead of inline for complex configurations
- Use selective remix to avoid copying unnecessary files
- Regularly clean up old sessions

## Common Workflows

### Development Environment Setup

```bash
# Start services
raworc start

# Authenticate
raworc auth login --user admin --pass admin

# Create coding Host session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key"}' \
  --instructions "You are a senior developer Host" \
  --setup "pip install black flake8 pytest"
```

### Data Analysis Session

```bash
# Create data science Host session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key","DATABASE_URL":"your-db"}' \
  --instructions "You are a data scientist Host" \
  --setup "pip install pandas numpy matplotlib seaborn jupyter"
```

### Quick Testing

```bash
# Minimal Host session for quick tasks (ANTHROPIC_API_KEY is required)
raworc session --secrets '{"ANTHROPIC_API_KEY":"your-key"}'
```

## Next Steps

- [Sessions Concepts](/docs/concepts/sessions) - Understanding session architecture
- [Complete API Reference](/docs/api/rest-api) - Full REST API documentation  
- [Troubleshooting](/docs/guides/troubleshooting) - Solve common issues