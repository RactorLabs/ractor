---
sidebar_position: 3
title: Agent Playground
---

# Agent Playground

Master the full power of Raworc agents with interactive examples and advanced features. This guide demonstrates agent management capabilities with Computer Use Agent through practical, hands-on examples.

## Prerequisites

Ensure an Ollama server is available (start an Ollama container or set `OLLAMA_HOST`).

```bash
# Optional if not using the built-in container
export OLLAMA_HOST=http://raworc_ollama:11434
```

## Interactive Agents

The simplest way to work with agents is through the interactive CLI:

```bash
# Start a new interactive agent (uses OLLAMA_HOST for model inference)
raworc agent create
```

In the interactive agent interface:
```bash
┌─────────────────────────────────────┐
│ ◊ Agent Start                       │
│ Agent: abc123-def456-789            │
│ User: admin (Operator)              │
│ Commands: /help (for commands)      │
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
```

**Interactive Commands:**
- `/help, /h` - Show all available commands
- `/status` - Display agent status with visual state indicators
- `/timeout <seconds>` - Change agent timeout (1-3600 seconds)
- `/name <name>` - Change agent name (alphanumeric and hyphens)
- `/detach, /d` - Detach from agent (keeps agent running)
- `/quit, /q` - End the agent completely

## Agent Sleep/Wake

Sleep/wake allows you to sleep agents to save resources, then wake them later with full state preservation:

### Basic Sleep/Wake Workflow

```bash
# Create an agent and work with it
raworc agent create
# Work with the agent...
# Note the agent ID (shown in status)

# Sleep the agent (saves resources)
raworc api agents/{agent-name}/sleep -m post

# Later, wake the agent
raworc api agents/{agent-name}/wake -m post

# Continue working with woken agent
raworc agent wake {agent-name}
```

### Sleep/Wake Features

**✅ State Preservation**: All files, data, and computer state are preserved
**✅ No Reprocessing**: Previous messages are not reprocessed - only new messages after wake
**✅ Fast Recovery**: Agents wake in 3-5 seconds with full context
**✅ Resource Efficiency**: Sleeping agents don't consume CPU or memory

### Practical Example: Long-Running Analysis

```bash
# Start a data analysis agent
raworc agent create

┌─────────────────────────────────────┐
│ ◊ Agent Start                       │
│ Agent: abc-123                      │
│ User: admin (Operator)              │
│ Commands: /help (for commands)      │
└─────────────────────────────────────┘

● ready
──────────────────────────────────────────────────
> Analyze the sales data in /data/sales_2024.csv

● Run
└─ Loading and analyzing sales_2024.csv

I'll analyze the sales data for you. Let me examine the file structure and perform initial analysis...

● ready
──────────────────────────────────────────────────
# Need to leave for a meeting? Use the detach command:
> /detach
👋 Detached from agent abc-123 (agent continues running)

# Sleep to save resources
raworc api agents/abc-123/sleep -m post

# After your meeting, wake and continue
raworc api agents/abc-123/wake -m post
raworc agent wake abc-123

● ready
──────────────────────────────────────────────────
> Continue with the regional breakdown analysis
```

## Agent Remix

Remix creates a new agent based on an existing one, allowing you to branch your workflow:

### Basic Remix Workflow

```bash
# Create a remix from existing agent
raworc agent remix {source-agent-name}

# The new agent starts with:
# - All files from the source agent
# - Complete computer state
# - Independent agent message history
```

### Remix Use Cases

#### 1. Experiment Branching
```bash
# Original agent: working on main algorithm
raworc agent create
> Implement quicksort algorithm in Python
# Agent ID: main-456

# Create branch to try different approach
raworc agent remix main-456
> Now implement the same using merge sort instead
# New agent with quicksort as starting point
```

#### 2. Template Agents
```bash
# Create a base agent with common setup
raworc agent
> Set up a Python project with pytest, black, and pre-commit hooks
# Agent ID: template-789

# Remix for each new project
raworc agent remix template-789 
> Add FastAPI with PostgreSQL setup
# Starts with all the base tools already configured

raworc agent remix template-789
> Add Django with MySQL setup  
# Another project from the same template
```

#### 3. A/B Testing Configurations
```bash
# Base agent with data loaded
raworc agent
> Load customer data from database
# Agent ID: base-321

# Test different ML models
raworc agent remix base-321
> Train a random forest classifier

raworc agent remix base-321
> Train a gradient boosting classifier

# Compare results from both agents
```

#### 4. Checkpoint Workflows
```bash
# Create checkpoints at key stages
raworc agent
> Complete phase 1 of the migration
# Agent ID: checkpoint-1

# Branch from checkpoint for different scenarios
raworc agent remix checkpoint-1
> Continue with the aggressive optimization approach

raworc agent remix checkpoint-1  
> Continue with the conservative safety-first approach
```

