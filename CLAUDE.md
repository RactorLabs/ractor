<div align="center">
  <img src="assets/logo.png" alt="Raworc Logo" width="200"/>
  
  # Raworc Project Documentation for Claude
  
  **Universal AI Agent Runtime**
  
  *AI assistant instructions for understanding and working with the Raworc codebase*
  
  [![Website](https://img.shields.io/badge/Website-raworc.com-blue?style=for-the-badge)](https://raworc.com)
  [![Twitter](https://img.shields.io/badge/Twitter-@raworc-1DA1F2?style=for-the-badge&logo=twitter&logoColor=white)](https://twitter.com/raworc)
  
</div>

## Project Overview for Claude

When working with this codebase, understand that Raworc is a Universal AI Agent Runtime that deploys AI agents from any framework in containerized environments. It supports Python, Node.js, and Rust agents with full computer access.

**Repository Status**: This repository is private and intentionally not licensed. Do not add or suggest adding license files.

**Related Repository**: The `raworc-community` repository is for consumers of the Docker containers published from this repo. Developers will use the `raworc` CLI on the community repo and use the product without the source code - only the binaries.

**CLI Usage**: The `raworc` CLI is now published as an npm package (`@raworc/cli`) for public use. This simplifies the developer experience by providing a single installation command.

**What makes Raworc unique:**
- **Framework-Agnostic**: Deploy agents from any framework (LangChain, CrewAI, AutoGen, LangGraph, custom implementations)
- **Language Support**: Python, Node.js, and Rust runtime environments
- **Full Computer Access**: Filesystem, web browsing, system tools, and code execution
- **Multi-Agent Support**: Multiple agents can work together in shared environments
- **Session Persistence**: Pause, save state, and resume complex workflows
- **Secure by Design**: JWT auth, role-based access, encrypted secrets, space isolation
- **No Lock-in**: Works with any AI provider or framework

## Supported Frameworks

- **LangChain**: Full support for LangChain agents and chains
- **CrewAI**: Multi-agent orchestration and collaboration
- **AutoGen**: Conversational AI agent framework
- **LangGraph**: State machine-based agent workflows
- **Custom Implementations**: Build agents with any Python, Node.js, or Rust libraries

## Tech Stack

- **Language**: Rust
- **Database**: MySQL 8.0
- **Authentication**: JWT-based with RBAC
- **Container Runtime**: Docker
- **Orchestration**: Multi-container session management

## Architecture

Like Kubernetes, Raworc uses a control plane and worker nodes pattern for AI agent orchestration:

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

### Components

- **CLI**: kubectl-like interface for managing AI agent deployments
- **API Server**: Kubernetes-style API server for agent session management
- **Operator**: Controller that watches desired state and manages agent containers
- **Agent Containers**: Isolated compute environments where AI agents execute tasks
- **Database**: etcd-like persistent storage for session state and configuration

## Why Raworc

Stop fighting infrastructure. Start building agents.

- **Prototype Fast**: Use any AI framework without setup headaches
- **Experiment Safe**: Isolated containers for safe testing
- **Save Progress**: Pause and resume long-running workflows  
- **Scale Easy**: Go from prototype to production without rewrites
- **Work Together**: Share workspaces with your team
- **Stay Flexible**: Switch frameworks and providers anytime

## Features

- **Computer Access**: Agents can manage files, browse web, run code, and use system tools
- **Multi-Agent Support**: Multiple agents work together in shared containers
- **Isolated Sessions**: Each session runs in a clean, secure container
- **Full Capabilities**: File I/O, web scraping, code generation, compilation, system admin
- **Session Persistence**: Pause and resume complex workflows
- **Space Organization**: Separate projects by team, environment, or use case  
- **Secret Management**: Secure API keys and config per space
- **Resource Controls**: Set CPU, memory, storage, and network limits
- **Production Ready**: RBAC, audit trails, and space isolation

## Two Ways to Use Raworc

### 🚀 End Users (Published Packages)

**For using Raworc in production or testing the system:**

```bash
# Install CLI from npm
npm install -g @raworc/cli

# Start services (automatically pulls Docker images from Docker Hub)
raworc start

# Use the system
raworc session
raworc api health
```

**Key Points:**
- Uses published Docker images from Docker Hub (`raworc/raworc_server`, etc.)
- No building required - everything is pre-built
- Install via npm package manager
- Simple, one-command setup
- No access to build/development commands

### 🛠️ Contributors (This Repository)

**For developing, contributing, or customizing Raworc:**

```bash
# Clone this repository
git clone <this-repo>
cd raworc

# Build images locally
./scripts/build.sh

# Start services with local images
./scripts/start.sh

# Link CLI for development
./scripts/link.sh
raworc session  # Now uses your local build
```

**Key Points:**
- Builds Docker images locally from source
- Full access to build, modify, and test changes
- Uses shell scripts for container management
- Can modify source code and rebuild
- Publishing capabilities to Docker Hub

---

## Working with This Codebase

### Development Prerequisites

- Docker must be installed (Docker Compose not required)
- Rust toolchain is required for building from source
- Node.js 16+ for the published CLI package

### Development vs End User Usage

**For Development (this repository):**
- Use `./scripts/*.sh` for local development with locally built images
- Use linked CLI via `./scripts/link.sh` or `cargo run`
- Build images locally with `./scripts/build.sh`
- All images are built locally (no registry dependency)

**For End Users (published packages):**
- Install CLI via npm: `npm install -g @raworc/cli`
- Uses published Docker images from Docker Hub automatically
- No building required - images are pulled as needed
- Simple `raworc start` command handles everything

### Building the Project

To build this project:

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

### Publishing (Contributors Only)

**Publishing Docker Images to Registry:**

```bash
# Build local images first
./scripts/build.sh

# Push to Docker Hub (requires Docker Hub login)
./scripts/push.sh --registry raworc --tag 1.0.0
./scripts/push.sh  # Uses defaults: raworc/latest
```

**Publishing CLI to npm:**

```bash
# Publish CLI package (requires npm login)
./scripts/publish.sh
./scripts/publish.sh --tag beta
```

**Publishing Workflow:**
1. Build and test locally with `./scripts/build.sh` and `./scripts/start.sh`
2. Push Docker images with `./scripts/push.sh`
3. Publish CLI with `./scripts/publish.sh`
4. End users can then `npm install -g @raworc/cli` and `raworc start`

### Testing the System (Development)

To test the system after making changes:

```bash
# 1. Link CLI for development (REQUIRED FIRST STEP)
./scripts/link.sh

# 2. Build Docker images locally
./scripts/build.sh

# 3. Start services with local images
./scripts/start.sh --build

# 4. Run integration tests
cargo test --test integration

# 5. Check service health (using linked CLI - NEVER node index.js)
raworc api health

# 6. Test CLI functionality
raworc start --restart
raworc session
raworc stop --cleanup

# 7. View logs for debugging
docker logs raworc_server --tail 50
docker logs raworc_operator --tail 50
```

### Testing Published Packages (End User)

```bash
# Install CLI
npm install -g @raworc/cli

# Start services (pulls published images automatically)
raworc start

# Check health
raworc api health
```

## Key Commands 

### Published CLI Commands (End Users)

```bash
raworc [COMMAND]

Commands:
  start     Start services (pulls published Docker images)
  stop      Stop services
  reset     Clean up everything: stop services, remove containers, and prune Docker
  cleanup   Clean up all live sessions
  session   Start an interactive session for testing messages
  auth      Authentication management
  api       Execute API requests using saved authentication
  help      Print this message or the help of the given subcommand(s)
```

### Development Scripts (Contributors)

```bash
./scripts/build.sh     # Build Docker images locally
./scripts/start.sh     # Start services with local images
./scripts/stop.sh      # Stop services
./scripts/restart.sh   # Restart services
./scripts/reset.sh     # Complete cleanup
./scripts/link.sh      # Link CLI for development
./scripts/push.sh      # Push images to registry (publishing)
```

**Key Commands:**
- `raworc` - Shows help (default behavior)
- `raworc start` - Start orchestration services  
- `raworc auth` - Show authentication status
- `raworc auth login` - Authenticate with server
- `raworc session` - Interactive AI messaging
- `raworc api` - Direct REST API access

**New Workflow:**
The CLI has been updated with a streamlined interface:
- **Intuitive auth**: `raworc auth` shows status, `raworc auth login --token <token>` to authenticate
- **Direct messaging**: `raworc session` starts a session where you can type messages directly
- **Standalone API**: `raworc api <endpoint>` (GET) or `raworc api <endpoint>` works from any terminal with saved auth
- **Professional interface**: Removed "playground" terminology for production usage

## CLI Usage

### Published CLI (End Users)

Install the published CLI for production use:

```bash
# Install from npm
npm install -g @raworc/cli

# Start services (automatically pulls published Docker images)
raworc start

# Stop services
raworc stop
raworc stop --cleanup    # Stop and clean session containers

# Build is not available - uses published images
# raworc build  # NOT AVAILABLE - for development only

# Restart services
raworc restart

# Complete reset
raworc reset --yes
```

### Development Scripts (Contributors)

Use shell scripts for development with locally built images:

```bash
# Build images locally first
./scripts/build.sh
./scripts/build.sh --no-cache  # Clean build

# Start services with local images
./scripts/start.sh
./scripts/start.sh --build     # Build and start
./scripts/start.sh mysql server # Start specific services

# Stop services
./scripts/stop.sh
./scripts/stop.sh --cleanup     # Stop and clean session containers
./scripts/stop.sh --remove      # Remove containers entirely

# Restart services
./scripts/restart.sh
./scripts/restart.sh --build --cleanup

# Complete reset
./scripts/reset.sh --yes

# Link CLI for development (use local build instead of npm)
./scripts/link.sh
```

**Key Differences:**
- **Published CLI**: Uses Docker Hub images, no build command, install via npm
- **Development Scripts**: Uses locally built images, includes build command, run from repo


### Authentication

```bash
# Check authentication status
raworc auth

# Token-based authentication
raworc auth login --token sk-ant-api03-your-token

# User/pass authentication  
raworc auth login --user admin --pass admin

# Custom server (default: http://localhost:9000)
raworc auth login --server https://raworc.example.com --token sk-ant-api03-your-token
```

**Authentication Commands:**
- **`raworc auth`**: Show current authentication status (default behavior)
- **`raworc auth login`**: Authenticate with server using token or credentials
- **Token**: Direct JWT token authentication (recommended for automation)
- **Credentials**: User/pass authentication (gets JWT token)
- **Server**: Defaults to `http://localhost:9000` for development

All login commands are non-interactive and suitable for scripting.

### AI Agent Session Management

Use `raworc session` for interactive messaging with AI agents:

```bash
# Start session in default space
raworc session

# Start session in specific space  
raworc session --space production

# In session interface:
You: Hello, how are you?
⠋ Waiting for agent response...
Agent: Hello! I'm doing well, thank you for asking...

You: What can you help me with?
⠋ Waiting for agent response...
Agent: I can help you with various tasks like coding, analysis, and more!

You: /quit
👋 Ending session...
```

**Chat Interface:**
- **Synchronous flow**: Send message → wait for response → send next message
- **Clean prompts**: Simple "You:" prompt for all input
- **Response waiting**: Shows spinner while waiting for agent response (60s timeout)
- **Turn-based**: Cannot send next message until agent responds
- **Clean format**: "You:" for input, "Agent:" for responses
- **Session cleanup**: Sessions are automatically deleted when you quit (via /quit, /q, or /exit)
- **Commands**: /status (show info), /quit (end session)

### API Command

Use `raworc api` for direct REST API access with saved authentication:

```bash
# GET requests
raworc api health                  # Check server health
raworc api sessions                # List sessions
raworc api spaces              # List spaces

# POST requests with JSON body
raworc api sessions --method POST --body '{"space":"default"}'
raworc api spaces/default/secrets --method POST --body '{"key_name":"API_KEY","value":"secret"}'

# Other HTTP methods
raworc api spaces/staging --method PUT --body '{"description":"Updated space"}'
raworc api sessions/session-123 --method DELETE

# Show response headers and pretty print JSON
raworc api sessions --headers --pretty
```

**API Command Features:**
- **Authentication status**: Shows auth status before each request
- **Auto-formatting**: Pretty prints JSON responses by default
- **Headers**: Optional response header display
- **All methods**: GET (default), POST, PUT, DELETE, PATCH supported via --method flag
- **Uses saved auth**: Automatically uses token from `raworc auth login`

### Agent Session Operations

```bash
# Create new AI agent session
raworc api sessions --method POST --body '{"space":"default"}'

# List all agent sessions  
raworc api sessions

# Pause agent session (saves resources)
raworc api sessions/{id}/pause

# Resume agent session
raworc api sessions/{id}/resume

# Terminate agent session
raworc api sessions/{id}
```

### Sending Messages to Agents

**Interactive Method (Recommended):**
```bash
# Start session and send messages directly
raworc session --space default
# Then just type: Hello, generate a Python script to calculate fibonacci numbers
```

**API Method:**
```bash
# Send a message to the agent in a session
raworc api sessions/{session-id}/messages --method POST --body '{"content":"Generate a Python script to calculate fibonacci numbers"}'

# Check for agent responses
raworc api sessions/{session-id}/messages

# View latest messages (limit to last 10)
raworc api sessions/{session-id}/messages?limit=10
```

The agent will receive your message, process it using AI capabilities (code generation, file operations, web browsing), and respond with results. In the interactive session interface, agent responses appear automatically in real-time as "Assistant:" messages. Messages are persistent and viewable through the API.

### Session Operations via API

Sessions support the following operations:
- **Create**: Creates new session and spawns container
- **Pause**: Pauses session and stops container to save resources
- **Resume**: Resumes paused session and restarts container
- **Delete**: Soft deletes session and destroys container

### Space Management

```bash
# List spaces
raworc api spaces

# Get specific space  
raworc api spaces/default

# Create space (admin only)
raworc api spaces --method POST --body '{"name":"staging","description":"Staging space","settings":{"environment":"staging"}}'

# Update space (admin only)
raworc api spaces/staging --method PUT --body '{"name":"updated-staging","description":"Updated staging space"}'

# Delete space (admin only, cannot delete 'default')
raworc api spaces/staging --method DELETE
```

### Space Secrets Management

```bash
# List secrets (metadata only)
raworc api spaces/default/secrets

# List secrets with values (requires read-values permission)
raworc api spaces/default/secrets?show_values=true

# Get specific secret
raworc api spaces/default/secrets/ANTHROPIC_API_KEY

# Get secret with value
raworc api spaces/default/secrets/ANTHROPIC_API_KEY?show_values=true

# Create new secret
raworc api spaces/default/secrets --method POST --body '{"key_name":"API_KEY","value":"secret-value","description":"API key"}'

# Update secret value
raworc api spaces/default/secrets/API_KEY --method PUT --body '{"value":"new-secret-value"}'

# Update secret description
raworc api spaces/default/secrets/API_KEY --method PUT --body '{"description":"Updated description"}'

# Delete secret
raworc api spaces/default/secrets/API_KEY --method DELETE
```

### Agent Management

Raworc supports deploying specialized AI agents that are compiled during space builds and executed in session containers. Agents are built from GitHub repositories using a simple manifest file (`raworc.json`). No Raworc dependencies required - just implement a function!

```bash
# List agents in space
raworc api spaces/default/agents

# Create agent (triggers automatic space rebuild)
raworc api spaces/default/agents --method POST --body '{
  "name": "data-analyzer",
  "description": "Data analysis specialist", 
  "purpose": "analyze data, create visualizations, statistical analysis",
  "source_repo": "Raworc/raworc-agent-python-demo",
  "source_branch": "main"
}'

# Manually trigger space build after adding agents
raworc api spaces/default/build --method POST

# Check agent deployment in sessions  
raworc api sessions --method POST --body '{"space": "default"}'  # Uses pre-built agents

# Check agent logs in session containers
# (Logs are captured in /session/logs/{agent}_{timestamp}_{stdout|stderr}.log)
```

#### Creating Agents

**Agent Repository Structure:**
```
my-agent/
├── raworc.json         # Manifest (required)
├── requirements.txt    # Dependencies (Python)
├── package.json        # Dependencies (Node.js)
├── Cargo.toml         # Dependencies (Rust)
└── main.py            # Your agent code
```

**raworc.json Manifest:**
```json
{
  "runtime": "python3",
  "handler": "main.process_message",
  "build_command": "pip install additional-package"
}
```

**Key Features:**
- **Auto-detection**: Raworc automatically detects Python, Node.js, or Rust projects
- **Automatic Building**: `pip install`, `npm install`, `cargo build` run automatically
- **Optional build_command**: For additional setup steps only
- **Pre-compilation**: All building happens at space build time, not runtime
- **Fast Execution**: Session containers use pre-built agents for instant startup

**Agent Code (Zero Raworc Dependencies):**
```python
def process_message(message: str, context: dict) -> str:
    # Use any framework: Langchain, CrewAI, custom logic
    return "Your agent response here"
```

**Supported Runtimes:**
- **Python**: Auto-creates virtual environment, installs requirements.txt
- **Node.js**: Auto-runs npm install for package.json
- **Rust**: Auto-runs cargo build --release for Cargo.toml

**Demo Repositories:**
- **Python**: https://github.com/Raworc/raworc-agent-python-demo
- **Node.js**: https://github.com/Raworc/raworc-agent-js-demo  
- **Rust**: https://github.com/Raworc/raworc-agent-rust-demo

#### Agent Logging

Agent execution logs are captured with unique timestamps:

```bash
# View agent logs in session containers
docker exec raworc_session_{session_id} ls -la /session/logs/

# Example log files:
# file-agent_20250820_143052_123_stdout.log
# file-agent_20250820_143052_123_stderr.log

# View specific execution logs
docker exec raworc_session_{session_id} cat /session/logs/file-agent_20250820_143052_123_stdout.log
```

### Authentication

```bash
# Get admin token
raworc api auth/login --method POST --body '{"user":"admin","pass":"admin"}'

# Get operator token
raworc api auth/login --method POST --body '{"user":"operator","pass":"admin"}'

# Check current authentication
raworc api auth/me
```

### Common Use Cases

#### Setting Up Environment Secrets
```bash
# Set up API keys for default space
raworc api spaces/default/secrets --method POST --body '{"key_name":"ANTHROPIC_API_KEY","value":"sk-ant-api03-xxx","description":"Claude API key"}'
raworc api spaces/default/secrets --method POST --body '{"key_name":"OPENAI_API_KEY","value":"sk-xxx","description":"OpenAI API key"}'
raworc api spaces/default/secrets --method POST --body '{"key_name":"DATABASE_URL","value":"mysql://user:pass@host/db","description":"Database connection"}'
```

#### Multi-Environment Setup
```bash
# Create staging space (admin only)
raworc api spaces --method POST --body '{"name":"staging","description":"Testing environment","settings":{"environment":"staging"}}'

# Set staging-specific secrets
raworc api spaces/staging/secrets --method POST --body '{"key_name":"API_KEY","value":"staging-key","description":"Staging API key"}'
raworc api spaces/staging/secrets --method POST --body '{"key_name":"DATABASE_URL","value":"mysql://test:test@staging-db/app"}'

# Set production secrets
raworc api spaces/production/secrets --method POST --body '{"key_name":"API_KEY","value":"prod-key","description":"Production API key"}'
raworc api spaces/production/secrets --method POST --body '{"key_name":"DATABASE_URL","value":"mysql://prod:xxx@prod-db/app"}'
```

#### Secret Rotation
```bash
# Check current secret
raworc api spaces/default/secrets/API_KEY?show_values=true

# Update with new rotated key
raworc api spaces/default/secrets/API_KEY --method PUT --body '{"value":"new-rotated-key"}'

# Verify update
raworc api spaces/default/secrets/API_KEY
```

## Troubleshooting

### Services won't start
```bash
# Check if ports are in use
lsof -i :9000
lsof -i :3307

# For published CLI users:
raworc restart --cleanup
raworc stop --cleanup
docker system prune -f
raworc start

# For developers:
./scripts/restart.sh --cleanup
./scripts/stop.sh --cleanup
docker system prune -f
./scripts/start.sh --build
```

### Database connection issues
```bash
# Check MySQL is healthy
docker exec raworc_mysql mysql -u raworc -praworc -e "SELECT 1"

# Check migrations
docker logs raworc_server | grep migration
```

### Build failures (Development Only)
```bash
# Clean build (development only - published CLI has no build command)
cargo clean
./scripts/build.sh --no-cache

# Published CLI users don't build - images are pre-built
```

### Session container issues
```bash
# List all session containers
docker ps -a --filter "name=raworc_session_"

# Clean up all sessions (API + containers)
raworc cleanup --yes      # Delete all sessions from API
raworc stop --cleanup     # Clean up remaining containers

# For published CLI users:
raworc start --restart    # Restart with container cleanup
raworc reset --yes        # Complete system reset

# For developers:
./scripts/restart.sh --cleanup
./scripts/stop.sh --cleanup
./scripts/reset.sh --yes

# Manual reset of specific session (both)
docker rm -f raworc_session_{session-id}

# Check session container logs (both)
docker logs raworc_session_{session-id} --tail 50

# Check agent logs inside session (both)
docker exec raworc_session_{session-id} ls -la /session/logs/
docker exec raworc_session_{session-id} cat /session/logs/{agent}_{timestamp}_stderr.log
```

### Space build issues
```bash
# Check space build status
raworc api spaces/default/build/latest

# View build logs
docker logs raworc_operator --tail 100

# Rebuild space
raworc api spaces/default/build --method POST

# Check operator is running
docker ps --filter "name=raworc_operator"

# Debug build failures
docker logs raworc_operator | grep -A 10 -B 10 "ERROR"

# Check available space images
docker images | grep raworc_space_

# Remove broken space images
docker rmi $(docker images -q raworc_space_*)
```

### CLI not found or issues
```bash
# If raworc command is not found in development
# NEVER use node index.js - always link first
./scripts/link.sh
raworc --help  # Now this should work

# If still having issues, check symlink
which raworc   # Should show /usr/local/bin/raworc
ls -la /usr/local/bin/raworc  # Should point to your project

# For published CLI users
npm install -g @raworc/cli
raworc --help
```

## Development Notes for Claude

### **🚨 CRITICAL CLI Usage Rule**

**❌ NEVER DO THIS:**
```bash
node index.js start
cd cli && node index.js --help
node cli/index.js api health
```

**✅ ALWAYS DO THIS:**
```bash
# First, link the CLI for development
./scripts/link.sh

# Then use the raworc command
raworc start
raworc --help 
raworc api health
```

**Why?**
- The linked `raworc` command uses the correct paths and configuration
- `node index.js` runs from wrong directory context and may fail
- Linked command matches production behavior exactly
- Avoids path resolution and module loading issues
- Required for proper testing of CLI functionality

### Project Structure

The codebase is organized as follows:

```
raworc/
├── src/           # Rust source code
│   ├── cli/       # CLI command implementations
│   ├── api/       # API server code
│   ├── operator/  # Operator service
│   └── host/      # Host agent for containers
├── migrations/    # Database migrations
├── docker/        # Docker configurations
└── tests/         # Integration tests
```

### Key Files to Understand

- `src/main.rs` - CLI entry point
- `src/api/server.rs` - API server implementation
- `src/operator/main.rs` - Operator service
- `src/host/agent.rs` - Container host agent
- `docker-compose.yml` - Service orchestration

### Code Patterns and Conventions

- **Error Handling**: Use `Result<T, E>` types with custom error types
- **Async Code**: Uses Tokio runtime for async operations
- **Database**: MySQL with sqlx for queries
- **API**: RESTful JSON API with JWT authentication
- **Testing**: Unit tests in modules, integration tests in `tests/`

### Testing Guidelines

When modifying code:

1. **Run unit tests**: `cargo test --lib`
2. **Run integration tests**: `cargo test --test integration`
3. **Check formatting**: `cargo fmt --check`
4. **Run linter**: `cargo clippy`
5. **Build release**: `cargo build --release`

### Common Development Tasks

- **Add new CLI command**: Modify `src/cli/mod.rs`
- **Add API endpoint**: Update `src/api/routes.rs`
- **Modify database schema**: Add migration in `migrations/`
- **Update Docker setup**: Edit `Dockerfile.*` files or shell scripts

### Important Implementation Details

- **Authentication**: JWT tokens with role-based permissions
- **Database**: MySQL 8.0 with automatic migrations
- **Container Management**: Docker API for lifecycle control
- **Session State**: Persistent volumes for data retention
- **Agent Communication**: JSON-RPC over stdio
- **Build System**: Multi-stage Docker builds for optimization

## Debugging Tips

- Check server logs: `docker logs raworc_server -f`
- Check operator logs: `docker logs raworc_operator -f`
- Database queries: `docker exec raworc_mysql mysql -u raworc -praworc raworc`
- Session containers: `docker ps -a --filter "name=raworc_session_"`
- Clean restart: `raworc start --restart` (linked CLI) or `./scripts/restart.sh` (development)
- **Remember**: Always use linked `raworc` command, never `node index.js`

## Version

Current version: 0.2.2

## Notes for Claude

This document is specifically formatted to help you understand and work with the Raworc codebase. Focus on:
1. Understanding the architecture before making changes
2. Following existing code patterns and conventions
3. Running tests after modifications
4. Using proper error handling and logging
5. Maintaining backward compatibility

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
- Always use the development scripts for local development:
  - Use `./scripts/build.sh` instead of `cargo build` or `docker build`
  - Use `./scripts/start.sh` instead of manual `docker run` commands
  - Use `./scripts/restart.sh` instead of manual restart sequences
  - **CRITICAL**: Link CLI with `./scripts/link.sh` then use `raworc` command
  - **NEVER use `node index.js` or `node cli/index.js`** - always use the linked `raworc` command
- Published CLI users should use `raworc` commands directly (installed via npm)

### Branch Naming
Use descriptive branch names:
```
type/brief-description
```

Examples:
- `feat/session-pause-resume`
- `fix/container-cleanup-race`
- `docs/api-reference-update`
