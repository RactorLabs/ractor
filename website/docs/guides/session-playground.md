---
sidebar_position: 3
title: Session Playground
---

# Session Playground

Master the full power of Raworc sessions with interactive examples and advanced features. This guide demonstrates session management capabilities with Host (Computer Use Agent) through practical, hands-on examples.

## Prerequisites

- **ANTHROPIC_API_KEY**: Required environment variable for all new sessions

```bash
export ANTHROPIC_API_KEY=sk-ant-your-actual-key
```

## Interactive Sessions

The simplest way to work with sessions is through the interactive CLI:

```bash
# Start a new interactive session with Host (uses ANTHROPIC_API_KEY from environment)
raworc session
```

In the interactive session interface:
- Type messages directly to control the Host and automate computer tasks
- Use `/status` to show session information
- Use `/quit` or `/q` to exit the session

## Session Restore

Restore allows you to close sessions to save resources, then bring them back later with full state preservation:

### Basic Restore Workflow

```bash
# Create a session and work with it
raworc session
# Work with the session...
# Note the session ID (shown in status)

# Close the session (saves resources)
raworc api sessions/{session-id}/close -m post

# Later, restore the session
raworc api sessions/{session-id}/restore -m post

# Continue working with restored session
raworc session restore {session-id}
```

### Restore Features

**✅ State Preservation**: All files, data, and computer state are preserved
**✅ No Reprocessing**: Previous messages are not reprocessed - only new messages after restore
**✅ Fast Recovery**: Sessions restore in 3-5 seconds with full Host context
**✅ Resource Efficiency**: Closed sessions don't consume CPU or memory

### Practical Example: Long-Running Analysis

```bash
# Start a data analysis session
raworc session
> Analyze the sales data in /data/sales_2024.csv

# Host begins processing...
# Need to leave for a meeting? Close the session:
/quit

# Close to save resources
raworc api sessions/abc-123/close -m post

# After your meeting, restore and continue
raworc api sessions/abc-123/restore -m post
raworc session restore abc-123
> Continue with the regional breakdown analysis
```

## Session Remix

Remix creates a new session based on an existing one, allowing you to branch your workflow:

### Basic Remix Workflow

```bash
# Create a remix from existing session
raworc session remix {source-session-id}

# The new session starts with:
# - All files from the source session
# - Complete computer state
# - Independent Host message history
```

### Remix Use Cases

#### 1. Experiment Branching
```bash
# Original session: working on main algorithm
raworc session
> Implement quicksort algorithm in Python
# Session ID: main-456

# Create branch to try different approach
raworc session remix main-456
> Now implement the same using merge sort instead
# New session with quicksort as starting point
```

#### 2. Template Sessions
```bash
# Create a base session with common setup
raworc session
> Set up a Python project with pytest, black, and pre-commit hooks
# Session ID: template-789

# Remix for each new project
raworc session remix template-789 
> Add FastAPI with PostgreSQL setup
# Starts with all the base tools already configured

raworc session remix template-789
> Add Django with MySQL setup  
# Another project from the same template
```

#### 3. A/B Testing Configurations
```bash
# Base session with data loaded
raworc session
> Load customer data from database
# Session ID: base-321

# Test different ML models
raworc session remix base-321
> Train a random forest classifier

raworc session remix base-321
> Train a gradient boosting classifier

# Compare results from both sessions
```

#### 4. Checkpoint Workflows
```bash
# Create checkpoints at key stages
raworc session
> Complete phase 1 of the migration
# Session ID: checkpoint-1

# Branch from checkpoint for different scenarios
raworc session remix checkpoint-1
> Continue with the aggressive optimization approach

raworc session remix checkpoint-1  
> Continue with the conservative safety-first approach
```

## Advanced Session Management

### Listing and Filtering Sessions

```bash
# List all sessions
raworc api sessions

# List active sessions only
raworc api "sessions?state=idle"
```

### Session State Transitions

Sessions follow a controlled state machine:

```
init → idle → busy → closed
  ↓      ↓      ↓       ↓
  └─── delete (removes session)
```

Monitor state transitions:
```bash
# Check session state
raworc api sessions/{session-id}

# Watch for state changes
watch -n 2 'raworc api sessions/{session-id} | grep state'
```

### Bulk Session Operations

```bash
# Close all idle sessions
for id in $(raworc api "sessions?state=idle" | jq -r '.[].id'); do
  raworc api sessions/$id/close -m post
done

# Delete old sessions (older than 7 days)
raworc api sessions | jq -r '.[] | select(.created_at < (now - 604800)) | .id' | \
while read id; do
  raworc api sessions/$id -m delete
done
```

## Session Messaging Patterns

### Synchronous Messaging
```bash
# Send message and wait for response
raworc api sessions/{id}/messages -m post -b '{"content":"Generate unit tests"}'

# Poll for response
raworc api sessions/{id}/messages
```

### Message History Management
```bash
# Get last 10 messages
raworc api "sessions/{id}/messages?limit=10"

# Get message count
raworc api sessions/{id}/messages/count

# Clear message history (careful!)
raworc api sessions/{id}/messages -m delete
```

### Multi-Turn Conversations
```bash
# Interactive session handles this automatically
raworc session restore {id}
> First question
# Wait for response...
> Follow-up question based on previous answer
# Context is maintained throughout
```

## Session Data Management

### Working with Session Files

Sessions have persistent storage that survives close/restore:

```bash
# In an interactive session
> Create a file called config.yaml with database settings
> Now update the connection string in config.yaml
# File persists across close/restore cycles
```

### Accessing Session Containers

For debugging or advanced operations:

```bash
# List session containers
docker ps -a --filter "name=raworc_session_"

# Access session filesystem
docker exec raworc_session_{id} ls -la /session/code/

# View Host logs
docker exec raworc_session_{id} ls /session/logs/
docker exec raworc_session_{id} cat /session/logs/host_*.log
```

## Best Practices

### Resource Management
- **Close inactive sessions** to save CPU and memory
- **Use restore** instead of creating new sessions when continuing work
- **Set resource limits** appropriate for your workload

### Workflow Organization
- **Create template sessions** for common starting points with Host configurations
- **Leverage remix** for experimentation without losing original work
- **Name sessions clearly** with metadata for easy identification
- **Use Host instructions** to specialize sessions for different tasks

### Performance Tips
- **Close sessions** when not actively using them to save resources
- **Use session remix** instead of recreating similar Host setups
- **Monitor resource usage** to optimize computer allocation

## Troubleshooting Sessions

### Session Won't Start
```bash
# Check session state and configuration
raworc api sessions/{id}

# Check operator logs
docker logs raworc_operator --tail 50
```

### Session Restore Fails
```bash
# Check session state
raworc api sessions/{id}

# Verify container status
docker ps -a | grep raworc_session_{id}

# Force recovery if needed
raworc api sessions/{id}/state -m put -b '{"state":"idle"}'
```

### Session Not Responding
```bash
# Check session state (should be "idle" to receive messages)
raworc api sessions/{id} | grep state

# View recent container logs
docker logs raworc_session_{id} --tail 100

# Restart session if needed
raworc api sessions/{id}/close -m post
raworc api sessions/{id}/restore -m post
```

## Next Steps

- [CLI Usage Guide](/docs/guides/cli-usage) - Complete CLI command reference
- [Sessions](/docs/concepts/sessions) - Technical architecture details
- [API Reference](/docs/api/rest-api) - Full REST API documentation
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions