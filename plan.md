# Session → Sandbox Refactoring Plan

## Overview
This plan outlines a comprehensive refactoring to rename "session" to "sandbox" throughout the codebase, introduce a snapshot system, and simplify the container lifecycle.

## Phase 1: Database Schema Changes

### 1.1 Create New Snapshots Table
```sql
CREATE TABLE snapshots (
    id CHAR(36) PRIMARY KEY,
    sandbox_id CHAR(36) NOT NULL,
    trigger_type ENUM('session_close', 'user') NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata JSON,
    FOREIGN KEY (sandbox_id) REFERENCES sessions(id)
);
```

### 1.2 Rename Sessions Table → Sandboxes
- Rename `sessions` table to `sandboxes`
- Rename `session_tasks` to `sandbox_tasks`
- Rename `session_requests` to `sandbox_requests`

### 1.3 Update Column Names
- Rename `parent_session_id` → `parent_sandbox_id`
- Rename `stop_timeout_seconds` → `idle_timeout_seconds` (default: 900 = 15 mins)
- Remove `archive_timeout_seconds` column
- Update state enum: replace `stopped` with `deleted`

### 1.4 Update Foreign Keys
- Update all foreign key references from `session_id` → `sandbox_id`
- Update indexes accordingly

## Phase 2: Volume Architecture Changes

### 2.1 Remove Individual Sandbox Volumes
- Sandboxes no longer get individual Docker volumes
- Data lives directly in the container filesystem
- Remove volume creation logic from controller

### 2.2 Create Shared Snapshots Volume
- Create single `tsbx_snapshots_data` volume
- Mount to controller at `/data/snapshots/`
- Structure: `/data/snapshots/{snapshot_id}/` contains snapshot data

### 2.3 Remove Unused Volumes
- Audit and remove: `api_data`, `controller_data`, `operator_data` if unused

## Phase 3: API Changes

### 3.1 Rename All Endpoints
- `/api/v0/sessions/*` → `/api/v0/sandboxes/*`
- `/sessions/{id}/idle` → `/sandboxes/{id}/idle`
- `/sessions/{id}/busy` → `/sandboxes/{id}/busy`
- `/sessions/{id}/stop` → `/sandboxes/{id}/delete` (or keep as "stop" but it deletes)
- `/sessions/{id}/tasks` → `/sandboxes/{id}/tasks`

### 3.2 Remove Endpoints
- Remove `/sessions/{id}/restart` entirely
- Remove remix/clone endpoints

### 3.3 Add New Snapshot Endpoints
- `POST /sandboxes/{id}/snapshots` - Create snapshot
  - Returns `{ snapshot_id, created_at }`
  - Controller copies container data to `/data/snapshots/{snapshot_id}/`
- `GET /sandboxes/{id}/snapshots` - List snapshots for sandbox
- `GET /snapshots` - List all snapshots
- `GET /snapshots/{id}` - Get snapshot details
- `DELETE /snapshots/{id}` - Delete snapshot

### 3.4 Update Create Endpoint
- `POST /sandboxes` accepts optional `snapshot_id` parameter
- If provided, controller restores from `/data/snapshots/{snapshot_id}/`

### 3.5 Deleted Sandbox Behavior
- All endpoints except `GET /sandboxes/{id}` return error for deleted sandboxes
- Error message: "Sandbox is deleted. Please create a new one."
- `GET /sandboxes/{id}` returns full details including `state: "deleted"`

## Phase 4: Controller Changes

### 4.1 Container Lifecycle
- Rename action: "start" → "create"
- On stop/timeout: delete container (no restart option)
- Auto-delete after idle_timeout (15 mins default)

### 4.2 Volume Mounting
- Mount `tsbx_snapshots_data` at `/data/snapshots/`
- Remove individual volume creation/mounting for sandboxes

### 4.3 Snapshot Creation Logic
- On sandbox stop/delete request: auto-create snapshot before deletion
- Copy container filesystem to `/data/snapshots/{snapshot_id}/`
- Use `docker cp` or exec into container to tar/copy data
- Record in snapshots table with `trigger_type: 'session_close'`

### 4.4 Snapshot Restoration Logic
- On sandbox create with `snapshot_id`:
- Copy from `/data/snapshots/{snapshot_id}/` into new container
- Use init script or entrypoint to restore data

### 4.5 Container Naming
- `tsbx_session_{id}` → `tsbx_sandbox_{id}`
- Volume names (if any temp volumes needed): `tsbx_sandbox_temp_{id}`

## Phase 5: Session Runtime Changes

### 5.1 API Endpoint Updates
- Update state reporting to use `/sandboxes/*` endpoints
- POST to `/sandboxes/{id}/idle` and `/sandboxes/{id}/busy`

### 5.2 Binary Renaming
- `tsbx-session` → `tsbx-sandbox` (or keep as-is for now?)

## Phase 6: CLI Changes

### 6.1 Command Updates
- Update all session references to sandbox in help text
- `tsbx stop sessions` → `tsbx stop sandboxes`
- Update environment variable injection

### 6.2 Output Messages
- Replace "session" → "sandbox" in all CLI output

## Phase 7: Operator UI Changes

### 7.1 Route Updates
- `/sessions/*` → `/sandboxes/*`
- Update all navigation, links, and route handlers

### 7.2 UI Text Updates
- Replace "Session" → "Sandbox" throughout
- Update button labels: "Stop" → "Delete" (or "Stop & Snapshot")
- Remove "Restart" button
- Remove "Remix" functionality

### 7.3 New Snapshot UI
- Add "Snapshots" section to sidebar
- List view for all snapshots
- "Create from Snapshot" button on snapshot details
- Show snapshot trigger type and source sandbox

### 7.4 Remove Features
- Remove name input (already done)
- Remove remix modal and functionality
- Remove restart option

## Phase 8: Shared Code Changes

### 8.1 Struct/Type Renaming
- `Session` → `Sandbox`
- `SessionState` → `SandboxState` (update enum: `Deleted` instead of `Stopped`)
- `SessionTask` → `SandboxTask`
- `SessionRequest` → `SandboxRequest`

### 8.2 Database Models
- Update all sqlx queries and structs

## Phase 9: Scripts & Docker Changes

### 9.1 Docker Compose
- Update volume definitions
- Add `tsbx_snapshots_data` volume
- Remove unused volumes
- Update service names if needed

### 9.2 Build Scripts
- Update image names/tags if they reference "session"

## Phase 10: Testing & Validation

### 10.1 Unit Tests
- Update all test names and assertions
- Test snapshot creation/restoration
- Test deleted sandbox behavior
- Test idle timeout → auto-delete

### 10.2 Integration Tests
- Full lifecycle: create → idle → delete → snapshot created
- Create from snapshot
- Verify old APIs return 404 or appropriate errors
- Verify only GET works on deleted sandboxes

### 10.3 Manual Testing
- Build all services
- Run through UI workflows
- Test CLI commands
- Verify data persistence in snapshots

## Questions & Clarifications Needed

### Q1: Snapshot Data Scope
- What data should be snapshotted? Entire container filesystem or specific paths?
- Should we snapshot `/workspace`, `/home`, or root `/`?

### Q2: Snapshot Size Limits
- Should there be a max snapshot size?
- Should we compress snapshots (tar.gz)?

### Q3: Auto-Snapshot on Delete
- Should EVERY delete trigger a snapshot, or only graceful stops?
- What about forced kills or errors?

### Q4: Snapshot Retention
- Should snapshots auto-expire after N days?
- Should there be a max snapshot count per sandbox?

### Q5: Create from Snapshot - Copy or Mount?
- Should we copy data into the new container or mount snapshot as read-only volume?
- Copy is cleaner but slower; mount is faster but more complex

### Q6: Binary Names
- Should `tsbx-session` binary be renamed to `tsbx-sandbox`?
- This affects Docker images, paths, process names

### Q7: Backward Compatibility
- Should old `/api/v0/sessions/*` endpoints return 410 Gone or just 404?
- Should we keep aliases during transition?

### Q8: Migration Strategy
- How to handle existing sessions during migration?
- Convert all to sandboxes? Create migration snapshots?

### Q9: State Transition for Deleted
- Can a deleted sandbox ever transition to another state?
- Or is deleted terminal forever?

### Q10: Idle Timeout Behavior
- Should the timeout be configurable per sandbox?
- Should there be a warning before auto-delete?

## Execution Order

1. ✅ **Create plan.md** - Document and confirm approach
2. Database migrations for snapshots table
3. Database migrations for rename + schema changes
4. Update shared types/structs
5. Update API handlers
6. Update Controller snapshot + lifecycle logic
7. Update Session runtime
8. Update CLI
9. Update Operator UI
10. Update Docker configs
11. Update scripts
12. Testing
13. Documentation updates

## Risk Areas

- **Data loss**: Auto-delete after 15 mins could surprise users
- **Breaking changes**: All API endpoints change, external integrations break
- **Migration complexity**: Existing sessions need conversion
- **Volume operations**: Copying data between controller and containers could be slow/fragile
- **Race conditions**: Snapshot creation during container shutdown needs careful orchestration

## Estimated Scope

- **Files to modify**: ~100+ files across Rust, TypeScript, SQL, Docker
- **Lines of code**: ~5000+ changes (mostly renaming, some logic changes)
- **Complexity**: High - touches every layer of the stack
- **Testing effort**: Significant - need full regression testing

---

**Next Step**: Review this plan, answer clarifying questions, then proceed phase by phase.
