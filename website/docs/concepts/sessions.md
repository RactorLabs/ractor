---
sidebar_position: 2
title: Sessions
---

# Sessions

Raworc organizes AI agent work through **Sessions** - isolated containerized environments where AI agents execute tasks. Each session provides secure execution, persistent storage, and comprehensive lifecycle management.

## Session Data Model

```typescript
interface Session {
  id: string;                    // UUID identifier
  created_by: string;            // Creator service account
  state: SessionState;           // Current lifecycle state
  container_id?: string;         // Docker container ID
  persistent_volume_id: string;  // Data volume ID
  parent_session_id?: string;    // For session remixing
  created_at: timestamp;         // Session creation
  started_at?: timestamp;        // Container started
  last_activity_at?: timestamp;  // Last message/activity
  terminated_at?: timestamp;     // Session termination
  termination_reason?: string;   // Why session ended
  metadata: object;              // JSON session metadata
}
```

## Session State Machine

Sessions follow a validated state machine with controlled transitions:

```
init → idle → busy → closed → errored
  ↓      ↓      ↓       ↓         ↓
  ✓      ✓      ✓       ✓         ✓
  └─── delete (soft delete with cleanup)
```

### State Definitions

- **`init`** - Container being created and initialized
- **`idle`** - Ready to receive and process messages
- **`busy`** - Processing messages and executing tasks
- **`closed`** - Container stopped, volume preserved (can be restored)
- **`errored`** - Container failed, requires intervention

### State Transitions

| From | To | Trigger | Result |
|------|----|---------|---------| 
| `init` | `idle` | Container ready | Agent polling starts |
| `idle` | `busy` | Message received | Agent processing |
| `busy` | `idle` | Task completed | Ready for next message |
| `idle` | `closed` | Manual close | Container stopped |
| `closed` | `idle` | Restore request | Container restarted |
| `idle` | `errored` | Error condition | Container marked failed |
| `errored` | `idle` | Manual recovery | Container recreated |

## Session Architecture

### Container Structure

Each session runs in an isolated Docker container with:

```
raworc_session_{session-id}/
├── /session/code/            # Instructions and setup scripts
│   ├── instructions.md      # AI agent instructions
│   └── setup.sh            # Environment setup script
├── /session/data/           # Persistent user data
├── /session/secrets/        # Environment secrets as files
└── /session/logs/           # Execution logs
```

### Persistent Storage

Sessions use persistent Docker volumes for data that survives container lifecycle:

- **Volume Name**: `raworc_session_data_{session-id}`
- **Mount Point**: `/session/` (code, data, secrets, logs)
- **Persistence**: Survives close/restore operations
- **Cleanup**: Removed only when session is deleted

### Resource Management

Each session container has configurable limits:

```yaml
resources:
  cpu_limit: "0.5"           # 50% of CPU core
  memory_limit: 536870912    # 512MB RAM
  disk_limit: 1073741824     # 1GB storage
  network: raworc_network    # Isolated network
```

## Session Creation with Configuration

Create sessions with optional secrets, instructions, and setup scripts:

```bash
# Basic session (requires ANTHROPIC_API_KEY)
raworc api sessions -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  }
}'

# Session with multiple secrets
raworc api sessions -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  }
}'

# Session with instructions
raworc api sessions -m post -b '{
  "instructions": "You are a helpful assistant specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy"
}'
```

### Configuration Options

- **`secrets`** - Environment variables/secrets for the session
- **`instructions`** - Instructions for the AI agent (written to `/session/code/instructions.md`)
- **`setup`** - Setup script to run in the container (written to `/session/code/setup.sh`)
- **`metadata`** - Additional metadata object

## Session Lifecycle Operations

### Create Session
```bash
# ANTHROPIC_API_KEY is required for all new sessions
raworc api sessions -m post -b '{
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  }
}'
```

**Flow:**
1. Validate ANTHROPIC_API_KEY is provided in secrets
2. Generate UUID and create session record with `init` state
3. Operator detects new session and spawns container
4. Container mounts persistent volume and starts Host
5. Setup script executed if provided
6. Session transitions to `idle` state when ready

### Session Messaging
```bash
raworc api sessions/{session-id}/messages -m post -b '{"content":"Hello"}'
```

**Flow:**
1. Message stored in database with `user` role
2. Session state transitions to `busy`
3. Host polls and receives message
4. Agent executes using AI capabilities and computer-use tools
5. Response stored with `assistant` role
6. Session returns to `idle` state

### Close Session
```bash
raworc api sessions/{session-id}/close -m post
```

**Flow:**
1. Session state transitions to `closed`
2. Container stopped and removed to free resources
3. Persistent volume preserved with all session data
4. Session can be restored later with full state

