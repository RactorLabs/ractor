# Plan: Raworc v0.4.0 - Complete System Overhaul

## Overview
Complete system redesign using session name as the primary key, removing all ID references, removing data folder, and adding public canvas serving capability.

## 1. Database Schema - Ultra-Clean
```sql
CREATE TABLE sessions (
    name VARCHAR(255) PRIMARY KEY,
    created_by VARCHAR(255) NOT NULL,
    state VARCHAR(50) NOT NULL DEFAULT 'init',
    -- NO container_id - derived from name
    -- NO persistent_volume_id - derived from name
    -- NO id field at all
    parent_session_name VARCHAR(255),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TIMESTAMP NULL,
    metadata JSON DEFAULT ('{}'),
    is_published BOOLEAN NOT NULL DEFAULT false,
    published_at TIMESTAMP NULL,
    published_by VARCHAR(255) NULL,
    publish_permissions JSON DEFAULT ('{"code": true, "secrets": true}'),
    timeout_seconds INT NOT NULL DEFAULT 300,
    auto_close_at TIMESTAMP NULL,
    canvas_port INT NULL,
    FOREIGN KEY (parent_session_name) REFERENCES sessions(name),
    CONSTRAINT sessions_name_check CHECK (name REGEXP '^[a-z][a-z0-9-]{0,61}[a-z0-9]$')
)
```

## 2. Everything Derived from Name
```
Session Name: my-project
├── Container: raworc_session_my-project
├── Volume: raworc_session_data_my-project
├── Public Dir: /public/my-project/
└── Canvas URL: http://localhost:8000/my-project/
```

## 3. Remove "Data" Folder Completely
- **Session Volume Structure:**
  - `/session/code/` ✓
  - `/session/secrets/` ✓
  - `/session/canvas/` ✓
  - ~~`/session/data/`~~ ✗ REMOVED

## 4. Remove All Data References
- **CLI:** Remove `--data` flag from remix/publish
- **API:** Remove `data` field from all requests/responses
- **Docker:** Remove data folder creation/copying
- **Prompts:** Remove `/session/data` from all instructions

## 5. Session Model - Minimal
```rust
pub struct Session {
    pub name: String,  // Primary key
    pub created_by: String,
    pub state: String,
    // NO container_id
    // NO persistent_volume_id
    // NO id
    pub parent_session_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<String>,
    pub publish_permissions: serde_json::Value,
    pub timeout_seconds: i32,
    pub auto_close_at: Option<DateTime<Utc>>,
    pub canvas_port: Option<i32>,
}
```

## 6. Docker Manager Simplifications
- **No container ID tracking** - use name directly
- **Container operations:**
  ```rust
  let container_name = format!("raworc_session_{}", session_name);
  // Check if container exists
  docker.inspect_container(&container_name)
  // Stop container
  docker.stop_container(&container_name)
  ```
- **Volume operations:**
  ```rust
  let volume_name = format!("raworc_session_data_{}", session_name);
  ```

## 7. API Endpoints - Pure Name-Based
- `POST /sessions` - Body: `{ "name": "my-project", ... }`
- `GET /sessions/{name}`
- `POST /sessions/{name}/close`
- `POST /sessions/{name}/restore`
- `POST /sessions/{name}/remix`
- `POST /sessions/{name}/publish`
- `POST /sessions/{name}/unpublish`
- `DELETE /sessions/{name}`

## 8. Docker Volume for Public Content
- **Setup:**
  ```bash
  docker volume create raworc_public
  docker run -v raworc_public:/public ...
  ```
- **Mount:** `/public` in server container
- **Persists** across container rebuilds

## 9. Public HTTP Server (Port 8000)
```rust
// In server startup
tokio::spawn(async {
    start_public_server().await
});

// Serve /public directory on port 8000
async fn start_public_server() {
    // Serve static files from /public
    // URL: http://localhost:8000/{session_name}/
}
```

## 10. Canvas Publishing
- **Publish:**
  ```bash
  # Copy from container to public volume
  docker cp raworc_session_${name}:/session/canvas/. \
           /public/${name}/
  ```
- **Unpublish:**
  ```bash
  rm -rf /public/${name}
  ```

## 11. CLI Simplifications
- **Remove:**
  - `--data` flag from all commands
  - `/name` command from session interface
  - All ID references
  - Name change capability
- **Keep:**
  - `--code` flag for remix/publish
  - `--secrets` flag for remix/publish
  - Canvas (always included)

## 12. Files to Modify

**Database:**
- `db/migrations/20250902000001_complete_schema.sql` - New schema

**Models:**
- `src/shared/models/session.rs` - Remove id, container_id, persistent_volume_id
- `src/shared/models/mod.rs` - Update structures

**API:**
- `src/server/rest/handlers/sessions.rs` - Name-based operations
- `src/server/rest/routes.rs` - Update routes

**Docker:**
- `src/operator/docker_manager.rs` - Derive container/volume names
- `src/operator/session_manager.rs` - Use names only

**CLI:**
- `cli/commands/session.js` - Remove data flag, ID usage
- `cli/lib/api.js` - Name-based API calls

**Server:**
- `src/server/rest/server.rs` - Add public HTTP server on port 8000
- `src/server/public_server.rs` - New file for public server
- `Dockerfile.server` - Expose 8000, create /public

**Scripts:**
- `scripts/start.sh` - Create and mount public volume, map port 8000
- `docker-compose.yml` - Add volume definition, expose port 8000

**Host:**
- `src/host/mod.rs` - Remove data folder from prompts

## 13. State Management Without Container ID
```rust
// Instead of checking container_id field
if session.container_id.is_some() { ... }

// Check container directly
let container_name = format!("raworc_session_{}", session.name);
if docker.container_exists(&container_name).await? { ... }
```

## 14. Benefits
- **No sync issues** - Container state always accurate
- **Less database updates** - No need to store/clear container ID
- **Simpler code** - Everything derives from name
- **Direct Docker operations** - No ID lookups
- **Ultra-simple** - Name is the only identifier
- **Clean structure** - Only 3 folders (code, secrets, canvas)
- **Predictable** - Everything derives from session name
- **No redundancy** - No separate volume ID storage
- **User-friendly** - Meaningful names everywhere

## 15. Example Usage
```bash
# Create session
raworc session start --name my-analysis

# All operations use name
raworc session restore my-analysis
raworc session publish my-analysis --secrets false

# Everything predictable
Container: raworc_session_my-analysis
Volume: raworc_session_data_my-analysis
Public: http://localhost:8000/my-analysis/
```

## 16. Port Configuration
- **REST API:** Port 9000 (unchanged)
- **Public Server:** Port 8000 
- **Canvas (in host):** Dynamic ports per session

## 17. Implementation Order
1. Create new database schema (complete replacement)
2. Update Session model (remove all derived fields)
3. Update Docker manager (use name-based operations)
4. Remove data folder references everywhere
5. Add public volume and HTTP server on port 8000
6. Implement publish/unpublish with canvas copy
7. Update CLI (remove data flag, IDs, name change)
8. Update all API endpoints
9. Test complete workflow
10. Reset database with new schema

## Notes
- No backward compatibility needed - complete data reset
- This creates the simplest possible system where everything flows from the session name
- The public volume ensures published content persists across container rebuilds