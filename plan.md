# Plan: Remove Spaces and Agents, Simplify to Session-Based System

## Overview
Transform Raworc from a space/agent-based system to a simplified session-based system where sessions start directly with the host image and store secrets at the session level.

## Database Schema Changes

### Remove Tables Entirely
- [x] `spaces` 
- [x] `agents`
- [x] `space_secrets` 
- [x] `space_builds`
- [x] `build_tasks`

### Modify Existing Tables
- [x] **`sessions`**: Remove `space` column and foreign key constraint
- [x] **`role_bindings`**: Remove `space_id` column, update primary key to `(principal, role_name)`
- [x] **`session_messages`**: Keep roles as-is (`'user'`, `'agent'`, `'system'`)

### No New Tables Needed
- ~~`session_secrets`~~ - Secrets will be stored directly in persistent volumes at `/session/secrets/`

## RBAC Permissions for Sessions Only

### Session-Level Permissions
- `session:create` - Create new sessions
- `session:list` - List sessions (own or all based on scope)
- `session:get` - Get session details
- `session:update` - Update session metadata
- `session:delete` - Delete sessions
- `session:close` - Close/suspend sessions
- `session:restore` - Restore closed sessions
- `session:message` - Send messages to sessions

### Default Roles (only 2 roles needed)
- **admin**: Full access to all sessions
- **user**: Can only access their own sessions (created_by = their username)

## Container and Session Changes

### Session Creation Updates
- [ ] Remove space validation requirements
- [ ] Remove space build validation 
- [ ] Use host image directly (`raworc_host:latest`)
- [ ] Accept parameters during session creation:
  - **secrets**: Key-value pairs written to `/session/secrets/`
  - **instructions**: Text content written to `/session/code/instructions.md`
  - **setup**: Shell script written to `/session/code/setup.sh`
- [ ] Create session containers with persistent volume containing:
  - `/session/code/` (with `instructions.md` and `setup.sh` if provided)
  - `/session/data/` (empty directory) 
  - `/session/secrets/` (populated from creation parameters)

### Session-Level Management
- [ ] Secrets provided only at session creation time, stored as files in `/session/secrets/`
- [ ] Instructions written to `/session/code/instructions.md` and used by Host agent in every Claude call
- [ ] Setup script written to `/session/code/setup.sh` and executed before Host agent starts
- [ ] All persist across container restarts/restores
- [ ] Environment variables sourced from `/session/secrets/` on container startup
- [ ] No API endpoints to manage secrets/instructions/setup after creation

## API Endpoints Removal

### Delete Completely
- [ ] `GET/POST /spaces` - List/create spaces
- [ ] `GET/PUT/DELETE /spaces/{space}` - Get/update/delete space
- [ ] `GET/POST /spaces/{space}/secrets` - List/create space secrets
- [ ] `GET/PUT/DELETE /spaces/{space}/secrets/{key}` - Manage space secrets
- [ ] `GET/POST /spaces/{space}/agents` - List/create agents
- [ ] `GET/PUT/DELETE /spaces/{space}/agents/{name}` - Manage agents
- [ ] `PATCH /spaces/{space}/agents/{name}/status` - Update agent status
- [ ] `POST /spaces/{space}/agents/{name}/deploy` - Deploy agent
- [ ] `POST /spaces/{space}/agents/{name}/stop` - Stop agent
- [ ] `GET /spaces/{space}/agents/running` - List running agents
- [ ] `GET /spaces/{space}/agents/{name}/logs` - Agent logs
- [ ] `POST /spaces/{space}/build` - Build space
- [ ] `GET /spaces/{space}/build/latest` - Get latest build
- [ ] `GET /spaces/{space}/build/{build_id}` - Get build status
- [ ] ~~All session secrets endpoints~~ - No secret management APIs needed

### Modify Session Endpoints
- [ ] `POST /sessions` - Remove space parameter, add optional parameters (secrets, instructions, setup), add RBAC permission checks
- [ ] `GET /sessions` - Add RBAC filtering (users see own sessions, admins see all)
- [ ] All session endpoints get appropriate RBAC permission checks with user ownership validation

## Core System Removal

### Delete Files Entirely
- [x] `src/operator/space_builder.rs` - Space building logic
- [x] `src/operator/build_manager.rs` - Build task processing
- [x] `src/server/rest/handlers/spaces.rs` - Space API handlers
- [x] `src/server/rest/handlers/agents.rs` - Agent API handlers
- [x] `src/server/rest/handlers/space_secrets.rs` - Space secrets handlers
- [x] `src/server/rest/handlers/space_build.rs` - Space build handlers  
- [x] `src/server/rest/handlers/agent_logs.rs` - Agent logs handlers
- [x] `src/shared/models/space.rs` - Space model
- [x] `src/shared/models/agent.rs` - Agent model

### Simplify Host Agent
- [x] Remove agent delegation logic from `src/host/agent_manager.rs`
- [x] Remove Claude-based agent routing
- [x] Remove agent manifest loading and execution
- [x] Host handles messages directly without delegation to agents
- [x] **Update Host to read `/session/code/instructions.md` and include in every Claude API call**
- [x] **Update Host startup to execute `/session/code/setup.sh` if it exists**

### Update Docker Manager
- [x] Remove space image building
- [x] Use host image directly for all session containers
- [x] Create persistent volume with `/session/code`, `/session/data`, `/session/secrets` directories
- [x] Write secrets to `/session/secrets/` during container creation
- [x] **Write instructions to `/session/code/instructions.md` during container creation**
- [x] **Write setup script to `/session/code/setup.sh` during container creation and make executable**
- [x] Container startup script sources environment variables from `/session/secrets/` and executes setup script

### Update Operator Service
- [ ] Remove build task processing entirely
- [ ] Remove space builder integration
- [ ] Focus only on session container lifecycle management

## CLI Simplification

### Remove Commands
- [ ] All space-related commands and options
- [ ] All agent management commands  
- [ ] Space build commands

### Update Session Commands
- [ ] `raworc session` - Remove space parameter, optionally allow setting secrets, instructions, and setup at creation
- [ ] Session creation works globally without space context

### Authentication Updates
- [ ] Only admin and user roles needed
- [ ] Users can only access their own sessions
- [ ] Admins can access all sessions

## Implementation Steps

- [ ] 1. **Database Schema**: Rewrite migration to remove spaces/agents, update roles to admin/user only
- [ ] 2. **RBAC Updates**: Update permission definitions for session-only access with admin/user roles and ownership checks
- [ ] 3. **API Cleanup**: Remove all space/agent/secret endpoints from routes.rs and handlers
- [ ] 4. **Session RBAC**: Add permission checks with user ownership validation to all session endpoints
- [ ] 5. **Core Logic Removal**: Delete space_builder, build_manager, agent management files
- [ ] 6. **Host Simplification**: Remove delegation logic, add instructions reading and setup script execution
- [ ] 7. **Session Updates**: Modify session creation to accept secrets/instructions/setup, use host image, create volume structure
- [ ] 8. **Container Setup**: Update Docker manager for `/session` structure with all files written at creation
- [ ] 9. **CLI Updates**: Remove space/agent commands, simplify session workflow
- [ ] 10. **Cleanup**: Delete all unused files and references

## Expected Outcome
- Sessions as the only resource with simple ownership-based RBAC
- Only admin and user roles needed
- Secrets, instructions, and setup scripts set only at session creation
- Users can only access their own sessions, admins can access all
- Direct host image usage without builds
- `/session/code/instructions.md` used by Host agent in every Claude call
- `/session/code/setup.sh` executed before Host agent starts (if provided)
- `/session/code` and `/session/data` directories for user work
- Session secrets written to `/session/secrets/` at creation time and sourced as environment variables
- Simplified API focused only on session lifecycle
- No build pipeline or complex deployment process
- Faster session startup with direct container creation