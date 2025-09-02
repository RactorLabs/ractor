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
raworc api version
```

## 2. Authentication

Raworc uses a two-step authentication process:

### Step 1: Generate Token (Login)
```bash
# Generate operator token (doesn't authenticate CLI)
raworc login -u admin -p admin

# Generate token with remote server
raworc login -s https://your-server.com -u admin -p admin
```

### Step 2: Authenticate CLI with Token
```bash
# Use token to authenticate CLI
raworc auth -t <jwt-token>

# Token auth with remote server
raworc auth -s https://your-server.com -t <jwt-token>

# Check authentication status
raworc auth

# Clear authentication
raworc logout
```

### Token Creation for Principals
```bash
# Create token for a user (requires authentication)
raworc token -p myuser -t User

# Create operator token
raworc token -p newoperator -t Operator
```

### Check Server Status
```bash
raworc api version
```

## 3. Session Management

Create and manage Host sessions:

### Session Subcommands

```bash
# Start new session (default subcommand)
raworc session start [options]
raworc session [options]              # Shorthand for 'start'

# Restore existing session
raworc session restore <session-id-or-name>

# Create remix from existing session
raworc session remix <session-id-or-name> [options]

# Publish session for public access
raworc session publish <session-id-or-name> [options]

# Remove session from public access
raworc session unpublish <session-id-or-name>
```

### Starting New Sessions

```bash
# Basic session (ANTHROPIC_API_KEY required for all new sessions)
raworc session start --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'

# Session with name and timeout
raworc session start \\
  --name "my-analysis-session" \\
  --timeout 300 \\
  --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'

# Session with instructions and setup
raworc session start \\
  --instructions "You are a helpful coding Host" \\
  --setup "pip install pandas numpy matplotlib" \\
  --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'

# Instructions from file
raworc session start --instructions-file ./instructions.md --secrets '{"ANTHROPIC_API_KEY":"sk-ant-key"}'

# Setup from file
raworc session start --setup-file ./setup.sh --secrets '{"ANTHROPIC_API_KEY":"sk-ant-key"}'

# Full configuration with prompt
raworc session start \
  --name "data-project" \
  --secrets '{"ANTHROPIC_API_KEY":"sk-ant-key","DATABASE_URL":"mysql://user:pass@host/db"}' \
  --instructions "You are a data analyst Host" \
  --setup "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy" \
  --prompt "Hello, let's start analyzing the customer data" \
  --timeout 600
```

### Restoring Sessions

```bash
# Restore by ID or name
raworc session restore abc123-def456-789
raworc session restore my-session-name

# Restore with immediate prompt
raworc session restore my-session --prompt "Continue the analysis from yesterday"
```

### Remixing Sessions

```bash
# Basic remix (copies everything by default)
raworc session remix abc123-def456-789

# Remix by name with new name
raworc session remix my-session --name "experiment-1"

# Selective copying
raworc session remix my-session \
  --name "data-only-version" \
  --data true \
  --code false \
  --secrets false

# Remix with immediate prompt
raworc session remix my-session \
  --name "alternative-approach" \
  --prompt "Try a different analysis method"
```

### Publishing Sessions

```bash
# Publish with all remix permissions
raworc session publish my-session

# Publish with selective permissions
raworc session publish my-session \
  --data true \
  --code true \
  --secrets false

# Unpublish session
raworc session unpublish my-session
```

### Interactive Session Interface

```bash
# In any interactive session:
# - Type messages directly: "Hello, help me write Python code"
# - Use /status to show session info
# - Use /quit to exit session
```

### API-based Session Management

```bash
# Create new session (requires ANTHROPIC_API_KEY)
raworc api sessions -m post -b '{"secrets":{"ANTHROPIC_API_KEY":"sk-ant-your-key"}}'

# Create session with full configuration
raworc api sessions -m post -b '{
  "name": "my-session",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Host specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy",
  "timeout_seconds": 300
}'

# List all sessions
raworc api sessions

# Get specific session details (by ID or name)
raworc api sessions/{session-id-or-name}

# Send message to Host
raworc api sessions/{session-id}/messages -m post -b '{"content":"Generate a Python script to calculate fibonacci numbers"}'

# View messages
raworc api sessions/{session-id}/messages

# Get latest messages (limit to last 10)
raworc api "sessions/{session-id}/messages?limit=10"

# Close session (saves resources, preserves data)
raworc api sessions/{session-id}/close -m post

# Restore closed session (with optional prompt)
raworc api sessions/{session-id-or-name}/restore -m post
raworc api sessions/{session-id-or-name}/restore -m post -b '{"prompt":"Continue from where we left off"}'

# Mark session as busy (prevents timeout)
raworc api sessions/{session-id-or-name}/busy -m post

# Mark session as idle (enables timeout)
raworc api sessions/{session-id-or-name}/idle -m post

# Create remix from session
raworc api sessions/{session-id-or-name}/remix -m post -b '{
  "name": "experiment-1",
  "data": true,
  "code": true,
  "secrets": false
}'

# Publish session for public access
raworc api sessions/{session-id-or-name}/publish -m post -b '{
  "data": true,
  "code": true,
  "secrets": false
}'

# Unpublish session
raworc api sessions/{session-id-or-name}/unpublish -m post

# View published sessions (no auth required)
raworc api published/sessions

# Get published session (no auth required)
raworc api published/sessions/{session-id-or-name}

# Terminate session permanently
raworc api sessions/{session-id-or-name} -m delete
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
# Clean containers only
raworc clean

# Clean containers and images (preserves volumes)
raworc clean --all

# Auto-confirm cleanup
raworc clean -y

# Complete Docker reset (nuclear option)
raworc reset -y

# Services-only cleanup (skip Docker cleanup)
raworc reset -s

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
# Check system status
raworc api version

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