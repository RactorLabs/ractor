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

### Environment Setup

Set your Anthropic API key as an environment variable:

```bash
export ANTHROPIC_API_KEY=sk-ant-your-actual-key
```

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

### Authentication Command Options

| Command | Options | Description |
|---------|---------|-------------|
| `raworc login` | `[-u/--user] [-p/--pass] [-s/--server]` | Generate operator authentication token |
| `raworc auth` | `[-t/--token] [-s/--server]` | Authenticate with token or show status |
| `raworc logout` | | Clear authentication credentials |
| `raworc token` | `[-p/--principal] [-t/--type]` | Create token for principal (User/Operator) |

### Authentication Options Details

| Option | Description | Default | Example |
|--------|-------------|---------|---------|
| `-u, --user <user>` | Operator username | Required | `--user admin` |
| `-p, --pass <pass>` | Password for authentication | Required | `--pass admin` |
| `-s, --server <url>` | Server URL | `http://localhost:9000` | `--server https://my-raworc.com` |
| `-t, --token <token>` | JWT token for authentication | Required for auth | `--token eyJ0eXAi...` |
| `-p, --principal <name>` | Principal name for token creation | Required | `--principal myuser` |
| `-t, --type <type>` | Principal type (User/Operator) | `User` | `--type Operator` |

### Check Server Status
```bash
raworc api version
```

## 3. Session Management

Create and manage Host sessions:

### Session Commands

```bash
# Start new session (default subcommand)
raworc session [options]
raworc session start [options]              # Explicit 'start' command

# Restore existing session
raworc session restore <session-id-or-name>

# Create remix from existing session
raworc session remix <session-id-or-name> [options]

# Publish session for public access
raworc session publish <session-id-or-name> [options]

# Remove session from public access
raworc session unpublish <session-id-or-name>

# Close active session
raworc session close <session-id-or-name>
```

### Starting New Sessions

```bash
# Basic session (uses ANTHROPIC_API_KEY from environment)
raworc session

# Session with name and timeout
raworc session \
  --name "my-analysis-session" \
  --timeout 300

# Session with instructions and setup
raworc session \
  --instructions "You are a helpful coding Host" \
  --setup "pip install pandas numpy matplotlib"

# Instructions from file
raworc session --instructions-file ./instructions.md

# Setup from file
raworc session --setup-file ./setup.sh

# Full configuration with prompt
raworc session \
  --name "data-project" \
  --secrets '{"DATABASE_URL":"mysql://user:pass@host/db"}' \
  --instructions "You are a data analyst Host" \
  --setup "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy" \
  --prompt "Hello, let's start analyzing the customer data" \
  --timeout 600
```

### Session Start Options

| Option | Description | Example |
|--------|-------------|---------|
| `-n, --name <name>` | Name for the session | `--name "my-session"` |
| `-t, --timeout <seconds>` | Session timeout in seconds (default: 300) | `--timeout 600` |
| `-S, --secrets <json>` | JSON secrets for session environment | `--secrets '{"DB_URL":"..."}'` |
| `-i, --instructions <text>` | Direct instructions text | `--instructions "You are a data analyst"` |
| `-if, --instructions-file <file>` | Path to instructions file | `--instructions-file ./instructions.md` |
| `-s, --setup <text>` | Direct setup script text | `--setup "pip install pandas"` |
| `-sf, --setup-file <file>` | Path to setup script file | `--setup-file ./setup.sh` |
| `-p, --prompt <text>` | Prompt to send after creation | `--prompt "Hello, let's start"` |

### Restoring Sessions

```bash
# Restore by ID or name
raworc session restore abc123-def456-789
raworc session restore my-session-name

# Restore with immediate prompt
raworc session restore my-session --prompt "Continue the analysis from yesterday"
```

### Session Restore Options

| Option | Description | Example |
|--------|-------------|---------|
| `-p, --prompt <text>` | Prompt to send after restoring | `--prompt "Continue work"` |

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

### Session Remix Options

| Option | Description | Default | Example |
|--------|-------------|---------|---------|
| `-n, --name <name>` | Name for the new session | Auto-generated | `--name "experiment-1"` |
| `-d, --data <boolean>` | Include data files | `true` | `--data false` |
| `-c, --code <boolean>` | Include code files | `true` | `--code false` |
| `-s, --secrets <boolean>` | Include secrets | `true` | `--secrets false` |
| `-p, --prompt <text>` | Prompt to send after creation | None | `--prompt "Try new approach"` |

**Note**: Canvas files are always included in remixes.

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

# Close active session
raworc session close my-session
raworc session close abc-123-def
```

### Session Close Management

Closing sessions saves system resources while preserving all data. Closed sessions can be restored later:

```bash
# Close any active session
raworc session close my-session

# Close shows current state before closing
# Provides instructions for restore/remix operations
```

### Session Publish Options

| Option | Description | Default | Example |
|--------|-------------|---------|---------|
| `-d, --data <boolean>` | Allow data remix | `true` | `--data false` |
| `-c, --code <boolean>` | Allow code remix | `true` | `--code false` |
| `-s, --secrets <boolean>` | Allow secrets remix | `true` | `--secrets false` |

**Note**: Canvas remixing is always allowed for published sessions.

### Interactive Session Interface

Once in an interactive session, you have access to powerful session commands and a clean interface with visual state indicators.

#### Session State Indicators

The CLI uses professional flat geometric icons to show session status:

- `◯ initializing...` - Session container starting up
- `● ready` - Session idle and ready for requests  
- `◐ working...` - Session actively processing requests
- `◻ closed` - Session suspended (container stopped)
- `◆ error` - Session encountered an error

#### Interactive Session Commands

```bash
# Communication
# Type messages directly: "Hello, help me write Python code"

