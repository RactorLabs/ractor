---
sidebar_position: 3
title: Agents
---

# Agents

Raworc organizes Agent work through **Agents** - isolated containerized environments where the Agent executes tasks. Each agent provides secure execution, persistent storage, and comprehensive lifecycle management.

## Agent Data Model

```typescript
interface Agent {
  id: string;                    // UUID identifier
  created_by: string;            // Creator operator
  name?: string;                 // Optional unique agent name
  state: AgentState;           // Current lifecycle state
  container_id?: string;         // Docker container ID
  persistent_volume_id: string;  // Data volume ID
  parent_agent_name?: string;  // For agent remixing
  created_at: timestamp;         // Agent creation
  started_at?: timestamp;        // Container started
  last_activity_at?: timestamp;  // Last message/activity
  terminated_at?: timestamp;     // Agent termination
  termination_reason?: string;   // Why agent ended
  metadata: object;              // JSON agent metadata
  is_published: boolean;         // Public access enabled
  published_at?: timestamp;      // When agent was published
  published_by?: string;         // Who published the agent
  publish_permissions: object;   // Remix permissions (data/code/secrets)
  timeout_seconds: number;       // Agent timeout in seconds
  auto_sleep_at?: timestamp;     // When agent will auto-sleep
}
```

## Agent State Machine

Agents follow a validated state machine with controlled transitions:

```
init → idle → busy → slept
  ↓      ↓      ↓      ↓
  ✓      ✓      ✓      ✓
  └─── deleted (soft delete with cleanup)
```

### State Definitions

- **`init`** - Container being created and Agent initialized
- **`idle`** - Ready to receive and process messages
- **`busy`** - Processing messages and executing tasks
- **`slept`** - Container stopped, volume preserved (can be woken)
- **`deleted`** - Agent marked for deletion, resources cleaned up

### State Transitions

| From | To | Trigger | Result |
|------|----|---------|---------| 
| `init` | `idle` | Container ready | Agent polling starts |
| `idle` | `busy` | Message received | Agent processing |
| `busy` | `idle` | Task completed | Ready for next message |
| `idle` | `slept` | Manual sleep | Container stopped |
| `slept` | `idle` | Wake request | Container restarted |

## Agent Architecture

### Container Structure

Each agent runs in an isolated Docker container with:

```
raworc_agent_{agent-name}/
├── /agent/code/            # User code, data, and project files
│   ├── instructions.md      # Agent instructions
│   └── setup.sh            # Environment setup script
├── /agent/secrets/        # Environment secrets as files
├── /agent/content/        # HTML files and web assets
└── /agent/logs/           # Agent execution logs
```

### Persistent Storage

Agents use persistent Docker volumes for data that survives container lifecycle:

- **Volume Name**: `raworc_agent_data_{agent-name}`
- **Mount Point**: `/agent/` (code, secrets, content, logs)
- **Persistence**: Survives sleep/wake operations
- **Cleanup**: Removed only when agent is deleted

### Resource Management

Each agent container has configurable limits:

```yaml
resources:
  cpu_limit: "0.5"           # 50% of CPU core
  memory_limit: 536870912    # 512MB RAM
  disk_limit: 1073741824     # 1GB storage
  network: raworc_network    # Isolated network
```

## Agent Creation with Configuration

Create agents with optional secrets, instructions, and setup scripts:

```bash
# Basic agent (requires ANTHROPIC_API_KEY)
raworc api agents -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  }
}'

# Agent with name and timeout
raworc api agents -m post -b '{
  "name": "my-analysis-agent",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  },
  "timeout_seconds": 300
}'

# Agent with full configuration
raworc api agents -m post -b '{
  "name": "data-project",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Agent specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy",
  "timeout_seconds": 600
}'
```

### Configuration Options

- **`name`** - Optional unique name for the agent (can be used instead of ID in all operations)
- **`secrets`** - Environment variables/secrets for the agent
- **`instructions`** - Instructions for the Agent (written to `/agent/code/instructions.md`)
- **`setup`** - Setup script to run in the container (written to `/agent/code/setup.sh`)
- **`metadata`** - Additional metadata object
- **`timeout_seconds`** - Agent timeout in seconds (default: 300, triggers auto-sleep when idle)

## Agent Lifecycle Operations

### Create Agent
```bash
# ANTHROPIC_API_KEY is required for all new agents
raworc api agents -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  }
}'
```

**Flow:**
1. Validate ANTHROPIC_API_KEY is provided in secrets
2. Generate UUID and create agent record with `init` state
3. Operator detects new agent and spawns container
4. Container mounts persistent volume and starts Agent
5. Setup script executed if provided
6. Agent transitions to `idle` state when ready

