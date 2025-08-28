---
sidebar_position: 2
title: Spaces and Sessions
---

# Spaces and Sessions

Raworc organizes agent work through **Spaces** and **Sessions** - two core data models that enable secure multi-tenancy, containerized execution, and session lifecycle management.

## Spaces: Multi-Tenant Organization

**Spaces** are isolated environments that organize agent projects by team, environment, or use case. Each space contains its own agents, secrets, and sessions with complete separation.

### Space Data Model

```typescript
interface Space {
  name: string;              // Unique space identifier
  description?: string;      // Human-readable description
  settings: object;          // JSON configuration
  active: boolean;           // Space status
  created_at: timestamp;     // Creation time
  updated_at: timestamp;     // Last modification
  created_by: string;        // Creator service account
}
```

### Space Components

#### **Agents**
- Git-based deployment from any repository
- Framework-agnostic: LangChain, CrewAI, AutoGen, custom code
- Pre-compiled during space builds for instant session startup
- Configured via `raworc.json` manifest

#### **Secrets**
- Encrypted storage of API keys and credentials
- Space-scoped access control
- Required for agent authentication (e.g., `ANTHROPIC_API_KEY`)
- Granular permissions for viewing secret values

#### **Builds**
- Immutable space images containing pre-built agents
- UUID-tagged for version control: `raworc_space_{name}:{build-id}`
- Triggered when agents are added or modified
- Build status tracking and error reporting

### Space Lifecycle

```
Create Space → Add Secrets → Add Agents → Build Space → Create Sessions
```

1. **Create Space**: Initialize isolated environment
2. **Add Secrets**: Store encrypted credentials for agent authentication
3. **Add Agents**: Deploy agents from GitHub repositories
4. **Build Space**: Compile agents into immutable container image
5. **Create Sessions**: Launch containerized sessions using built image

## Sessions: Containerized Execution

**Sessions** are individual containerized environments where AI agents execute tasks. Each session runs in isolation with persistent storage and state management.

### Session Data Model

```typescript
interface Session {
  id: string;                    // UUID identifier
  space: string;                 // Parent space name
  created_by: string;            // Creator service account
  state: SessionState;           // Current lifecycle state
  container_id?: string;         // Docker container ID
  persistent_volume_id: string;  // Data volume ID
  parent_session_id?: string;    // For session forking
  created_at: timestamp;         // Session creation
  started_at?: timestamp;        // Container started
  last_activity_at?: timestamp;  // Last message/activity
  terminated_at?: timestamp;     // Session termination
  termination_reason?: string;   // Why session ended
  metadata: object;              // JSON session metadata
}
```

### Session State Machine

Sessions follow a validated state machine with controlled transitions:

```
init → idle → busy → closed → errored
  ↓      ↓      ↓       ↓         ↓
  ✓      ✓      ✓       ✓         ✓
  └─── delete (soft delete with cleanup)
```

#### State Definitions

- **`init`** - Container being created and initialized
- **`idle`** - Ready to receive and process messages
- **`busy`** - Processing messages and executing tasks
- **`closed`** - Container stopped, volume preserved (can be restored)
- **`errored`** - Container failed, requires intervention
- **`error`** - Error state requiring manual intervention

#### State Transitions

| From | To | Trigger | Result |
|------|----|---------|---------| 
| `init` | `idle` | Container ready | Agent polling starts |
| `idle` | `busy` | Message received | Agent processing |
| `busy` | `idle` | Task completed | Ready for next message |
| `idle` | `closed` | Manual close | Container stopped |
| `closed` | `idle` | Restore request | Container restarted |
| `idle` | `errored` | Error condition | Container marked failed |
| `errored` | `idle` | Manual recovery | Container recreated |
| `*` | `error` | System error | Manual intervention needed |

## Session Architecture

### Container Structure

Each session runs in an isolated Docker container with:

```
raworc_session_{session-id}/
├── /session/agents/          # Pre-built agents from space
│   ├── langchain-rag/       # LangChain agent with venv
│   ├── crewai-team/         # CrewAI multi-agent setup
│   └── custom-rust/         # Compiled Rust binary
├── /session/workspace/       # User working directory
├── /session/state/          # Session metadata and context
└── /session/logs/           # Agent execution logs
```

### Persistent Storage

Sessions use persistent Docker volumes for data that survives container lifecycle:

- **Volume Name**: `raworc_session_data_{session-id}`
- **Mount Point**: `/session/` (workspace, state, logs)
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

## Session Lifecycle Operations

