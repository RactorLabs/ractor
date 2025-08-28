<div align="center">
  <img src="assets/logo.png" alt="Raworc Logo" width="200"/>
  
  # Raworc
  
  **Universal AI Agent Runtime**
  
  Deploy AI agents from any framework in containerized environments with full computer access.
  
  [![Website](https://img.shields.io/badge/Website-raworc.com-blue?style=for-the-badge)](https://raworc.com)
  [![Version](https://img.shields.io/badge/Version-0.2.8-green?style=for-the-badge)](https://github.com/SivaRagavan/raworc/releases)
  [![License](https://img.shields.io/badge/License-Proprietary-red?style=for-the-badge)](LICENSE)
</div>

## What is Raworc?

Raworc is a **Universal AI Agent Runtime** that lets you deploy AI agents from any framework (LangChain, CrewAI, AutoGen, custom implementations) in secure, containerized environments with full computer access.

### Key Features

- 🚀 **Framework Agnostic** - Works with LangChain, CrewAI, AutoGen, LangGraph, or custom agents
- 🏗️ **Multi-Language** - Deploy Python, Node.js, and Rust agents  
- 💻 **Full Computer Access** - Filesystem, web browsing, code execution, system tools
- 🔄 **Session Persistence** - Close, save state, and restore complex workflows
- 🏢 **Production Ready** - RBAC, audit trails, space isolation, secret management
- 🐳 **Kubernetes-Style** - Control plane architecture for reliable orchestration

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

# Test changes
raworc auth login --user admin --pass admin
raworc session
```

## Architecture

Raworc uses a **Kubernetes-inspired control plane** pattern for AI agent orchestration:

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
                    │          Agent Nodes            │
                    │ ┌─────────────┐ ┌─────────────┐ │
                    │ │ AI Agent    │ │ AI Agent    │ │
                    │ │ Container   │ │ Container   │ │
                    │ └─────────────┘ └─────────────┘ │
                    └─────────────────────────────────┘
```

## Recent Major Updates (v0.2.x)

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

### 🏗️ **Production Infrastructure (v0.2.3-0.2.4)**
- **Automated releases** - Complete GitHub Actions workflow with Docker Hub publishing
- **Space management** - Multi-tenant environments with secret isolation  
- **RBAC system** - Role-based permissions and audit trails
- **Build system** - Comprehensive Docker image management and npm publishing

### 🔧 **Core Improvements (v0.2.0-0.2.2)**
- **Container lifecycle** - Reliable session container management with volume persistence
- **Database consolidation** - Streamlined migrations and schema management  
- **Operator service** - Kubernetes-style container orchestration
- **Shared constants** - Consistent state management across all services

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
│   ├── operator/  # Container orchestration
│   └── host/      # Session agent runtime
├── cli/           # Node.js CLI (@raworc/cli)
├── scripts/       # Development automation
├── website/       # Documentation site (Docusaurus)
├── migrations/    # Database schema
└── docker/        # Docker configurations
```

### Testing

```bash
# Full integration test
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