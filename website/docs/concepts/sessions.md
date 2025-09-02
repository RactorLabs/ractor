---
sidebar_position: 3
title: Sessions
---

# Sessions

Raworc organizes Host work through **Sessions** - isolated containerized environments where the Host executes tasks. Each session provides secure execution, persistent storage, and comprehensive lifecycle management.

## Session Data Model

```typescript
interface Session {
  id: string;                    // UUID identifier
  created_by: string;            // Creator operator
  name?: string;                 // Optional unique session name
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
  is_published: boolean;         // Public access enabled
  published_at?: timestamp;      // When session was published
  published_by?: string;         // Who published the session
  publish_permissions: object;   // Remix permissions (data/code/secrets)
  timeout_seconds: number;       // Session timeout in seconds
  auto_close_at?: timestamp;     // When session will auto-close
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

- **`init`** - Container being created and Host initialized
- **`idle`** - Ready to receive and process messages
- **`busy`** - Processing messages and executing tasks
- **`closed`** - Container stopped, volume preserved (can be restored)
- **`errored`** - Container failed, requires intervention

### State Transitions

| From | To | Trigger | Result |
|------|----|---------|---------| 
| `init` | `idle` | Container ready | Host polling starts |
| `idle` | `busy` | Message received | Host processing |
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
│   ├── instructions.md      # Host instructions
│   └── setup.sh            # Environment setup script
├── /session/data/           # Persistent user data
├── /session/secrets/        # Environment secrets as files
└── /session/logs/           # Host execution logs
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

# Session with name and timeout
raworc api sessions -m post -b '{
  "name": "my-analysis-session",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key"
  },
  "timeout_seconds": 300
}'

# Session with full configuration
raworc api sessions -m post -b '{
  "name": "data-project",
  "secrets": {
    "ANTHROPIC_API_KEY": "sk-ant-your-key",
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "instructions": "You are a helpful Host specialized in data analysis.",
  "setup": "#!/bin/bash\necho \"Setting up environment\"\npip install pandas numpy",
  "timeout_seconds": 600
}'
```

### Configuration Options

- **`name`** - Optional unique name for the session (can be used instead of ID in all operations)
- **`secrets`** - Environment variables/secrets for the session
- **`instructions`** - Instructions for the Host (written to `/session/code/instructions.md`)
- **`setup`** - Setup script to run in the container (written to `/session/code/setup.sh`)
- **`metadata`** - Additional metadata object
- **`timeout_seconds`** - Session timeout in seconds (default: 60, triggers auto-close when idle)

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
4. Host executes using AI capabilities and computer-use tools
5. Response stored with `host` role
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
raworc session restore {session-id}
```

**Key Features:**
- ✅ **No message reprocessing** - Restored sessions only handle new messages
- ✅ **Persistent storage** - All files and state preserved between restarts
- ✅ **Reliable message loop** - Second and subsequent messages process correctly
- ✅ **Fast restoration** - Host sessions resume quickly with minimal overhead

## Session Remix

Create new sessions based on existing ones with selective content copying:

```bash
# CLI usage with selective copying
raworc session remix {source-session-id}
raworc session remix {source-session-id} --data false
raworc session remix {source-session-id} --code false
raworc session remix {source-session-id} --secrets false
raworc session remix {source-session-id} --name "new-version" --secrets false --data true --code false

# API usage
raworc api sessions/{source-session-id}/remix -m post -b '{
  "name": "experiment-1",
  "data": true,
  "code": false,
  "secrets": true
}'
```

### Selective Copying Options

- **`name`** (optional) - Name for the new session
- **`data`** (default: true) - Copy data files from parent session
- **`code`** (default: true) - Copy code files from parent session
- **`secrets`** (default: true) - Copy secrets from parent session

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
    secrets: boolean;
  };
}
```

## Session Publishing System

Share sessions publicly with configurable remix permissions:

### Publishing Sessions

```bash
# CLI - Publish session with all permissions
raworc session publish my-session

# CLI - Publish with selective permissions
raworc session publish my-session \
  --data true \
  --code true \
  --secrets false

# API - Publish session
raworc api sessions/my-session/publish -m post -b '{
  "data": true,
  "code": true,
  "secrets": false
}'

# Unpublish session
raworc session unpublish my-session
raworc api sessions/my-session/unpublish -m post
```