## Advanced Agent Management

### Listing and Filtering Agents

```bash
# List all agents
raworc api agents

# List active agents only
raworc api "agents?state=idle"
```

### Agent State Transitions

Agents follow a controlled state machine:

```
init → idle → busy → slept
  ↓      ↓      ↓       ↓
  └─── deleted (soft delete with cleanup)
```

**Visual State Indicators:**
- `◯` (init) - Agent initializing
- `●` (idle) - Agent ready for messages  
- `◉` (busy) - Agent processing messages
- `◼` (slept) - Agent sleeping, can be woken
- `◼` (deleted) - Agent permanently deleted

Monitor state transitions:
```bash
# Check agent state
raworc api agents/{agent-name}

# Watch for state changes
watch -n 2 'raworc api agents/{agent-name} | grep state'
```

### Bulk Agent Operations

```bash
# Sleep all idle agents
for id in $(raworc api "agents?state=idle" | jq -r '.[].id'); do
  raworc api agents/$id/sleep -m post
done

# Delete old agents (older than 7 days)
raworc api agents | jq -r '.[] | select(.created_at < (now - 604800)) | .id' | \
while read id; do
  raworc api agents/$id -m delete
done
```

## Agent Messaging Patterns

### Synchronous Messaging
```bash
# Send message and wait for response
raworc api agents/{id}/messages -m post -b '{"content":"Generate unit tests"}'

# Poll for response
raworc api agents/{id}/messages
```

### Message History Management
```bash
# Get last 10 messages
raworc api "agents/{id}/messages?limit=10"

# Get message count
raworc api agents/{id}/messages/count

# Clear message history (careful!)
raworc api agents/{id}/messages -m delete
```

### Multi-Turn Conversations
```bash
# Interactive agent handles this automatically
raworc agent wake {id}

● ready
──────────────────────────────────────────────────
> First question about the data

I'll help you with that. Let me analyze the data and provide insights...

● ready  
──────────────────────────────────────────────────
> Follow-up question based on previous answer

Based on my previous analysis, I can elaborate further...

# Context is maintained throughout the conversation
```

## Agent Data Management

### Working with Agent Files

Agents have persistent storage that survives sleep/wake:

```bash
# In an interactive agent
● ready
──────────────────────────────────────────────────
> Create a file called config.yaml with database settings

● Edit
└─ Creating config.yaml

I'll create a config.yaml file with database settings for you...

● ready
──────────────────────────────────────────────────
> Now update the connection string in config.yaml

● Edit  
└─ Updating connection string in config.yaml

I've updated the connection string in your config.yaml file...

# File persists across sleep/wake cycles
```

### Accessing Agent Containers

For debugging or advanced operations:

```bash
# List agent containers
docker ps -a --filter "name=raworc_agent_"

# Access agent filesystem
docker exec raworc_agent_{id} ls -la /agent/code/

# View agent logs
docker exec raworc_agent_{id} ls /agent/logs/
docker exec raworc_agent_{id} cat /agent/logs/agent_*.log
```

## Best Practices

### Resource Management
- **Sleep inactive agents** to save CPU and memory
- **Use wake** instead of creating new agents when continuing work
- **Set resource limits** appropriate for your workload

### Workflow Organization
- **Create template agents** for common starting points with agent configurations
- **Leverage remix** for experimentation without losing original work
- **Name agents clearly** with metadata for easy identification
- **Use agent instructions** to specialize agents for different tasks

### Performance Tips
- **Sleep agents** when not actively using them to save resources
- **Use agent remix** instead of recreating similar agent setups
- **Monitor resource usage** to optimize computer allocation

## Troubleshooting Agents

### Agent Won't Start
```bash
# Check agent state and configuration
raworc api agents/{id}

# Check controller logs
docker logs raworc_controller --tail 50
```

### Agent Wake Fails
```bash
# Check agent state
raworc api agents/{id}

# Verify container status
docker ps -a | grep raworc_agent_{id}

# Force recovery if needed
raworc api agents/{id}/state -m put -b '{"state":"idle"}'
```

### Agent Not Responding
```bash
# Check agent state (should be "idle" to receive messages)
raworc api agents/{id} | grep state

# View recent container logs
docker logs raworc_agent_{id} --tail 100

# Restart agent if needed
raworc api agents/{id}/sleep -m post
raworc api agents/{id}/wake -m post
```

## Next Steps

- [CLI Usage Guide](/docs/guides/cli-usage) - Complete CLI command reference
- [Agents](/docs/concepts/agents) - Technical architecture details
- [API Reference](/docs/api/rest-api-reference) - Full REST API documentation
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions
