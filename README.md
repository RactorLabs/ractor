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
                    │ │   Agent +   │ │   Agent +   │ │
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