### Accessing Published Sessions

```bash
# List all published sessions (no auth required)
raworc api published/sessions

# Get published session details (no auth required)
raworc api published/sessions/session-name

# Remix published session
raworc session remix published-session-name --name "my-version"
```

### Publishing Features

- **Public Access**: Published sessions can be viewed without authentication
- **Granular Permissions**: Control what can be remixed (data/code/secrets)
- **Cross-User Remixing**: Anyone can remix published sessions
- **Name Resolution**: Published sessions findable by name globally

## Session Naming & Resolution

Sessions can be named for easier identification and access:

### Session Names

```bash
# Create session with name
raworc session --name "my-analysis" 
# Use name in all operations
raworc session restore my-analysis
raworc session remix my-analysis --name "experiment-1"
raworc api sessions/my-analysis
```

### Name Resolution Rules

1. **Unique Constraint**: Session names must be globally unique
2. **ID Fallback**: If name not found, system tries to resolve as UUID
3. **Owner Priority**: For owned sessions, search by name first
4. **Published Search**: If not owned, search published sessions by name
5. **Admin Access**: Admin users can access any session by ID or name

## Session Timeouts & Auto-Close

Automatic resource management through configurable timeouts:

### Timeout Configuration

```bash
# Set timeout during creation (uses ANTHROPIC_API_KEY from environment)
raworc session --timeout 300
# API with timeout
raworc api sessions -m post -b '{
  "timeout_seconds": 1800,
  "secrets": {"DATABASE_URL": "mysql://user:pass@host/db"}
}'
```

### Timeout States

- **Busy State**: Session processing messages, timeout suspended
- **Idle State**: Session waiting for messages, timeout counting down
- **Auto-Close**: Session automatically closed when timeout reached

### Manual State Control

```bash
# Mark session as busy (prevents timeout)
raworc api sessions/my-session/busy -m post

# Mark session as idle (enables timeout)  
raworc api sessions/my-session/idle -m post
```

### Timeout Features

- **Default Timeout**: 60 seconds unless specified
- **Idle-Based**: Only counts down when session is idle
- **Resource Saving**: Automatically closes unused sessions
- **Restore Ready**: Auto-closed sessions can be restored instantly

## Auto-Restore for Closed Sessions

Seamless session resumption when sending messages to closed sessions:

### Auto-Restore Behavior

```bash
# Send message to closed session - automatically restores
raworc api sessions/closed-session/messages -m post -b '{"content":"Hello"}'
# Returns 200 OK immediately, queues restore task
```

### Auto-Restore Flow

1. **Message Detection**: API detects message sent to closed session
2. **Immediate Response**: Returns 200 OK without delay
3. **Background Restore**: Queues session restoration task
4. **Message Processing**: Session processes message after restoration
5. **Transparent Experience**: User sees no difference from active session

### Key Benefits

- **Zero Downtime**: No user-facing errors for closed sessions
- **Resource Efficiency**: Sessions auto-close when idle, restore on demand
- **Seamless UX**: Users don't need to manually restore sessions
- **Background Processing**: Restoration happens asynchronously

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
- **Session Ownership**: Sessions tied to creator operator
- **RBAC Permissions**: Role-based access control for session operations

### Secret Management
- **Environment Variables**: Secrets injected as environment variables
- **File-based Secrets**: Secrets written to `/session/secrets/` directory

## Interactive Session Interface

Use the interactive session interface for real-time Host interaction:

```bash
# All new sessions require ANTHROPIC_API_KEY environment variable
raworc session                                                    # Start new Host session
raworc session restore abc123                                           # Continue existing Host session
raworc session remix def456                                             # Create remix

# In session interface:
You: Hello, how can you help me?
Host: I can help you with coding, analysis, and more!

You: /quit
```

**Commands:**
- `/quit` or `/exit` - End session
- `/status` - Show session information

## Next Steps

- [CLI Usage](/docs/guides/cli-usage) - Complete CLI usage guide
- [API Reference](/docs/api/rest-api-reference) - Complete REST API documentation
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions