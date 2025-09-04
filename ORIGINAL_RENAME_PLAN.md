# Original Comprehensive Session/Host → Agent Renaming Plan (v0.4.0)

## Summary
Complete system-wide rename from "Session/Host" to "Agent" with no backward compatibility. This will unify the conceptual model where an "Agent" represents both the orchestrated work unit and its runtime container. This change will be part of the unreleased v0.4.0.

## 🚨 **CRITICAL USER REQUIREMENTS** 🚨

### **Database Schema Approach**
- ❌ **DO NOT** create separate migration files
- ✅ **ONLY** update the existing complete schema: `db/migrations/20250902000001_complete_schema.sql`
- **Reason**: No backward compatibility needed, users start fresh with clean agent schema

### **Container Internal Paths** (User specifically emphasized this)
- **ALL** container internal paths must change: `/session/*` → `/agent/*`
- This includes: `/session/code/`, `/session/secrets/`, `/session/content/`, `/session/logs/`
- Update working directory and environment variables accordingly

### **Version Management**
- Keep version as 0.4.0 (unreleased)
- No version bump needed since 0.4.0 was never released

## Phase 1: Database Schema Update

### 1.1 **CRITICAL: Update existing complete schema file ONLY**
- **DO NOT create new migration file**
- **Update existing file**: `db/migrations/20250902000001_complete_schema.sql`
- **Reason**: No backward compatibility needed, users start fresh with agent schema

### 1.2 Table renames in complete schema
- `sessions` → `agents`
- `session_messages` → `agent_messages` 
- `session_tasks` → `agent_tasks`

### 1.3 Column renames in complete schema
- `parent_session_name` → `parent_agent_name`
- `session_name` columns → `agent_name` columns
- Keep `name` as primary key (no change needed)

### 1.4 Update foreign keys and constraints in complete schema
- All FK references from `session_name` → `agent_name`
- All constraint names: `sessions_*` → `agents_*`
- All index names: `idx_session_*` → `idx_agent_*`
- Update role constraint: `'host'` → `'agent'`

## Phase 2: Rust Backend Core Changes

### 2.1 Model files
- Rename `src/shared/models/session.rs` → `agent.rs`
- Rename struct `Session` → `Agent`
- Rename `CreateSessionRequest` → `CreateAgentRequest`
- Rename `RemixSessionRequest` → `RemixAgentRequest`
- Update all field names: `session_name` → `agent_name`, `parent_session_name` → `parent_agent_name`

### 2.2 Handler files
- Rename `src/server/rest/handlers/sessions.rs` → `agents.rs`
- Update all function names from `session_*` → `agent_*`

### 2.3 Operator component
- Rename `src/operator/session_manager.rs` → `agent_manager.rs`
- Update `SessionManager` → `AgentManager`

### 2.4 Host → Agent binary rename
- Rename directory `src/host/` → `src/agent/`
- Update `Cargo.toml`: `raworc-host` → `raworc-agent`
- Update all references to "host" in agent code

### 2.5 Constants
- Update `src/shared/models/constants.rs`: `SESSION_*` → `AGENT_*`
- Update state constants: `AGENT_STATE_INIT`, `AGENT_STATE_IDLE`, etc.
- Update message roles: `MESSAGE_ROLE_AGENT` (instead of MESSAGE_ROLE_HOST)

## Phase 3: Docker & Infrastructure

### 3.1 Dockerfiles
- Rename `Dockerfile.host` → `Dockerfile.agent`
- Update internal references from "host" user to "agent" user
- Update user creation: `useradd -m -s /bin/bash agent` (instead of host)

### 3.2 **CRITICAL: Container internal paths** (User specifically requested this)
- `/session` → `/agent` (working directory)
- `/session/code/` → `/agent/code/` 
- `/session/secrets/` → `/agent/secrets/`
- `/session/content/` → `/agent/content/`
- `/session/logs/` → `/agent/logs/`
- Update environment variable: `RAWORC_SESSION_DIR=/session` → `RAWORC_AGENT_DIR=/agent`
- **This affects ALL container internal filesystem paths**