### Agent Messaging
```bash
raworc api agents/{agent-name}/messages -m post -b '{"content":"Hello"}'
```

**Flow:**
1. Message stored in database with `user` role
2. Agent state transitions to `busy`
3. Agent polls and receives message
4. Agent executes using AI capabilities and computer-use tools
5. Response stored with `assistant` role
6. Agent returns to `idle` state

### Sleep Agent
```bash
raworc api agents/{agent-name}/sleep -m post
```

**Flow:**
1. Agent state transitions to `sleeping`
2. Container stopped and removed to free resources
3. Persistent volume preserved with all agent data
4. Agent can be woken later with full state

### Wake Agent
```bash
raworc api agents/{agent-name}/wake -m post
```

**Flow:**
1. New container created from host image
2. Persistent volume remounted with preserved state
3. Agent initializes and resumes message polling
4. **No reprocessing** - Only new messages after wake are handled
5. Agent returns to `idle` state (~3-5 seconds)

### Delete Agent
```bash
raworc api agents/{agent-name} -m delete
```

**Flow:**
1. Container stopped and removed
2. Persistent volume destroyed
3. Agent marked as soft-deleted in database
4. All agent data and logs permanently removed

## Agent Sleep/Wake

Raworc supports **reliable agent persistence** - sleep agents to save resources and wake them later with full state preservation:

```bash
# Sleep agent (preserves state)
raworc api agents/{agent-name}/sleep -m post

# Wake agent later  
raworc api agents/{agent-name}/wake -m post

# Continue with new messages
raworc agent wake {agent-name}
```

**Key Features:**
- ✅ **No message reprocessing** - Woken agents only handle new messages
- ✅ **Persistent storage** - All files and state preserved between restarts
- ✅ **Reliable message loop** - Second and subsequent messages process correctly
- ✅ **Fast wake up** - Agents resume quickly with minimal overhead

## Agent Remix

Create new agents based on existing ones with selective content copying:

```bash
# CLI usage with selective copying
raworc agent remix {source-agent-name}
raworc agent remix {source-agent-name} --data false
raworc agent remix {source-agent-name} --code false
raworc agent remix {source-agent-name} --secrets false
raworc agent remix {source-agent-name} --name "new-version" --secrets false --data true --code false

# API usage
raworc api agents/{source-agent-name}/remix -m post -b '{
  "name": "experiment-1",
  "data": true,
  "code": false,
  "secrets": true
}'
```

### Selective Copying Options

- **`name`** (optional) - Name for the new agent
- **`data`** (default: true) - Copy data files from parent agent
- **`code`** (default: true) - Copy code files from parent agent
- **`secrets`** (default: true) - Copy secrets from parent agent

**Use Cases:**
- 🔄 **Experiment branching** - Try different approaches from the same starting point
- 📋 **Template agents** - Create base agents and remix them for new projects
- 🧪 **A/B testing** - Compare different configurations from same baseline
- 🎯 **Checkpoint workflows** - Save progress and create multiple paths forward

### Remix Data Lineage

Agents support creating child agents from parent agents for tracking relationships:

```typescript
interface AgentLineage {
  agent_name: string;
  parent_agent_name?: string;
  children: string[];         // Child agent names
  depth: number;              // How many levels from root
  created_from: "new" | "remix";
  remix_options?: {           // What was copied in remix
    data: boolean;
    code: boolean;
    secrets: boolean;
  };
}
```

## Agent Publishing System

Share agents publicly with configurable remix permissions:

### Publishing Agents

```bash
# CLI - Publish agent with all permissions
raworc agent publish my-agent

# CLI - Publish with selective permissions
raworc agent publish my-agent \
  --data true \
  --code true \
  --secrets false

# API - Publish agent
raworc api agents/my-agent/publish -m post -b '{
  "data": true,
  "code": true,
  "secrets": false
}'

# Unpublish agent
raworc agent unpublish my-agent
raworc api agents/my-agent/unpublish -m post
```

### Accessing Published Agents

```bash
# List all published agents (no auth required)
raworc api published/agents

# Get published agent details (no auth required)
raworc api published/agents/agent-name

# Remix published agent
raworc agent remix published-agent-name --name "my-version"
```

### Publishing Features

- **Public Access**: Published agents can be viewed without authentication
- **Granular Permissions**: Control what can be remixed (data/code/secrets)
- **Cross-User Remixing**: Anyone can remix published agents
- **Name Resolution**: Published agents findable by name globally

## Agent Naming & Resolution

Agents can be named for easier identification and access:

### Agent Names

