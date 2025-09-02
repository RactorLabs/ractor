<div align="center">
  <img src="assets/logo.png" alt="Raworc Logo" width="200"/>
  
  # Raworc Project Documentation for Claude
  
  **Remote Agentic Work Orchestrator**
  
  *AI assistant instructions for understanding and working with the Raworc codebase*
  
  [![Website](https://img.shields.io/badge/Website-raworc.com-blue?style=for-the-badge)](https://raworc.com)
  [![Twitter](https://img.shields.io/badge/Twitter-@raworc-1DA1F2?style=for-the-badge&logo=twitter&logoColor=white)](https://twitter.com/raworc)
  
</div>

## Project Overview for Claude

For complete project overview, features, and architecture, see [README.md](README.md).

**Repository Status**: This repository is private and intentionally not licensed. Do not add or suggest adding license files.

**Key Points for Development:**
- Raworc is a Remote Agentic Work Orchestrator providing Computer use agents with dedicated computers
- Published as npm package (`@raworc/cli`) with pre-built Docker images from Docker Hub
- Uses Kubernetes-inspired control plane pattern for session orchestration
- Development repository for local development and contributions

## Two Ways to Use Raworc

### 🚀 End Users (Published Packages)

**For using Raworc in production or testing the system:**

```bash
# Install CLI from npm 
npm install -g @raworc/cli

# Set required environment variable
export ANTHROPIC_API_KEY=sk-ant-your-actual-key

# Start services (automatically pulls Docker images from Docker Hub)
raworc start

# Authenticate and use the system
raworc login -u admin -p admin
raworc auth -t <jwt-token-from-login>
raworc session
raworc api version
```

**Key Points:**
- Uses published Docker images from Docker Hub (`raworc/raworc_server`, etc.)
- No building required - everything is pre-built
- Install via npm package manager (CLI is Node.js implementation from cli/ folder)
- Simple, one-command setup
- No access to build/development commands

### 🛠️ Contributors (This Repository)

**For developing, contributing, or customizing Raworc:**

```bash
# Clone this repository
git clone <this-repo>
cd raworc

# Build images locally (using shell scripts from scripts/ folder)
./scripts/build.sh

# Start services with local images (using shell scripts from scripts/ folder)
./scripts/start.sh

# Link CLI for development (shell script links the Node.js CLI from cli/ folder)
./scripts/link.sh
raworc session  # Now uses your local build
```

**Key Points:**
- Builds Docker images locally from source
- Full access to build, modify, and test changes
- Uses shell scripts (scripts/ folder) for container management
- CLI is Node.js implementation (cli/ folder) linked via shell scripts
- Can modify source code and rebuild
- Publishing capabilities to Docker Hub

---

## Working with This Codebase

### Prerequisites

- **Rust toolchain** - For building backend services
- **Node.js 16+** - For CLI development  
- **Docker** - For container orchestration
- **MySQL** - Database (auto-managed in Docker)

### Repository Structure

```
raworc/
├── src/           # Rust backend services
│   ├── server/    # API server  
│   ├── operator/  # Session orchestration
│   └── host/      # Session runtime with Claude integration
├── cli/           # Node.js CLI (@raworc/cli)
├── scripts/       # Development automation
├── website/       # Documentation site (Docusaurus)
├── migrations/    # Database schema
└── docker/        # Docker configurations
```

### Development Scripts

| Command | Purpose |
|---------|---------|
| `./scripts/link.sh` | Link CLI for development |
| `./scripts/build.sh` | Build Rust binaries and Docker images |
| `./scripts/start.sh` | Start development services |
| `./scripts/stop.sh` | Stop services |
| `./scripts/restart.sh` | Restart all services |
| `./scripts/reset.sh` | Complete cleanup |
| `./scripts/push.sh` | Push images to registry |
| `./scripts/publish.sh` | Publish CLI to npm |

### Testing

```bash
# Full integration test (requires ANTHROPIC_API_KEY)
export ANTHROPIC_API_KEY=sk-ant-api03-your-key
./scripts/build.sh
./scripts/start.sh
./scripts/link.sh
raworc login -u admin -p admin
raworc auth -t <jwt-token-from-login>
raworc session
```

### Publishing

This repository publishes to:
- **Docker Hub** - `raworc/raworc_server`, `raworc/raworc_operator`, `raworc/raworc_host`
- **npm** - `@raworc/cli` package

### Architecture

Raworc uses a **Kubernetes-inspired control plane** pattern for Computer use agent orchestration:

```
┌────────────┐      ┌─────────────────────────────────┐
│ raworc CLI │─────▶│          Control Plane          │
└────────────┘      │ ┌─────────────┐ ┌─────────────┐ │
                    │ │ API Server  │ │    MySQL    │ │
                    │ └─────────────┘ └─────────────┘ │
                    │        │                        │
                    │        ▼                        │
                    │ ┌─────────────┐                 │
                    │ │  Operator   │                 │
                    │ └─────────────┘                 │
                    └─────────────────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────┐
                    │    Computer Use Agents          │
                    │ ┌─────────────┐ ┌─────────────┐ │
                    │ │   Host +    │ │   Host +    │ │
                    │ │  Computer   │ │  Computer   │ │
                    │ └─────────────┘ └─────────────┘ │
                    └─────────────────────────────────┘
```

### Development vs End User Usage

**For Development (this repository):**
- Use shell scripts (scripts/ folder) for local development with locally built images
- Use linked CLI (Node.js implementation from cli/ folder) via `./scripts/link.sh`
- Build images locally with shell scripts: `./scripts/build.sh`
- All images are built locally (no registry dependency)

**For End Users (published packages):**
- Install CLI (Node.js/npm package) via npm: `npm install -g @raworc/cli`
- Uses published Docker images from Docker Hub automatically
- No building required - images are pulled as needed
- Simple `raworc start` command handles everything


## CLI Reference

The Raworc CLI provides complete control over the orchestrator. Install globally via npm or use for development.

### Installation

**End Users (Production)**
```bash
npm install -g @raworc/cli
export ANTHROPIC_API_KEY=sk-ant-your-actual-key
raworc start
raworc login -u admin -p admin
raworc auth -t <jwt-token-from-login>
```

**Contributors (Development)**  
```bash
git clone <this-repo>
./scripts/link.sh  # Link CLI for development
```

### Authentication Commands

| Command | Description | Example |
|---------|-------------|---------|
| `raworc login` | Generate operator authentication token | `raworc login -u admin -p admin` |
| `raworc auth` | Authenticate with token or show status | `raworc auth -t <jwt-token>` |
| `raworc logout` | Clear authentication credentials | `raworc logout` |
| `raworc token` | Create authentication token for principal | `raworc token -p myuser -t User` |

### Service Management

| Command | Description | Example |
|---------|-------------|---------|
| `raworc start` | Start Docker services | `raworc start server mysql` |
| `raworc stop` | Stop Docker services | `raworc stop` |
| `raworc clean` | Clean containers and images | `raworc clean --all` |
| `raworc reset` | **Nuclear option**: Clean everything | `raworc reset` |
| `raworc pull` | Update CLI and Docker images | `raworc pull` |

### Session Management

| Command | Description | Example |
|---------|-------------|---------|
| `raworc session` | Start interactive session | `raworc session` |
| `raworc session restore <id>` | Restore session | `raworc session restore abc123` |
| `raworc session remix <id>` | Remix session | `raworc session remix abc123` |
| `raworc session publish <id>` | Publish session | `raworc session publish abc123` |
| `raworc session unpublish <id>` | Unpublish session | `raworc session unpublish abc123` |

### API Access

| Command | Description | Example |
|---------|-------------|---------|
| `raworc api <endpoint>` | Direct REST API calls | `raworc api sessions` |
| `raworc api version` | Check server health | `raworc api version` |
| `raworc api sessions -m POST` | Create session via API | `raworc api sessions -m POST` |

### CLI Options Reference

**Global Options:**
- `-v, --version` - Show version
- `-h, --help` - Show help

**Authentication:**
- `raworc login [-u/--user] [-p/--pass] [-s/--server]` - Generate operator token
- `raworc auth [-t/--token] [-s/--server]` - Authenticate with token or show status  
- `raworc logout` - Clear authentication credentials
- `raworc token [-p/--principal] [-t/--type]` - Create token for principal

**Service Management:**
- `raworc start [-r/--restart] [components...]` - Start services
- `raworc stop [-c/--cleanup] [components...]` - Stop services
- `raworc clean` - Clean session containers (preserves core services)
- `raworc reset [-y/--yes] [-s/--services-only]` - Complete cleanup
- `raworc pull [-c/--cli-only] [-i/--images-only]` - Update CLI and images

**Session Management:**
- `raworc session [-n/--name] [-t/--timeout] [-S/--secrets] [-i/--instructions] [-if/--instructions-file] [-s/--setup] [-sf/--setup-file] [-p/--prompt]` - Start new session
- `raworc session restore <session-id> [-p/--prompt]` - Restore existing session
- `raworc session remix <session-id> [-n/--name] [-d/--data] [-c/--code] [-s/--secrets] [-p/--prompt]` - Create remix session
- `raworc session publish <session-id> [-d/--data] [-c/--code] [-s/--secrets]` - Publish session
- `raworc session unpublish <session-id>` - Unpublish session

**API Access:**
- `raworc api <endpoint> [-m/--method] [-b/--body] [-H/--headers] [-p/--pretty] [-s/--status]` - Execute API requests

## API Reference

Raworc exposes a comprehensive REST API for programmatic access to all functionality.

### Base URL
```
http://localhost:9000/api/v0
```

### Authentication

**Operator Login**
```bash
POST /operators/{name}/login
Content-Type: application/json

{
  "pass": "admin"
}

Response:
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "token_type": "Bearer", 
  "expires_at": "2025-09-02T03:26:35Z",
  "user": "admin",
  "role": "admin"
}
```

**Token Validation**
```bash
GET /auth
Authorization: Bearer <jwt-token>

Response:
{
  "user": "admin",
  "type": "Operator"
}
```

**Create Token**
```bash
POST /auth/token
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "principal": "myuser",
  "type": "User"
}

Response:
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "expires_at": "2025-09-02T03:26:35Z"
}
```

### Sessions

**List Sessions**
```bash
GET /sessions
Authorization: Bearer <jwt-token>

Response:
[
  {
    "id": "session-abc123",
    "state": "running",
    "created_at": "2025-09-01T12:00:00Z",
    "updated_at": "2025-09-01T12:30:00Z"
  }
]
```

**Create Session**
```bash
POST /sessions
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "instructions": "Analyze this data file",
  "setup": "pip install pandas",
  "secrets": {
    "DATABASE_URL": "mysql://user:pass@host/db"
  },
  "prompt": "Hello, analyze this data"
}

Response:
{
  "id": "session-abc123",
  "state": "created",
  "created_at": "2025-09-01T12:00:00Z"
}
```

**Get Session**
```bash
GET /sessions/{id}
Authorization: Bearer <jwt-token>

Response:
{
  "id": "session-abc123",
  "state": "running",
  "created_at": "2025-09-01T12:00:00Z",
  "updated_at": "2025-09-01T12:30:00Z",
  "instructions": "Analyze this data file"
}
```

**Session Actions**
```bash
# Close session
POST /sessions/{id}/close
Authorization: Bearer <jwt-token>

# Restore session with optional prompt
POST /sessions/{id}/restore
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "prompt": "Let's continue working"
}

# Remix session with optional prompt
POST /sessions/{id}/remix
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "data": true,
  "code": false,
  "prompt": "Try a different approach"
}

# Update session state
PUT /sessions/{id}/state
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "state": "paused"
}

# Delete session
DELETE /sessions/{id}
Authorization: Bearer <jwt-token>
```

### Messages

**List Messages**
```bash
GET /sessions/{id}/messages
Authorization: Bearer <jwt-token>

Response:
[
  {
    "id": "msg-123",
    "content": "Hello, please analyze this file",
    "role": "user", 
    "timestamp": "2025-09-01T12:00:00Z"
  },
  {
    "id": "msg-124",
    "content": "I'll analyze the file for you...",
    "role": "assistant",
    "timestamp": "2025-09-01T12:01:00Z"
  }
]
```

**Send Message**
```bash
POST /sessions/{id}/messages
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "content": "What's in this file?",
  "role": "user"
}

Response:
{
  "id": "msg-125",
  "content": "What's in this file?",
  "role": "user",
  "timestamp": "2025-09-01T12:02:00Z"
}
```

**Message Count**
```bash
GET /sessions/{id}/messages/count
Authorization: Bearer <jwt-token>

Response:
{
  "count": 42
}
```

**Clear Messages**
```bash
DELETE /sessions/{id}/messages
Authorization: Bearer <jwt-token>

Response:
{
  "deleted": 42
}
```

### Operators

**List Operators**
```bash
GET /operators
Authorization: Bearer <jwt-token>

Response:
[
  {
    "user": "admin",
    "created_at": "2025-09-01T10:00:00Z",
    "last_login": "2025-09-01T12:00:00Z"
  }
]
```

**Create Operator**
```bash
POST /operators
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "user": "newuser",
  "pass": "securepassword"
}
```

**Update Operator Password**
```bash
PUT /operators/{name}/password
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "pass": "newpassword"
}
```

**Delete Operator**
```bash
DELETE /operators/{name}
Authorization: Bearer <jwt-token>
```

### System

**Health Check**
```bash
GET /version

Response:
{
  "version": "0.3.3",
  "api": "v0"
}
```

### Error Responses

All API endpoints return consistent error responses:

```json
{
  "error": "Authentication required",
  "status": 401
}
```

Common HTTP status codes:
- `200` - Success
- `201` - Created  
- `400` - Bad Request
- `401` - Unauthorized
- `403` - Forbidden
- `404` - Not Found
- `500` - Internal Server Error

### API Authentication

All protected endpoints require a Bearer token in the Authorization header:

```bash
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ5...
```

Obtain tokens through:
1. **Operator Login**: `POST /operators/{name}/login`
2. **Token Creation**: `POST /auth/token` (requires existing auth)

## Development Notes for Claude

### **🚨 CRITICAL CLI Usage Rule**

**❌ NEVER DO THIS:**
```bash
node index.js start                    # Wrong - direct Node.js execution
cd cli && node index.js --help         # Wrong - manual CLI execution
node cli/index.js api health          # Wrong - bypasses linking
```

**✅ ALWAYS DO THIS:**
```bash
# First, link the CLI (Node.js implementation from cli/ folder) using shell script
./scripts/link.sh

# Then use the raworc command (linked Node.js CLI)
raworc start
raworc --help 
raworc api health
```

**Why?**
- The shell script `./scripts/link.sh` properly links the Node.js CLI from cli/ folder
- Direct `node index.js` execution runs from wrong directory context and may fail
- Linked command matches production behavior exactly
- Avoids path resolution and module loading issues
- Required for proper testing of CLI functionality
- Shell scripts (scripts/ folder) handle proper setup and linking

### Project Structure

For complete repository structure, see [README.md](README.md#repository-structure).

### Development Guidelines

**Key Development Patterns:**
- **Error Handling**: Use `Result<T, E>` types with custom error types
- **Async Code**: Uses Tokio runtime for async operations
- **Database**: MySQL with sqlx for queries
- **API**: RESTful JSON API with JWT authentication
- **Testing**: Unit tests in modules, integration tests in `tests/`

**Testing After Changes:**
1. **Run unit tests**: `cargo test --lib`
2. **Run integration tests**: `cargo test --test integration`  
3. **Check formatting**: `cargo fmt --check`
4. **Run linter**: `cargo clippy`
5. **Build release**: `cargo build --release`

## Notes for Claude

This document provides Claude-specific development instructions. For general project information, see [README.md](README.md).

## Communication Style Guidelines

When writing commit messages and documentation:
- **NEVER mention "Claude", "Claude Code", or any AI assistant references in commit messages, PRs, or code**
- Do not use emojis in commits or code changes
- Use direct, clear language without unnecessary emphasis
- Avoid overemphasizing words like "comprehensive", "extensive", "robust", etc.
- Keep commit messages concise and factual
- Write as if changes were made by a human developer, not an AI assistant

## Commit Message Standards

Use conventional commit format:

```
type: brief description

Optional longer description explaining the change.
```

**CRITICAL: Do NOT include any of the following in commit messages:**
- `🤖 Generated with [Claude Code](https://claude.ai/code)`
- `Co-Authored-By: Claude <noreply@anthropic.com>`
- Any mention of "Claude", "AI assistant", or automated generation
- Emojis or attribution signatures

**Write commits as if made by a human developer - purely technical and professional.**

### Commit Types
- `feat`: New feature or functionality
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring without feature changes
- `test`: Adding or updating tests
- `chore`: Maintenance tasks, dependencies, build changes
- `perf`: Performance improvements
- `style`: Code style changes (formatting, whitespace)
- `remove`: Removing files or functionality

### Commit Guidelines
- Keep the subject line under 50 characters
- Use imperative mood ("add" not "added" or "adds")
- Do not end the subject line with a period
- Separate subject from body with a blank line
- Focus on what and why, not how

## Code Standards

### Before Committing
- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Test changes in development environment

### Making Changes (Critical Workflow)
- **ALWAYS run `cargo check` immediately after completing any code changes**
- **WAIT for user instruction before testing or committing changes**
- Complete all changes first, then run cargo check to verify compilation
- Only proceed with testing or committing after receiving explicit user approval
- Always use the shell scripts (scripts/ folder) for local development:
  - Use `./scripts/build.sh` instead of `cargo build` or `docker build`
  - Use `./scripts/start.sh` instead of manual `docker run` commands
  - Use `./scripts/restart.sh` instead of manual restart sequences
  - **CRITICAL**: Link CLI with `./scripts/link.sh` then use `raworc` command
  - **NEVER use `node index.js` or `node cli/index.js`** - always use the linked `raworc` command (Node.js CLI from cli/ folder)
- Published CLI users should use `raworc` commands directly (Node.js package installed via npm)

### Version Bump Requirements

**For detailed version bump requirements and complete file list, see `.claude/commands/release.md`**

**Quick Summary:**
- Update version in 8 different files (Cargo.toml, package.json files, API response, docs)
- Run `cargo build --release` to validate and update Cargo.lock
- Run `npm install` in cli/ and website/ folders to update package-lock.json files
- Commit all files including lock files to ensure consistency

### Branch Naming
Use descriptive branch names:
```
type/brief-description
```

Examples:
- `feat/session-close-restore`
- `fix/container-cleanup-race`
- `docs/api-reference-update`
