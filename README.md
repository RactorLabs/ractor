<div align="center">
  <img src="assets/logo.png" alt="Raworc Logo" width="200"/>
  
  # Raworc
  
  **Remote Agentic Work Orchestrator**
  
  Computer use agents with dedicated computers to automate manual work. Intelligent agents that use computers like humans do to perform any task.
  
  [![Website](https://img.shields.io/badge/Website-raworc.com-blue?style=for-the-badge)](https://raworc.com)
  [![Version](https://img.shields.io/badge/Version-0.3.0-green?style=for-the-badge)](https://github.com/SivaRagavan/raworc/releases)
  [![License](https://img.shields.io/badge/License-Proprietary-red?style=for-the-badge)](LICENSE)
</div>

## What is Raworc?

Raworc is a **Remote Agentic Work Orchestrator** that provides Computer use agents with dedicated computers for each session. Intelligent agents that use computers like humans do - with natural language interfaces, full software access, and the ability to perform any computer-based task.

### Key Features

- 🖥️ **Computer Use Agents** - Each session provides Computer use agents with dedicated computers
- 🗣️ **Natural Language Control** - Control computers through conversation, no APIs or SDKs required  
- 🔧 **Complete Automation** - Web browsing, file operations, software installation, system administration
- 🔄 **Persistent Sessions** - Close, save state, and restore long-running automation workflows
- 🏢 **Enterprise Ready** - RBAC, audit trails, session isolation, encrypted secret management
- 🐳 **Scalable Infrastructure** - Deploy multiple Computer use agents with dedicated computers for reliable automation orchestration

## Development Setup

**This repository is for local development and contributing to Raworc.**

```bash
# Clone and setup
git clone <this-repo>
cd raworc

# Link CLI for development  
./scripts/link.sh

# Build and start services
./scripts/build.sh
./scripts/start.sh

# Test changes (requires ANTHROPIC_API_KEY)
export ANTHROPIC_API_KEY=sk-ant-api03-your-key
raworc auth login --user admin --pass admin
raworc session
```

## CLI Reference

The Raworc CLI provides complete control over the orchestrator. Install globally via npm or use for development.

### Installation

**End Users (Production)**
```bash
npm install -g @raworc/cli
raworc start
raworc login -u admin -p admin
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
| `raworc stop` | Stop Docker services | `raworc stop -y` |
| `raworc clean` | Clean containers and images | `raworc clean --all` |
| `raworc reset` | **Nuclear option**: Clean everything | `raworc reset -y` |
| `raworc pull` | Update CLI and Docker images | `raworc pull` |

### Session Management

| Command | Description | Example |
|---------|-------------|---------|
| `raworc session` | Start interactive session | `raworc session` |
| `raworc session -r <id>` | Restore session | `raworc session -r abc123` |
| `raworc session -R <id>` | Remix session | `raworc session -R abc123` |
| `raworc session -S <json>` | Session with secrets | `raworc session -S '{"API_KEY":"value"}'` |

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
- `raworc login -u/--user -p/--pass -s/--server`
- `raworc auth -t/--token -s/--server`
- `raworc token -p/--principal -t/--type`

**Service Management:**
- `raworc start -r/--restart`
- `raworc stop -y/--yes`
- `raworc clean -y/--yes -a/--all`
- `raworc reset -y/--yes -s/--services-only`
- `raworc pull -c/--cli-only -i/--images-only`

**Session Management:**
- `raworc session -r/--restore -R/--remix -d/--data -c/--code`
- `raworc session -S/--secrets -i/--instructions -if/--instructions-file`
- `raworc session -s/--setup -sf/--setup-file -p/--prompt`

**API Access:**
- `raworc api -m/--method -b/--body -H/--headers -p/--pretty -s/--status`

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
  "principal_type": "User"
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
    "API_KEY": "sk-123"
  }
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

# Restore session  
POST /sessions/{id}/restore
Authorization: Bearer <jwt-token>

# Remix session
POST /sessions/{id}/remix
Authorization: Bearer <jwt-token>
Content-Type: application/json

{
  "data": true,
  "code": false
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
  "version": "0.3.0",
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

### Authentication

All protected endpoints require a Bearer token in the Authorization header:

```bash
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

Obtain tokens through:
1. **Operator Login**: `POST /operators/{name}/login`
2. **Token Creation**: `POST /auth/token` (requires existing auth)

## Architecture

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

## Major Updates

### 🏗️ **Remote Computer Use Focus (v0.3.0)**
- **Computer use agents with dedicated computers** - Each session provides Computer use agents with dedicated computers
- **Manual work automation** - Computer use agents automate tasks using natural computer interfaces  
- **Direct Claude integration** - Claude API support for computer-use capabilities
- **Required API key** - ANTHROPIC_API_KEY validation ensures automation sessions work immediately
- **Session-based workflow** - Simplified direct computer access with intelligent automation

### 🔄 **Session Persistence & Restore (v0.2.7-0.2.8)**
- **Session close/restore** - Close sessions and restore with full state preservation
- **Message loop reliability** - Fixed critical bugs preventing second messages from processing  
- **No reprocessing** - Restored sessions only process new messages, not old ones
- **Improved CLI** - Better session handling with proper state constants

### 🚀 **Enhanced CLI Experience (v0.2.5-0.2.6)**  
- **Pull command** - Update CLI and Docker images: `raworc pull`
- **Streamlined auth** - Simple authentication: `raworc auth login --token <token>`
- **Interactive sessions** - Direct messaging: `raworc session`
- **API access** - Direct REST calls: `raworc api <endpoint>`

## Development

### Prerequisites

- **Rust toolchain** - For building backend services
- **Node.js 16+** - For CLI development  
- **Docker** - For container orchestration
- **MySQL** - Database (auto-managed in Docker)

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

### Testing

```bash
# Full integration test (requires ANTHROPIC_API_KEY)
export ANTHROPIC_API_KEY=sk-ant-api03-your-key
./scripts/build.sh
./scripts/start.sh
raworc auth login --user admin --pass admin
raworc session
```

### Publishing

This repository publishes to:
- **Docker Hub** - `raworc/raworc_server`, `raworc/raworc_operator`, `raworc/raworc_host`
- **npm** - `@raworc/cli` package

## Support

- 📖 **Documentation** - [raworc.com](https://raworc.com)  
- 🐛 **Issues** - [GitHub Issues](https://github.com/SivaRagavan/raworc/issues)
- 💬 **Community** - [GitHub Discussions](https://github.com/SivaRagavan/raworc/discussions)