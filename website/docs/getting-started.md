---
sidebar_position: 2
title: Getting Started
---

# Getting Started with Raworc

Get started with the Remote Agentic Work Orchestrator in just a few commands. Raworc provides dedicated remote computers with Host to automate any manual work with enterprise-grade reliability.

## Prerequisites

- **Node.js 16+**: For the Raworc CLI
- **Docker**: Docker Engine 20.10+ and Docker Compose v2+ 
- **Anthropic API Key**: Required for AI functionality - get one at [console.anthropic.com](https://console.anthropic.com)

### Environment Setup

Set your Anthropic API key as an environment variable:

```bash
export ANTHROPIC_API_KEY=sk-ant-your-actual-key
```

## Quick Start (30 seconds)

### 1. Install Raworc CLI

```bash
npm install -g @raworc/cli
```

### 2. Start Services

```bash
raworc start
```

### 3. Authenticate

```bash
# Step 1: Get authentication token
raworc login --user admin --pass admin

# Step 2: Authenticate CLI with the token
raworc auth -t <jwt-token-from-step-1>
```

### 4. Start Your First Session

```bash
raworc session
```

**Note**: Make sure you have set the `ANTHROPIC_API_KEY` environment variable as shown in the prerequisites.

That's it! You now have a running Host session.

## Session Configuration

### Basic Session

```bash
# Create new session (uses ANTHROPIC_API_KEY from environment)
raworc session
```

**Note**: The Anthropic API key environment variable is required for all new sessions.

### Session with Instructions

```bash
raworc session --instructions "You are a helpful coding assistant specialized in Python"
```

### Session with Setup Script

```bash
raworc session --setup "pip install pandas numpy matplotlib"
```

For more advanced session configuration options, see the [CLI Usage Guide](/docs/guides/cli-usage#4-session-configuration-options).

## Interactive Session Usage

Once in a session, you can interact directly with the Host using the clean CLI interface:

```bash
$ raworc session

┌─────────────────────────────────────┐
│ ◊ Session Start                     │
│ SessionId: abc123-def456-789        │
│ User: admin (Operator)              │
│ Commands: /help (for commands)      │
└─────────────────────────────────────┘

◯ initializing...
──────────────────────────────────────────────────
> Hello, how can you help me?

I'm a Host that can help you with various tasks including:
- Writing and debugging code
- Data analysis and visualization  
- File management and organization
- Web research and information gathering
- And much more!

● ready
──────────────────────────────────────────────────
> Create a Python script to calculate fibonacci numbers

● Edit
└─ Creating fibonacci.py

I'll create a Python script to calculate Fibonacci numbers for you.

[Creates fibonacci.py with implementation]

You: /quit
👋 Ending session...
```

### Session Commands

- **Regular messages**: Just type your request
- **`/quit`** or **`/exit`** - End the session
- **`/status`** - Show session information

## Session Management

### List Your Sessions

```bash
raworc api sessions
```

### Restore Previous Session

```bash
raworc session restore abc123-def456-789
```

### Create Session Remix

```bash
# Remix from existing session
raworc session remix abc123-def456-789

# Selective remix options
raworc session remix abc123-def456-789 --data false    # Don't copy data files
raworc session remix abc123-def456-789 --code false    # Don't copy code files
```

## Direct API Usage

### Create Session via API

```bash
# Basic session
raworc api sessions -m POST -b '{}'

# Session with configuration
raworc api sessions -m POST -b '{
  "secrets": {
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Host specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy"
}'
```

### Send Messages to Session

```bash
raworc api sessions/{session-name}/messages -m POST -b '{
  "content": "Generate a Python script to calculate fibonacci numbers"
}'
```

### Session Lifecycle

```bash
# Close session (saves resources, preserves data)
raworc api sessions/{session-name}/close -m POST

# Restore closed session
raworc api sessions/{session-name}/restore -m POST

# Delete session permanently
raworc api sessions/{session-name} -m DELETE
```

## Troubleshooting

### Services won't start
```bash
raworc stop
raworc start --restart
```

### Check system status
```bash
raworc api version
```

### View service logs
```bash
docker logs raworc_server
docker logs raworc_operator
docker logs raworc_mysql
```

### Complete reset
```bash
raworc reset --yes
```


## Next Steps

Now that you have Raworc running:

- **[Common Use Cases](/docs/guides/cli-usage#common-use-cases)** - Practical examples for different automation scenarios
- **[CLI Usage Guide](/docs/guides/cli-usage)** - Master all CLI commands and features
- **[Sessions Concepts](/docs/concepts/sessions)** - Understand session architecture and lifecycle
- **[API Reference](/docs/api/rest-api-reference)** - Complete REST API documentation

## Support

- **📚 Documentation**: [raworc.com/docs](https://raworc.com/docs)
- **🌐 Website**: [raworc.com](https://raworc.com)
- **🐦 Community**: Follow us on [X/Twitter](https://x.com/raworc)