### 3.3 Container naming
- Update container names: `raworc_session_*` → `raworc_agent_*`
- Update volume names: `raworc_session_data_*` → `raworc_agent_data_*`

### 3.4 Environment variables
- `RAWORC_SESSION_NAME` → `RAWORC_AGENT_NAME`
- `RAWORC_SESSION_DIR` → `RAWORC_AGENT_DIR`
- `HOST_*` → `AGENT_*` variables

### 3.5 Build scripts
- Update `scripts/build.sh`: host → agent references
- Update Docker image names: `raworc_host` → `raworc_agent`

## Phase 4: Path Updates in Code

### 4.1 Docker manager (src/operator/docker_manager.rs)
- `/session/secrets/` → `/agent/secrets/`
- `/session/code/instructions.md` → `/agent/code/instructions.md`
- `/session/code/setup.sh` → `/agent/code/setup.sh`
- `/session/content/` → `/agent/content/`

### 4.2 Agent runtime (src/agent/mod.rs - formerly src/host/mod.rs)
- `/session/code` → `/agent/code`
- `/session/secrets` → `/agent/secrets`
- `/session/content` → `/agent/content`
- `/session/code/setup.sh` → `/agent/code/setup.sh`

### 4.3 Docker copy commands
- `docker cp {}:/session/content/` → `docker cp {}:/agent/content/`

## Phase 5: API Routes

### 5.1 Route updates
- `/sessions` → `/agents`
- `/sessions/{name}` → `/agents/{name}`
- `/sessions/{name}/messages` → `/agents/{name}/messages`
- All session-related endpoints to agent

### 5.2 RBAC permissions
- Update permission constants: `sessions:*` → `agents:*`

## Phase 6: CLI Updates

### 6.1 Command rename
- Rename `cli/commands/session.js` → `agent.js`
- Change command from `raworc session` to `raworc agent`

### 6.2 Subcommands
- `raworc agent` (create new)
- `raworc agent restore <name>`
- `raworc agent remix <name>`
- `raworc agent publish <name>`

### 6.3 Constants and display
- Update `cli/lib/constants.js`: `SESSION_*` → `AGENT_*`
- Update display messages and icons

## Phase 7: Documentation

### 7.1 Concept docs
- Rename `website/docs/concepts/sessions.md` → `agents.md`
- Rename `website/docs/concepts/session-names-and-publishing.md` → `agent-names-and-publishing.md`
- Rename `website/docs/guides/session-playground.md` → `agent-playground.md`

### 7.2 Content updates
- Replace all references to "session" with "agent"
- Update architecture diagrams
- Update folder structure documentation:
  - `/session/code/` → `/agent/code/`
  - `/session/secrets/` → `/agent/secrets/`
  - `/session/content/` → `/agent/content/`
  - `/session/logs/` → `/agent/logs/`

## Phase 8: Final Integration

### 8.1 Testing & validation
- Run `cargo check` after all changes
- Update integration tests
- Test full workflow: build → start → CLI commands

### 8.2 Version remains 0.4.0
- Keep version as 0.4.0 (unreleased)
- Update changelog to reflect these changes as part of v0.4.0

## Key Mapping

| Old Term | New Term |
|----------|----------|
| Session | Agent |
| Host | Agent (runtime) |
| session_name | agent_name |
| session_id | agent_name |
| SessionState | AgentState |
| session container | agent container |
| host binary | agent binary |
| host user (in Docker) | agent user |
| /session/ directory | /agent/ directory |
| /session/code/ | /agent/code/ |
| /session/secrets/ | /agent/secrets/ |
| /session/content/ | /agent/content/ |
| /session/logs/ | /agent/logs/ |
| /sessions endpoint | /agents endpoint |
| raworc session command | raworc agent command |
| MESSAGE_ROLE_HOST | MESSAGE_ROLE_AGENT |

## Impact Summary
- ~75 files affected across database, backend, CLI, docs
- Major breaking change but still within unreleased v0.4.0
- All existing data would need migration (but no backward compatibility needed)
- Docker images, containers, and internal paths will be renamed
- Container internal directory structure completely changes from /session/* to /agent/*