### Create Session
```bash
raworc api sessions -m post -b '{"space":"production"}'
```

**Flow:**
1. Validate space exists and user has permissions
2. Generate UUID and create session record with `init` state
3. Operator detects new session and spawns container
4. Container mounts persistent volume and starts host agent
5. Session transitions to `idle` state when ready

### Session Messaging
```bash
raworc api sessions/{session-id}/messages -m post -b '{"content":"Hello"}'
```

**Flow:**
1. Message stored in database with `user` role
2. Session state transitions to `busy`
3. Host agent polls and receives message
4. Agent executes using AI capabilities and computer-use tools
5. Response stored with `agent` role
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
1. New container created from latest space image
2. Persistent volume remounted with preserved state
3. Host agent initializes and resumes message polling
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

## Session Forking and Data Lineage

Sessions support creating child sessions from parent sessions:

```bash
raworc api sessions -m post -b '{
  "space": "default",
  "parent_session_id": "parent-uuid"
}'
```

**Benefits:**
- **Experimentation**: Try different approaches without losing original work
- **Branching**: Create parallel workflows from common starting point
- **Data Lineage**: Track relationships between related sessions
- **Collaboration**: Share session state between team members

### Lineage Tracking

```typescript
interface SessionLineage {
  session_id: string;
  parent_session_id?: string;
  children: string[];         // Child session IDs
  depth: number;              // How many levels from root
  created_from: "new" | "fork";
}
```

## Space Build Process

Before sessions can be created, spaces must be built to compile agents into immutable container images.

### Build Trigger

Builds are triggered when:
- Agents are added to a space
- Space build is manually requested
- Agent repositories are updated (future)

### Build Process

```bash
raworc api spaces/{space}/build -m post
```

**Flow:**
1. Build task created with `pending` status
2. Operator creates temporary build container
3. Agents cloned from GitHub repositories
4. Dependencies compiled (pip install, npm install, cargo build)
5. Immutable image tagged: `raworc_space_{space}:{build-id}`
6. Build status updated to `completed` or `failed`
7. New sessions use latest successful build image

### Build Data Model

```typescript
interface SpaceBuild {
  id: string;                    // Build UUID
  space: string;                 // Target space
  status: BuildStatus;           // pending|building|completed|failed
  image_tag?: string;            // Docker image tag
  build_id: string;              // Unique build identifier
  started_at: timestamp;         // Build start time
  completed_at?: timestamp;      // Build completion
  agents_deployed?: object;      // Successfully built agents
  error?: string;                // Build error message
}
```

## Performance and Optimization

### Zero-Cold-Start Architecture

Sessions start instantly because:
- **Pre-Compilation**: Agents built during space creation, not runtime
- **Immutable Images**: `raworc_space_{name}:{build-id}` ready for deployment
- **Container Recreation**: Close/restore with quick startup
- **Persistent Volumes**: Data survives container lifecycle

### Resource Efficiency

- **Close Unused Sessions**: Automatic container stopping saves resources
- **Persistent Volumes**: Data preservation without container overhead
- **Shared Base Images**: Common dependencies cached across sessions
- **Connection Pooling**: Efficient database connections from API server

### Scaling Strategies

- **Horizontal Sessions**: Multiple concurrent sessions per space
- **Space Isolation**: Complete separation between teams/projects
- **Resource Limits**: Prevent runaway sessions from consuming resources
- **Cleanup Automation**: Old sessions automatically cleaned up

## Security Model

### Space Isolation
- **Secrets**: Encrypted per-space with access control
- **RBAC**: Role-based permissions scoped to spaces
- **Network**: Container networking with controlled access
- **Data**: Complete separation between spaces

### Session Security
- **Container Isolation**: Each session runs in secure boundaries
- **Resource Limits**: CPU, memory, and storage constraints
- **Volume Encryption**: Persistent data encrypted at rest
- **Audit Trails**: All operations tracked with attribution

### Access Control
- **JWT Authentication**: Secure token-based authentication
- **Space Permissions**: Fine-grained access control per space
- **Session Ownership**: Sessions tied to creator service account
- **Secret Permissions**: Separate permissions for viewing secret values

## Next Steps

- [Architecture Overview](/docs/concepts/architecture) - Complete system architecture
- [CLI Usage](/docs/guides/cli-usage) - Complete CLI usage guide
- [API Reference](/docs/api/overview) - CLI and API for spaces and sessions
- [Troubleshooting](/docs/guides/troubleshooting) - Common issues and solutions