---
sidebar_position: 2
title: Getting Started
---

# Getting Started with Raworc

Get started with the Remote Agentic Work Orchestrator in just a few commands. Raworc provides dedicated remote computers with computer use agents to automate any manual work with enterprise-grade reliability.

## Prerequisites

- **Node.js 16+**: For the Raworc CLI
- **Docker**: Docker Engine 20.10+ and Docker Compose v2+
- **Ollama + Model**: Local model runtime with the `gpt-oss:20b` model available

Notes about Ollama and models:
- The Raworc Controller uses an Ollama server for AI inference.
- If you run Ollama yourself, set `OLLAMA_HOST` for the Controller to reach it.
- Ensure the `gpt-oss:20b` model is pulled on the Ollama server you use.

### Ollama Setup

You can run Ollama yourself (recommended for the CLI flow) or point to a remote Ollama server.

Option A — Run Ollama in Docker (attach to Raworc network, CPU-only):
```bash
# Create network Raworc uses (if not already created by Raworc CLI later)
docker network create raworc_network || true

# Start Ollama attached to the same network
docker run -d \
  --name raworc_ollama \
  --network raworc_network \
  -p 11434:11434 \
  -v raworc_ollama_data:/root/.ollama \
  ollama/ollama:latest

# Pull required model inside the container
docker exec raworc_ollama ollama pull gpt-oss:20b

# Tell Raworc Controller where Ollama is
export OLLAMA_HOST=http://raworc_ollama:11434
```

Option B — Use a remote/hosted Ollama:
```bash
# Point Raworc to your Ollama server
export OLLAMA_HOST=http://host.docker.internal:11434   # or your remote URL

# Ensure the model is available on that server
# (run on the Ollama server):
ollama pull gpt-oss:20b
```

## Quick Start (30 seconds)

### 1. Install Raworc CLI

```bash
npm install -g @raworc/cli
```

### 2. Start Services (API + Controller + DB)

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

### 4. Start Your First Agent

```bash
raworc agent create
```

Note: The CLI can also manage Ollama locally. By default, `raworc start` brings up MySQL, Ollama, the API server, and the controller. If you prefer a remote Ollama, set `--controller-ollama-host` or export `OLLAMA_HOST` and skip starting the `ollama` component explicitly: `raworc start mysql server controller`.

That's it! You now have a running agent.

## Agent Configuration

### Basic Agent

```bash
# Create new agent (uses OLLAMA_HOST for model inference)
raworc agent create
```

### Agent with Instructions

```bash
raworc agent create --instructions "You are a helpful coding assistant specialized in Python"
```

### Agent with Setup Script

```bash
raworc agent create --setup "pip install pandas numpy matplotlib"
```

For more advanced agent configuration options, see the [CLI Usage Guide](/docs/guides/cli-usage#4-agent-configuration-options).

## Interactive Agent Usage

Once in an agent, you can interact directly with the agent using the clean CLI interface:

```bash
$ raworc agent create

┌─────────────────────────────────────┐
│ ◊ Agent Start                       │
│ Agent: abc123-def456-789             │
│ User: admin (Operator)               │
│ Commands: /help (for commands)       │
└─────────────────────────────────────┘

◯ initializing...
──────────────────────────────────────────────────
> Hello, how can you help me?

I'm an agent that can help you with various tasks including:
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
👋 Ending agent...
```

### Agent Commands

- **Regular messages**: Just type your request
- **`/quit`** or **`/exit`** - End the agent
- **`/status`** - Show agent information

## Agent Management

### List Your Agents

```bash
raworc api agents
```

### Wake Previous Agent

```bash
raworc agent wake abc123-def456-789
```

### Create Agent Remix

```bash
# Remix from existing agent
raworc agent remix abc123-def456-789

# Selective remix options
raworc agent remix abc123-def456-789 --data false    # Don't copy data files
raworc agent remix abc123-def456-789 --code false    # Don't copy code files
```

## Direct API Usage

### Create Agent via API

```bash
# Basic agent
raworc api agents -m POST -b '{}'

# Agent with configuration
raworc api agents -m POST -b '{
  "secrets": {
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful agent specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy"
}'
```

### Send Messages to Agent

```bash
raworc api agents/{agent-name}/messages -m POST -b '{
  "content": "Generate a Python script to calculate fibonacci numbers"
}'
```

### Agent Lifecycle

```bash
# Sleep agent (saves resources, preserves data)
raworc api agents/{agent-name}/sleep -m POST

# Wake sleeping agent
raworc api agents/{agent-name}/wake -m POST

# Delete agent permanently
raworc api agents/{agent-name} -m DELETE
```

## Troubleshooting

### Services won't start
```bash
raworc stop
raworc start
```

### Check system status
```bash
raworc api version
```

### View service logs
```bash
docker logs raworc_server
docker logs raworc_controller
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
- **[Agent Concepts](/docs/concepts/agents)** - Understand agent architecture and lifecycle
- **[API Reference](/docs/api/rest-api-reference)** - Complete REST API documentation

## Support

- **📚 Documentation**: [raworc.com/docs](https://raworc.com/docs)
- **🌐 Website**: [raworc.com](https://raworc.com)
- **🐦 Community**: Follow us on [X/Twitter](https://x.com/raworc)
