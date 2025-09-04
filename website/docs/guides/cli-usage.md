---
sidebar_position: 1
title: CLI Usage Guide
---

# Using the Raworc CLI

The Raworc CLI provides complete command-line access to all functionality for managing computer use agents and runtime operations. This guide covers everything you need to know about using the CLI effectively.

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
raworc stop --cleanup           # Also clean up agent containers

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

## 3. Agent Management

Create and manage computer use agents:

### Agent Commands

```bash
# Start new agent (default subcommand)
raworc agent [options]
raworc agent start [options]              # Explicit 'start' command

# Wake existing agent
raworc agent wake <agent-name-or-id>

# Create remix from existing agent
raworc agent remix <agent-name-or-id> [options]

# Publish agent for public access
raworc agent publish <agent-name-or-id> [options]

# Remove agent from public access
raworc agent unpublish <agent-name-or-id>

# Sleep active agent
raworc agent sleep <agent-name-or-id>
```

### Starting New Agents

```bash
# Basic agent (uses ANTHROPIC_API_KEY from environment)
raworc agent

# Agent with name and timeout
raworc agent \
  --name "my-analysis-agent" \
  --timeout 300

# Agent with instructions and setup
raworc agent \
  --instructions "You are a helpful coding agent" \
  --setup "pip install pandas numpy matplotlib"

# Instructions from file
raworc agent --instructions-file ./instructions.md

# Setup from file
raworc agent --setup-file ./setup.sh

# Full configuration with prompt
raworc agent \
  --name "data-project" \
  --secrets '{"DATABASE_URL":"mysql://user:pass@host/db"}' \
  --instructions "You are a data analyst agent" \
  --setup "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy" \
  --prompt "Hello, let's start analyzing the customer data" \
  --timeout 600
```

### Agent Start Options

| Option | Description | Example |
|--------|-------------|---------|
| `-n, --name <name>` | Name for the agent | `--name "my-agent"` |
| `-t, --timeout <seconds>` | Agent timeout in seconds (default: 300) | `--timeout 600` |
| `-S, --secrets <json>` | JSON secrets for agent environment | `--secrets '{"DB_URL":"..."}'` |
| `-i, --instructions <text>` | Direct instructions text | `--instructions "You are a data analyst"` |
| `-if, --instructions-file <file>` | Path to instructions file | `--instructions-file ./instructions.md` |
| `-s, --setup <text>` | Direct setup script text | `--setup "pip install pandas"` |
| `-sf, --setup-file <file>` | Path to setup script file | `--setup-file ./setup.sh` |
| `-p, --prompt <text>` | Prompt to send after creation | `--prompt "Hello, let's start"` |

### Restoring Agents

```bash
# Restore by ID or name
raworc agent restore abc123-def456-789
raworc agent restore my-agent-name

# Restore with immediate prompt
raworc agent restore my-agent --prompt "Continue the analysis from yesterday"
```

### Agent Restore Options

| Option | Description | Example |
|--------|-------------|---------|
| `-p, --prompt <text>` | Prompt to send after waking | `--prompt "Continue work"` |

### Remixing Agents

```bash
# Basic remix (copies everything by default)
raworc agent remix abc123-def456-789

# Remix by name with new name
raworc agent remix my-agent --name "experiment-1"

# Selective copying
raworc agent remix my-agent \
  --name "data-only-version" \
  --data true \
  --code false \
  --secrets false

# Remix with immediate prompt
raworc agent remix my-agent \
  --name "alternative-approach" \
  --prompt "Try a different analysis method"
```

### Agent Remix Options

| Option | Description | Default | Example |
|--------|-------------|---------|---------|
| `-n, --name <name>` | Name for the new agent | Auto-generated | `--name "experiment-1"` |
| `-d, --data <boolean>` | Include data files | `true` | `--data false` |
| `-c, --code <boolean>` | Include code files | `true` | `--code false` |
| `-s, --secrets <boolean>` | Include secrets | `true` | `--secrets false` |
| `-p, --prompt <text>` | Prompt to send after creation | None | `--prompt "Try new approach"` |

**Note**: Content files are always included in remixes.

### Publishing Agents

```bash
# Publish with all remix permissions
raworc agent publish my-agent

# Publish with selective permissions
raworc agent publish my-agent \
  --data true \
  --code true \
  --secrets false

# Unpublish agent
raworc agent unpublish my-agent

# Close active agent
raworc agent close my-agent
raworc agent close abc-123-def
```

### Agent Close Management

Closing agents saves system resources while preserving all data. Closed agents can be restored later:

```bash
# Close any active agent
raworc agent close my-agent

# Close shows current state before closing
# Provides instructions for restore/remix operations
```

### Agent Publish Options

| Option | Description | Default | Example |
|--------|-------------|---------|---------|
| `-d, --data <boolean>` | Allow data remix | `true` | `--data false` |
| `-c, --code <boolean>` | Allow code remix | `true` | `--code false` |
| `-s, --secrets <boolean>` | Allow secrets remix | `true` | `--secrets false` |

**Note**: Content remixing is always allowed for published agents.

### Interactive Agent Interface

Once in an interactive agent, you have access to powerful agent commands and a clean interface with visual state indicators.

#### Agent State Indicators

The CLI uses professional flat geometric icons to show agent status:

- `◯ initializing...` - Agent container starting up
- `● ready` - Agent idle and ready for requests  
- `◐ working...` - Agent actively processing requests
- `◻ sleeping` - Agent suspended (container stopped)
- `◆ error` - Agent encountered an error

#### Interactive Agent Commands

```bash
# Communication
# Type messages directly: "Hello, help me write Python code"

# Agent management commands:
/help, /h                    # Show all available commands
/status                      # Display agent status and information
/timeout <seconds>           # Change agent timeout (1-3600 seconds)
/name <name>                 # Change agent name (alphanumeric and hyphens)
/detach, /d                  # Detach from agent (keeps agent running)
/quit, /q                    # End the agent completely
```

#### Example Interactive Agent

```bash
$ raworc agent --name "coding-project"

┌─────────────────────────────────────┐
│ ◊ Agent Start                     │
│ Agent: coding-project             │
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

✓ Agent timeout updated to 600 seconds (10 minutes)

● ready  
──────────────────────────────────────────────────
> /detach

◊ Detached from agent. Agent continues running.
Reconnect with: raworc agent restore coding-project
```

### API-based Agent Management

```bash
# Create new agent (requires ANTHROPIC_API_KEY)
raworc api agents -m post -b '{"secrets":{}}'

# Create agent with full configuration
raworc api agents -m post -b '{
  "name": "my-agent",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful agent specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy",
  "timeout_seconds": 300
}'

# List all agents
raworc api agents

# Get specific agent details (by ID or name)
raworc api agents/{agent-name-or-id}

# Send message to agent
raworc api agents/{agent-name}/messages -m post -b '{"content":"Generate a Python script to calculate fibonacci numbers"}'

# View messages
raworc api agents/{agent-name}/messages

# Get latest messages (limit to last 10)
raworc api "agents/{agent-name}/messages?limit=10"

# Close agent (saves resources, preserves data)
raworc api agents/{agent-name}/sleep -m post

# Restore sleeping agent (with optional prompt)
raworc api agents/{agent-name-or-id}/wake -m post
raworc api agents/{agent-name-or-id}/wake -m post -b '{"prompt":"Continue from where we left off"}'

# Mark agent as busy (prevents timeout)
raworc api agents/{agent-name-or-id}/busy -m post

# Mark agent as idle (enables timeout)
raworc api agents/{agent-name-or-id}/idle -m post

# Create remix from agent
raworc api agents/{agent-name-or-id}/remix -m post -b '{
  "name": "experiment-1",
  "data": true,
  "code": true,
  "secrets": false
}'

# Publish agent for public access
raworc api agents/{agent-name-or-id}/publish -m post -b '{
  "data": true,
  "code": true,
  "secrets": false
}'

# Unpublish agent
raworc api agents/{agent-name-or-id}/unpublish -m post

# View published agents (no auth required)
raworc api published/agents

# Get published agent (no auth required)
raworc api published/agents/{agent-name-or-id}

# Terminate agent permanently
raworc api agents/{agent-name-or-id} -m delete
```

## 4. Agent Configuration Options

### Secrets Configuration

Pass environment variables and API keys to computer use agents:

```bash
# Agent with environment variables only
raworc agent
# Multiple secrets
raworc agent --secrets '{
  "DATABASE_URL": "mysql://user:pass@host/db",
  "OPENAI_API_KEY": "sk-your-openai-key"
}'
```

### Instructions Configuration

Provide system instructions for the agent:

```bash
# Inline instructions
raworc agent --instructions "You are a helpful coding agent specialized in Python"

# Instructions from file
raworc agent --instructions-file ./my-instructions.md
```

### Setup Script Configuration

Run initialization commands in the agent container:

```bash
# Inline setup
raworc agent --setup "pip install pandas numpy matplotlib"

# Multi-line setup
raworc agent --setup "#!/bin/bash
echo 'Setting up development environment'
apt-get update
apt-get install -y curl git
pip install pandas numpy matplotlib jupyter"

# Setup from file
raworc agent --setup-file ./setup.sh
```

## 5. Agent Remix (Advanced)

Create new agents based on existing ones with selective copying:

```bash
# Default remix (copies everything)
raworc agent remix {source-agent-name}

# Selective copying
raworc agent remix {source-agent-name} --data false      # Skip data files
raworc agent remix {source-agent-name} --code false      # Skip code files

# Combination
raworc agent remix {source-agent-name} --data false --code false

# API version with selective copying
raworc api agents/{source-agent-name}/remix -m post -b '{
  "metadata": {
    "remixed_from": "{source-agent-name}",
    "purpose": "experiment with new approach"
  },
  "data": true,
  "code": false
}'
```

### Remix Use Cases

- **Experimentation**: Try different approaches from same starting point
- **Template Agents**: Create base agents and remix for new projects
- **A/B Testing**: Compare different configurations from same baseline

## 6. System Maintenance

### Cleanup Operations

```bash
# Clean agent containers only (preserves core services and volumes)
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
| `raworc stop` | `[-c/--cleanup] [components...]` | Stop services, optionally clean agent containers |
| `raworc clean` | | Clean agent containers (preserves core services) |
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
    "message": "Agent not found"
  }
}
```

## CLI Tips and Best Practices

### General Usage
- Use `raworc agent` for interactive development and testing
- Use `raworc api` for automation and scripting
- Methods are case-insensitive: `-m post` or `-m POST` both work
- POST requests automatically get empty `{}` body if none specified

### Authentication
- CLI stores tokens securely in `~/.raworc/`
- Default server: `http://localhost:9000` (for local development)
- Use `--server` flag to connect to remote Raworc instances
- Check auth status anytime with `raworc auth`
- Re-authenticate if tokens expire: `raworc login`

### Agent Management
- Agents persist until explicitly deleted
- Interactive agents auto-cleanup on exit (`/quit`)
- Use sleep/wake for long-running computer use agents to save resources
- **Set `ANTHROPIC_API_KEY` environment variable** - required for all new agents

### Performance Tips
- Use `raworc start --pull` to ensure latest images
- Close unused agents to save system resources
- Use `raworc stop --cleanup` for complete cleanup
- Use selective remix to avoid copying unnecessary data

### Security Best Practices
- Never commit secrets to version control
- Use file-based instructions/setup instead of inline for complex configurations
- Use selective remix to avoid copying unnecessary files
- Regularly clean up old agents

## Common Use Cases

Here are practical examples for different automation scenarios using Computer Use Agents:

### Web Automation

```bash
# Create a web automation agent
raworc agent \
  --instructions "You automate web tasks. Use browsers to fill forms, extract data, and navigate websites." \
  --setup "pip install selenium beautifulsoup4 requests"
```

### Document Processing

```bash
# Create a document processing agent
raworc agent \
  --instructions "You process documents and files. Generate reports, manipulate spreadsheets, and handle data workflows." \
  --setup "pip install pandas openpyxl python-docx pdfplumber"
```

### System Administration

```bash
# Create a system automation agent
raworc agent \
  --instructions "You automate system administration tasks. Manage servers, deploy applications, and monitor systems." \
  --setup "apt-get update && apt-get install -y curl jq && pip install fabric paramiko"
```

### Data Analysis & Visualization

```bash
# Create a data science agent
raworc agent \
  --secrets '{"DATABASE_URL":"postgresql://user:pass@host/db"}' \
  --instructions "You are a data scientist agent. Analyze data, create visualizations, and generate insights." \
  --setup "pip install pandas numpy matplotlib seaborn jupyter plotly"
```

### Development & Coding

```bash
# Create a development agent
raworc agent \
  --instructions "You are a senior developer agent. Write code, debug issues, and manage repositories." \
  --setup "pip install black flake8 pytest mypy && npm install -g typescript"
```

### Quick Testing & Experimentation

```bash
# Minimal agent for quick tasks
raworc agent \
  --instructions "You help with quick tasks and experimentation."
```

### AI Agent Development

```bash
# Create an AI agent development agent
raworc agent \
  --instructions "You develop AI agents and automation tools using frameworks like LangGraph, CrewAI, and AutoGen." \
  --setup "pip install langgraph crewai autogen langchain openai"
```

## Next Steps

- [Agent Concepts](/docs/concepts/agents) - Understanding agent architecture
- [Complete API Reference](/docs/api/rest-api-reference) - Full REST API documentation  
- [Troubleshooting](/docs/guides/troubleshooting) - Solve common issues