### Restore Session
```bash
raworc api sessions/{session-id}/restore -m post
```

**Flow:**
1. New container created from host image
2. Persistent volume remounted with preserved state
3. Host initializes and resumes message polling
4. **No reprocessing** - Only new messages after restore are handled
5. Session returns to `idle` state (~3-5 seconds)

### Delete Session
```bash
raworc api sessions/{session-id} -m delete
```

**Flow:**
1. Container stopped and removed
2. Persistent volume destroyed
3. Session marked as soft-deleted in database
4. All session data and logs permanently removed

## Session Restore

Raworc supports **reliable session persistence** - close sessions to save resources and restore them later with full state preservation:

```bash
# Close session (preserves state)
raworc api sessions/{session-id}/close -m post

# Restore session later  
raworc api sessions/{session-id}/restore -m post

# Continue with new messages
raworc session --restore {session-id}
```

**Key Features:**
- ✅ **No message reprocessing** - Restored sessions only handle new messages
- ✅ **Persistent storage** - All files and state preserved between restarts
- ✅ **Reliable message loop** - Second and subsequent messages process correctly
- ✅ **Fast restoration** - Sessions resume quickly with minimal overhead

## Session Remix

Create new sessions based on existing ones with selective content copying:

```bash
# CLI usage with selective copying
raworc session --remix {source-session-id}
raworc session --remix {source-session-id} --data false
raworc session --remix {source-session-id} --code false
raworc session --remix {source-session-id} --code false --data false

# API usage
raworc api sessions/{source-session-id}/remix -m post -b '{
  "data": true,
  "code": false
}'
```

### Selective Copying Options

- **`data`** (default: true) - Copy data files from parent session
- **`code`** (default: true) - Copy code files from parent session

**Use Cases:**
- 🔄 **Experiment branching** - Try different approaches from the same starting point
- 📋 **Template sessions** - Create base sessions and remix them for new projects
- 🧪 **A/B testing** - Compare different configurations from same baseline
- 🎯 **Checkpoint workflows** - Save progress and create multiple paths forward

### Remix Data Lineage

Sessions support creating child sessions from parent sessions for tracking relationships:

```typescript
interface SessionLineage {
  session_id: string;
  parent_session_id?: string;
  children: string[];         // Child session IDs
  depth: number;              // How many levels from root
  created_from: "new" | "remix";
  remix_options?: {           // What was copied in remix
    data: boolean;
    code: boolean;
  };
}
```

## Performance and Optimization

### Direct Session Architecture

Sessions start quickly because:
- **No Build Pipeline**: Direct host image usage without pre-compilation steps
- **Environment Variables**: Secrets passed directly to container
- **Container Recreation**: Close/restore with quick startup
- **Persistent Volumes**: Data survives container lifecycle

### Resource Efficiency

- **Close Unused Sessions**: Automatic container stopping saves resources
- **Persistent Volumes**: Data preservation without container overhead
- **Shared Base Images**: Common dependencies cached across sessions
- **Connection Pooling**: Efficient database connections from API server

### Scaling Strategies

- **Horizontal Sessions**: Multiple concurrent sessions per user
- **Resource Limits**: Prevent runaway sessions from consuming resources
- **Cleanup Automation**: Old sessions automatically cleaned up
- **Selective Remixing**: Copy only needed data for new sessions

## Security Model

### Session Security
- **Container Isolation**: Each session runs in secure boundaries
- **Resource Limits**: CPU, memory, and storage constraints
- **Volume Encryption**: Persistent data encrypted at rest
- **Audit Trails**: All operations tracked with attribution

### Access Control
- **JWT Authentication**: Secure token-based authentication
- **Session Ownership**: Sessions tied to creator service account
- **RBAC Permissions**: Role-based access control for session operations

### Secret Management
- **Environment Variables**: Secrets injected as environment variables
- **File-based Secrets**: Secrets written to `/session/secrets/` directory

## Interactive Session Interface

Use the interactive session interface for real-time AI interaction:

```bash
# All new sessions require ANTHROPIC_API_KEY
raworc session --secrets '{"ANTHROPIC_API_KEY":"sk-ant-your-key"}'    # Start new session
raworc session --restore abc123                                         # Continue existing session
raworc session --remix def456                                           # Create remix (inherits key if secrets=true)

# In session interface:
You: Hello, how can you help me?
Assistant: I can help you with coding, analysis, and more!

You: /quit
```

**Commands:**
- `/quit` or `/exit` - End session
- `/status` - Show session information

## Next Steps

- [CLI Usage](/docs/guides/cli-usage) - Complete CLI usage guide
- [API Reference](/docs/api/rest-api) - Complete REST API documentation
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions