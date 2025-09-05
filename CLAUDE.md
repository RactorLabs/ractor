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
- Uses Kubernetes-inspired control plane pattern for agent orchestration
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
raworc agent
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
raworc agent  # Now uses your local build
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
│   ├── operator/  # Agent orchestration
│   └── agent/      # Agent runtime with Claude integration
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
raworc agent
```

### Publishing

This repository publishes to:

- **Docker Hub** - `raworc/raworc_server`, `raworc/raworc_operator`, `raworc/raworc_agent`
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
                    │ │   Agent +   │ │   Agent +   │ │
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

### Agent Management

| Command | Description | Example |
|---------|-------------|---------|
| `raworc agent` | Start interactive agent | `raworc agent` |
| `raworc agent wake <id>` | Wake agent | `raworc agent wake abc123` |
| `raworc agent remix <id>` | Remix agent | `raworc agent remix abc123` |
| `raworc agent publish <id>` | Publish agent | `raworc agent publish abc123` |
| `raworc agent unpublish <id>` | Unpublish agent | `raworc agent unpublish abc123` |

### API Access

| Command | Description | Example |
|---------|-------------|---------|
| `raworc api <endpoint>` | Direct REST API calls | `raworc api agents` |
| `raworc api version` | Check server health | `raworc api version` |
| `raworc api agents -m POST` | Create agent via API | `raworc api agents -m POST` |

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
- `raworc clean` - Clean agent containers (preserves core services)
- `raworc reset [-y/--yes] [-s/--services-only]` - Complete cleanup
- `raworc pull [-c/--cli-only] [-i/--images-only]` - Update CLI and images

**Agent Management:**

- `raworc agent [-n/--name] [-t/--timeout] [-S/--secrets] [-i/--instructions] [-if/--instructions-file] [-s/--setup] [-sf/--setup-file] [-p/--prompt]` - Start new agent
- `raworc agent wake <agent-name> [-p/--prompt]` - Wake existing agent
- `raworc agent remix <agent-name> [-n/--name] [-d/--data] [-c/--code] [-s/--secrets] [-p/--prompt]` - Create remix agent
- `raworc agent publish <agent-name> [-d/--data] [-c/--code] [-s/--secrets]` - Publish agent
- `raworc agent unpublish <agent-name>` - Unpublish agent
- `raworc agent open <agent-name>` - Show content links (private and public)
- `raworc agent sleep <agent-name>` - Sleep an active agent

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

### Agents

**List Agents**

```bash
GET /agents
Authorization: Bearer <jwt-token>

Response:
[
  {
    "id": "agent-abc123",
    "state": "running",
    "created_at": "2025-09-01T12:00:00Z",
    "updated_at": "2025-09-01T12:30:00Z"
  }
]
```

**Create Agent**

```bash
POST /agents
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "instructions": "Analyze this data file",
  "setup": "pip install pandas",
  "secrets": {
    "DATABASE_URL": "mysql://user:pass@agent/db"
  },
  "prompt": "Hello, analyze this data"
}

Response:
{
  "id": "agent-abc123",
  "state": "created",
  "created_at": "2025-09-01T12:00:00Z"
}
```

**Get Agent**

```bash
GET /agents/{id}
Authorization: Bearer <jwt-token>

Response:
{
  "id": "agent-abc123",
  "state": "running",
  "created_at": "2025-09-01T12:00:00Z",
  "updated_at": "2025-09-01T12:30:00Z",
  "instructions": "Analyze this data file"
}
```

**Agent Actions**

```bash
# Sleep agent
POST /agents/{id}/sleep
Authorization: Bearer <jwt-token>

# Wake agent with optional prompt
POST /agents/{id}/wake
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "prompt": "Let's continue working"
}

# Remix agent with optional prompt
POST /agents/{id}/remix
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "data": true,
  "code": false,
  "prompt": "Try a different approach"
}

# Update agent state
PUT /agents/{id}/state
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "state": "paused"
}

# Delete agent
DELETE /agents/{id}
Authorization: Bearer <jwt-token>
```

### Messages

**List Messages**

```bash
GET /agents/{id}/messages
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
POST /agents/{id}/messages
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
GET /agents/{id}/messages/count
Authorization: Bearer <jwt-token>

Response:
{
  "count": 42
}
```

**Clear Messages**

```bash
DELETE /agents/{id}/messages
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
  "version": "0.4.4",
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

- `feat/agent-sleep-wake`
- `fix/container-cleanup-race`
- `docs/api-reference-update`

## CLI Design System

The Raworc CLI uses a consistent, professional design system with flat geometric icons and standardized layouts. All CLI commands follow this unified visual approach for a cohesive user experience.

### Design Principles

**Flat Icon System**: Uses simple geometric Unicode characters instead of emojis for terminal compatibility and professional appearance:

- `◯` (clean) - Agent container cleanup
- `◎` (auth) - Authentication operations  
- `◐` (user) - User/operator login/logout
- `⬟` (token) - Token operations
- `▶` (start) - Start services
- `◼` (stop) - Stop services  
- `⇣` (pull) - Pull/download operations
- `⟲` (reset) - Reset/restart operations
- `◈` (api) - API operations
- `◊` (agent) - Agent operations
- `≡` (history) - History/logs
- `◉` (chat) - Chat/messaging

**Status Icons**:

- `✓` (success) - Successful operations
- `✗` (error) - Error conditions
- `⚠` (warning) - Warning conditions  
- `ℹ` (info) - Information messages

**Agent State Visual Indicators**:

- `●` (idle) - Agent idle state
- `◉` (busy) - Agent processing  
- `◎` (init) - Agent initializing
- `◼` (slept) - Agent slept
- `◼` (deleted) - Agent deleted

### Command Box Layout

All CLI commands use a standardized command box format:

```
┌─────────────────────────────────────┐
│ [icon] Command Title                │
│ Server: http://localhost:9000       │
│ User: admin (Operator)              │
│ Operation: Brief operation desc     │
└─────────────────────────────────────┘
```

**Box Structure**:

- Title row with appropriate flat icon and command name
- Server URL (when relevant)
- Current authenticated user and type (when relevant)  
- Operation description explaining what the command does
- Optional target/agent information for specific operations

### Agent Interface

Interactive agents use a clean command box with essential information:

```
┌─────────────────────────────────────┐
│ ◊ Agent Start                     │
│ Agent: abc123                     │
│ User: admin (Operator)              │
│ Commands: /help (for commands)      │
└─────────────────────────────────────┘
```

**Agent Commands**:

- `/help` - Show all available commands
- `/status` - Display agent status with visual state indicators
- `/timeout <s>` - Change agent timeout (1-3600 seconds)
- `/name <name>` - Change agent name (alphanumeric and hyphens)
- `/detach` or `/d` - Detach from agent (keeps agent running)
- `/quit` or `/q` - End the agent completely

### Implementation

**Display Utility** (`cli/lib/display.js`):

- Centralized icon definitions and display functions
- `showCommandBox()` - Creates standardized command boxes
- `success()`, `error()`, `warning()`, `info()` - Consistent status messaging
- `icons` object - All flat icon mappings

**Usage Pattern**:

```javascript
const display = require('../lib/display');

// Show command box
display.showCommandBox(`${display.icons.start} Start Services`, {
  operation: 'Start all Raworc services'
});

// Status messages
display.success('Services started successfully');
display.error('Failed to connect to Docker');
display.info('Checking system status...');
display.warning('Some containers may not be running');
```

**No Spinners or Animations**: All commands use simple, immediate feedback without loading spinners or animations for clean terminal output and better accessibility.

### Visual Consistency Guidelines

1. **Always use flat icons** from the design system instead of emojis
2. **Consistent command boxes** for all operations showing context
3. **Immediate feedback** with success/error/info/warning status messages
4. **Professional terminal appearance** suitable for developer workflows
5. **Agent state indicators** use geometric shapes for clear status communication
6. **Help-first approach** - commands show `/help (for commands)` to guide users

## Error Handling Philosophy

**Agent Error Handling**: Raworc uses a comprehensive error handling system that ensures agents never fail silently:

- **No Error State**: The system does NOT use an "errored" state. Agents use a clean 5-state model: `init → idle → busy → slept → deleted`
- **Top-Level Error Catching**: Agent host catches ALL errors at the highest level and sends them as messages to users instead of crashing
- **Automatic Recovery**: Agents automatically restart on crashes with exponential backoff, ensuring continuous operation
- **Operator Health Monitoring**: The operator continuously monitors non-sleeping agents and marks unreachable containers as "slept" for recovery
- **Message-Based Error Reporting**: All errors are communicated to users through the message system, maintaining transparency while ensuring system reliability

This approach ensures agents remain operational and users are informed of any issues without system-level failures.