# Session management commands:
/help, /h                    # Show all available commands
/status                      # Display session status and information
/timeout <seconds>           # Change session timeout (1-3600 seconds)
/name <name>                 # Change session name (alphanumeric and hyphens)
/detach, /d                  # Detach from session (keeps session running)
/quit, /q                    # End the session completely
```

#### Example Interactive Session

```bash
$ raworc session --name "coding-project"

┌─────────────────────────────────────┐
│ ◊ Session Start                     │
│ SessionId: coding-project           │
│ User: admin (Operator)              │
│ Commands: /help (for commands)      │
└─────────────────────────────────────┘

◯ initializing...
──────────────────────────────────────────────────
> Hello, I need help with Python

● Search
└─ Searching for Python help resources

Based on your request, I can help you with Python programming. What specific aspect would you like assistance with?

● ready
──────────────────────────────────────────────────
> /timeout 600

✓ Session timeout updated to 600 seconds (10 minutes)

● ready  
──────────────────────────────────────────────────
> /detach

◊ Detached from session. Session continues running.
Reconnect with: raworc session restore coding-project
```

### API-based Session Management

```bash
# Create new session (requires ANTHROPIC_API_KEY)
raworc api sessions -m post -b '{"secrets":{}}'

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
# Session with environment variables only
raworc session
# Multiple secrets
raworc session --secrets '{
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
raworc session --instructions-file ./my-instructions.md
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
raworc session --setup-file ./setup.sh
```

## 5. Session Remix (Advanced)

Create new sessions based on existing ones with selective copying:

```bash
# Default remix (copies everything)
raworc session remix {source-session-id}

# Selective copying
raworc session remix {source-session-id} --data false      # Skip data files
raworc session remix {source-session-id} --code false      # Skip code files

# Combination
raworc session remix {source-session-id} --data false --code false

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
# Clean session containers only (preserves core services and volumes)
raworc clean

# Complete Docker reset (nuclear option)
raworc reset
raworc reset --yes                    # Auto-confirm without prompting
raworc reset --services-only          # Only stop services, don't clean Docker

# Stop services with cleanup
raworc stop --cleanup
```

### Service Management Options

| Command | Options | Description |
|---------|---------|-------------|
| `raworc start` | `[-r/--restart] [components...]` | Start services, optionally restart existing |
| `raworc stop` | `[-c/--cleanup] [components...]` | Stop services, optionally clean session containers |
| `raworc clean` | | Clean session containers (preserves core services) |
| `raworc reset` | `[-y/--yes] [-s/--services-only]` | Complete cleanup with optional confirmation |

### Update Operations

```bash
# Pull latest CLI and images
raworc pull

# Only update CLI
raworc pull --cli-only

# Only pull Docker images
raworc pull --images-only
```

### Pull Command Options

| Option | Description | Example |
|--------|-------------|---------|
| `-c, --cli-only` | Only update CLI, skip Docker images | `raworc pull --cli-only` |
| `-i, --images-only` | Only pull Docker images, skip CLI update | `raworc pull --images-only` |

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
- Re-authenticate if tokens expire: `raworc login`

### Session Management
- Sessions persist until explicitly deleted
- Interactive sessions auto-cleanup on exit (`/quit`)
- Use close/restore for long-running Host sessions to save resources
- **Set `ANTHROPIC_API_KEY` environment variable** - required for all new sessions

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

## Common Use Cases

Here are practical examples for different automation scenarios using Computer Use Agents:

### Web Automation

```bash
# Create a web automation Host session
raworc session \
  --instructions "You automate web tasks. Use browsers to fill forms, extract data, and navigate websites." \
  --setup "pip install selenium beautifulsoup4 requests"
```

### Document Processing

```bash
# Create a document processing Host session
raworc session \
  --instructions "You process documents and files. Generate reports, manipulate spreadsheets, and handle data workflows." \
  --setup "pip install pandas openpyxl python-docx pdfplumber"
```

### System Administration

```bash
# Create a system automation Host session
raworc session \
  --instructions "You automate system administration tasks. Manage servers, deploy applications, and monitor systems." \
  --setup "apt-get update && apt-get install -y curl jq && pip install fabric paramiko"
```

### Data Analysis & Visualization

```bash
# Create a data science Host session
raworc session \
  --secrets '{"DATABASE_URL":"postgresql://user:pass@host/db"}' \
  --instructions "You are a data scientist Host. Analyze data, create visualizations, and generate insights." \
  --setup "pip install pandas numpy matplotlib seaborn jupyter plotly"
```

### Development & Coding

```bash
# Create a development Host session
raworc session \
  --instructions "You are a senior developer Host. Write code, debug issues, and manage repositories." \
  --setup "pip install black flake8 pytest mypy && npm install -g typescript"
```

### Quick Testing & Experimentation

```bash
# Minimal Host session for quick tasks
raworc session \
  --instructions "You help with quick tasks and experimentation."
```

### AI Agent Development

```bash
# Create an AI agent development session
raworc session \
  --instructions "You develop AI agents and automation tools using frameworks like LangGraph, CrewAI, and AutoGen." \
  --setup "pip install langgraph crewai autogen langchain openai"
```

## Next Steps

- [Sessions Concepts](/docs/concepts/sessions) - Understanding session architecture
- [Complete API Reference](/docs/api/rest-api-reference) - Full REST API documentation  
- [Troubleshooting](/docs/guides/troubleshooting) - Solve common issues