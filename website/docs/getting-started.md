---
sidebar_position: 2
title: Getting Started
---

# Getting Started with Raworc

Get started with the Remote Agentic Work Orchestrator in just a few commands. Raworc provides dedicated remote computers with computer use agents to automate any manual work with enterprise-grade reliability.

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

### 4. Start Your First Agent

```bash
raworc agent
```

**Note**: Make sure you have set the `ANTHROPIC_API_KEY` environment variable as shown in the prerequisites.

That's it! You now have a running agent.

## Agent Configuration

### Basic Agent

```bash
# Create new agent (uses ANTHROPIC_API_KEY from environment)
raworc agent
```

**Note**: The Anthropic API key environment variable is required for all new agents.

### Agent with Instructions

```bash
raworc agent --instructions "You are a helpful coding assistant specialized in Python"
```

### Agent with Setup Script

```bash
raworc agent --setup "pip install pandas numpy matplotlib"
```

For more advanced agent configuration options, see the [CLI Usage Guide](/docs/guides/cli-usage#4-agent-configuration-options).

## Interactive Agent Usage

Once in an agent, you can interact directly with the agent using the clean CLI interface:

```bash
$ raworc agent

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
- **[Agent Concepts](/docs/concepts/agents)** - Understand agent architecture and lifecycle
- **[API Reference](/docs/api/rest-api-reference)** - Complete REST API documentation

## Support

- **📚 Documentation**: [raworc.com/docs](https://raworc.com/docs)
- **🌐 Website**: [raworc.com](https://raworc.com)
- **🐦 Community**: Follow us on [X/Twitter](https://x.com/raworc)