```bash
# Create agent with name
raworc agent --name "my-analysis" 
# Use name in all operations
raworc agent restore my-analysis
raworc agent remix my-analysis --name "experiment-1"
raworc api agents/my-analysis
```

### Name Resolution Rules

1. **Unique Constraint**: Agent names must be globally unique
2. **ID Fallback**: If name not found, system tries to resolve as UUID
3. **Owner Priority**: For owned agents, search by name first
4. **Published Search**: If not owned, search published agents by name
5. **Admin Access**: Admin users can access any agent by ID or name

## Agent Timeouts & Auto-Close

Automatic resource management through configurable timeouts:

### Timeout Configuration

```bash
# Set timeout during creation (uses ANTHROPIC_API_KEY from environment)
raworc agent --timeout 300
# API with timeout
raworc api agents -m post -b '{
  "timeout_seconds": 1800,
  "secrets": {"DATABASE_URL": "mysql://user:pass@host/db"}
}'
```

### Timeout States

- **Busy State**: Agent processing messages, timeout suspended
- **Idle State**: Agent waiting for messages, timeout counting down
- **Auto-Sleep**: Agent automatically sleeps when timeout reached

### Manual State Control

```bash
# Mark agent as busy (prevents timeout)
raworc api agents/my-agent/busy -m post

# Mark agent as idle (enables timeout)  
raworc api agents/my-agent/idle -m post
```

### Timeout Features

- **Default Timeout**: 60 seconds unless specified
- **Idle-Based**: Only counts down when agent is idle
- **Resource Saving**: Automatically sleeps unused agents
- **Wake Ready**: Auto-sleeping agents can be woken instantly

## Auto-Wake for Sleeping Agents

Seamless agent resumption when sending messages to sleeping agents:

### Auto-Wake Behavior

```bash
# Send message to sleeping agent - automatically wakes
raworc api agents/sleeping-agent/messages -m post -b '{"content":"Hello"}'
# Returns 200 OK immediately, queues wake task
```

### Auto-Wake Flow

1. **Message Detection**: API detects message sent to sleeping agent
2. **Immediate Response**: Returns 200 OK without delay
3. **Background Wake**: Queues agent wake task
4. **Message Processing**: Agent processes message after waking
5. **Transparent Experience**: User sees no difference from active agent

### Key Benefits

- **Zero Downtime**: No user-facing errors for sleeping agents
- **Resource Efficiency**: Agents auto-sleep when idle, wake on demand
- **Seamless UX**: Users don't need to manually wake agents
- **Background Processing**: Wake happens asynchronously

## Performance and Optimization

### Direct Agent Architecture

Agents start quickly because:
- **No Build Pipeline**: Direct host image usage without pre-compilation steps
- **Environment Variables**: Secrets passed directly to container
- **Container Recreation**: Sleep/wake with quick startup
- **Persistent Volumes**: Data survives container lifecycle

### Resource Efficiency

- **Sleep Unused Agents**: Automatic container stopping saves resources
- **Persistent Volumes**: Data preservation without container overhead
- **Shared Base Images**: Common dependencies cached across agents
- **Connection Pooling**: Efficient database connections from API server

### Scaling Strategies

- **Horizontal Agents**: Multiple concurrent agents per user
- **Resource Limits**: Prevent runaway agents from consuming resources
- **Cleanup Automation**: Old agents automatically cleaned up
- **Selective Remixing**: Copy only needed data for new agents

## Security Model

### Agent Security
- **Container Isolation**: Each agent runs in secure boundaries
- **Resource Limits**: CPU, memory, and storage constraints
- **Volume Encryption**: Persistent data encrypted at rest
- **Audit Trails**: All operations tracked with attribution

### Access Control
- **JWT Authentication**: Secure token-based authentication
- **Agent Ownership**: Agents tied to creator operator
- **RBAC Permissions**: Role-based access control for agent operations

### Secret Management
- **Environment Variables**: Secrets injected as environment variables
- **File-based Secrets**: Secrets written to `/agent/secrets/` directory

## Interactive Agent Interface

Use the interactive agent interface for real-time Agent interaction:

```bash
# All new agents require ANTHROPIC_API_KEY environment variable
raworc agent                                                    # Start new agent
raworc agent wake abc123                                        # Continue existing agent
raworc agent remix def456                                             # Create remix

# In agent interface:
You: Hello, how can you help me?
Agent: I can help you with coding, analysis, and more!

You: /quit
```

**Commands:**
- `/quit` or `/exit` - End agent
- `/status` - Show agent information

## Next Steps

- [CLI Usage](/docs/guides/cli-usage) - Complete CLI usage guide
- [API Reference](/docs/api/rest-api-reference) - Complete REST API documentation
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions