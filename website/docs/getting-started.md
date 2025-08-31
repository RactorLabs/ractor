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
raworc auth login --user admin --pass admin
```

### 4. Start Your First Session (with API Key)

```bash
raworc session --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-actual-key"}'
```

**⚠️ Important**: You must provide an Anthropic API key to start a new session. Get your key from [console.anthropic.com](https://console.anthropic.com).

That's it! You now have a running Host session.

## Session Configuration

### Basic Session (Requires API Key)

```bash
# New sessions always require an Anthropic API key
raworc session --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-actual-key"}'
```

**Note**: The Anthropic API key is required for all new sessions. You cannot start a session without it unless you're remixing from an existing session that already has the key.

### Session with Instructions

```bash
raworc session --instructions ./my-instructions.md
```

### Session with Setup Script

```bash
raworc session --setup ./setup.sh
```

### Full Configuration

```bash
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key","DATABASE_URL":"mysql://user:pass@host/db"}' \
  --instructions "You are a helpful data analysis assistant." \
  --setup "#!/bin/bash\necho 'Setting up environment'\npip install pandas numpy"
```

## Interactive Session Usage

Once in a session, you can interact directly with the Host:

```
You: Hello, how can you help me?
⠋ Waiting for agent response...
Assistant: Hello! I'm an AI assistant that can help you with various tasks including:
- Writing and debugging code
- Data analysis and visualization  
- File management and organization
- Web research and information gathering
- And much more!

You: Create a Python script to calculate fibonacci numbers
⠋ Waiting for agent response...
Assistant: I'll create a Python script to calculate Fibonacci numbers for you.

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
raworc session --restore abc123-def456-789
```

### Create Session Remix

```bash
# Remix from existing session
raworc session --remix abc123-def456-789

# Selective remix options
raworc session --remix abc123-def456-789 --data false    # Don't copy data files
raworc session --remix abc123-def456-789 --code false    # Don't copy code files
```

## Direct API Usage

### Create Session via API

```bash
# Basic session
raworc api sessions -m POST -b '{}'

# Session with configuration
raworc api sessions -m POST -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful assistant specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy"
}'
```

### Send Messages to Session

```bash
raworc api sessions/{session-id}/messages -m POST -b '{
  "content": "Generate a Python script to calculate fibonacci numbers"
}'
```

### Session Lifecycle

```bash
# Close session (saves resources, preserves data)
raworc api sessions/{session-id}/close -m POST

# Restore closed session
raworc api sessions/{session-id}/restore -m POST

# Delete session permanently
raworc api sessions/{session-id} -m DELETE
```

## Troubleshooting

### Services won't start
```bash
raworc stop
raworc start --restart
```

### Check system health
```bash
raworc api health
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

## Common Use Cases

### Web Automation

```bash
# Create a web automation Host session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key"}' \
  --instructions "You automate web tasks. Use browsers to fill forms, extract data, and navigate websites."
```

### Document Processing

```bash
# Create a document processing Host session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key"}' \
  --instructions "You process documents and files. Generate reports, manipulate spreadsheets, and handle data workflows." \
  --setup "pip install pandas openpyxl python-docx"
```

### System Administration

```bash
# Create a system automation Host session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key"}' \
  --instructions "You automate system administration tasks. Manage servers, deploy applications, and monitor systems."
```

## Next Steps

Now that you have Raworc running:

- **[CLI Usage Guide](/docs/guides/cli-usage)** - Master all CLI commands and features
- **[Sessions Concepts](/docs/concepts/sessions)** - Understand session architecture and lifecycle
- **[API Reference](/docs/api/rest-api)** - Complete REST API documentation

## Support

- **📚 Documentation**: [raworc.com/docs](https://raworc.com/docs)
- **🌐 Website**: [raworc.com](https://raworc.com)
- **🐦 Community**: Follow us on [X/Twitter](https://x.com/raworc)