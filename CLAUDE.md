<div align="center">
  <img src="assets/logo.png" alt="Raworc Logo" width="200"/>
  
  # Raworc Project Documentation for Claude
  
  **Remote Agentic Work Orchestrator**
  
  *AI assistant instructions for understanding and working with the Raworc codebase*
  
  [![Website](https://img.shields.io/badge/Website-raworc.com-blue?style=for-the-badge)](https://raworc.com)
  [![Twitter](https://img.shields.io/badge/Twitter-@raworc-1DA1F2?style=for-the-badge&logo=twitter&logoColor=white)](https://twitter.com/raworc)
  
</div>

## Project Overview for Claude

When working with this codebase, understand that Raworc is a Remote Agentic Work Orchestrator that provides Computer use agents with dedicated computers to automate manual work. Each session gives you Computer use agents with dedicated computers that use computers like humans do - through natural language control and full system access.

**For End Users**: Raworc is available as a published npm package (`@raworc/cli`) that provides a simple installation experience with pre-built Docker images from Docker Hub.

**Repository Status**: This repository is private and intentionally not licensed. Do not add or suggest adding license files.

**CLI Usage**: The `raworc` CLI is now published as an npm package (`@raworc/cli`) for public use. This simplifies the developer experience by providing a single installation command.

**What makes Raworc unique:**
- **Computer Use Agents**: Intelligent agents with dedicated computers ready to automate manual work
- **Natural Language Control**: Control computers through conversation without APIs or complex integrations
- **Full Computer Access**: Filesystem, web browsing, system tools, software installation, and code execution
- **Session-Based Automation**: Each session provides Computer use agents with dedicated computers for automation tasks
- **Session Persistence**: Close, save state, and restore long-running automation workflows
- **Secure by Design**: JWT auth, role-based access, session isolation

## What You Can Automate

- **Web Automation**: Browser tasks, form filling, data extraction, website navigation
- **Document Processing**: File manipulation, report generation, spreadsheet management, data transformation
- **System Administration**: Server management, application deployment, monitoring, DevOps tasks
- **Development Tasks**: Code generation, testing, debugging, deployment automation
- **Data Analysis**: Dataset processing, visualization, statistical analysis, insights generation
- **Custom Workflows**: Any manual computer-based work that can be described in natural language

## Tech Stack

- **Language**: Rust
- **Database**: MySQL 8.0
- **Authentication**: JWT-based with RBAC
- **Container Runtime**: Docker
- **Orchestration**: Multi-container session management

## Architecture

Raworc uses a control plane and worker nodes pattern for session orchestration:

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
                    │       Session Computers         │
                    │ ┌─────────────┐ ┌─────────────┐ │
                    │ │ Computer    │ │ Computer    │ │
                    │ │ + Agent     │ │ + Agent     │ │
                    │ └─────────────┘ └─────────────┘ │
                    └─────────────────────────────────┘
```

### Components

- **CLI**: Interface for managing computer sessions and automation tasks
- **API Server**: Session management and automation orchestration
- **Operator**: Controller that manages session containers and computer environments
- **Session Computers**: Isolated computer environments with built-in Computer Use agents
- **Database**: Persistent storage for session state and configuration

## Why Raworc

Stop doing manual work. Start automating.

- **Instant Automation**: Get a remote computer with built-in Computer Use agent in seconds
- **Safe Automation**: Isolated computer environments for safe automation testing
- **Never Lose Progress**: Close and restore long-running automation workflows  
- **Scale Automation**: Go from single tasks to enterprise automation without rewrites
- **Team Automation**: Share automation sessions with proper access controls
- **Flexible Automation**: Automate any computer-based task through natural language

## Key Capabilities

- **Complete Computer Access**: Built-in agents can manage files, browse web, run software, and use any system tools
- **Session-Based Computers**: Each session provides an isolated computer environment for automation tasks
- **Clean Computer Environments**: Each session starts with a fresh, secure remote computer
- **Full Computer Capabilities**: File I/O, web automation, software installation, system administration
- **Session Persistence**: Close and restore complex automation workflows
- **Natural Language Control**: Control computers through conversation without APIs or SDKs
- **Resource Controls**: Set CPU, memory, storage, and network limits per computer session
- **Production Ready**: RBAC, audit trails, and session isolation for enterprise automation

## Two Ways to Use Raworc

### 🚀 End Users (Published Packages)

**For using Raworc in production or testing the system:**

```bash
# Install CLI from npm 
npm install -g @raworc/cli

# Start services (automatically pulls Docker images from Docker Hub)
raworc start

# Authenticate and use the system
raworc auth login --user admin --pass admin
raworc session
raworc api health
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

### Development Prerequisites

- Docker must be installed (Docker Compose not required)
- Rust toolchain is required for building from source
- Node.js 16+ for the published CLI package

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
# Publish CLI package (Node.js implementation from cli/ folder - requires npm login)
./scripts/publish.sh
./scripts/publish.sh --tag beta
```

**Publishing Workflow:**
1. Build and test locally with shell scripts: `./scripts/build.sh` and `./scripts/start.sh`
2. Push Docker images with shell script: `./scripts/push.sh`
3. Publish CLI (Node.js package from cli/ folder) with shell script: `./scripts/publish.sh`
4. End users can then `npm install -g @raworc/cli` and `raworc start`

### Testing the System (Development)

To test the system after making changes:

```bash
# 1. Link CLI for development (shell script links Node.js CLI from cli/ folder - REQUIRED FIRST STEP)
./scripts/link.sh

# 2. Build Docker images locally (using shell script)
./scripts/build.sh

# 3. Start services with local images (using shell script)
./scripts/start.sh --build

# 4. Run integration tests
cargo test --test integration

# 5. Check service health (using linked CLI - Node.js implementation)
raworc api health

# 6. Test CLI functionality (Node.js CLI linked via shell script)
raworc start --restart
raworc session
raworc stop --cleanup

# 7. View logs for debugging
docker logs raworc_server --tail 50
docker logs raworc_operator --tail 50
```

### Testing Published Packages (End User)

```bash
# Install CLI (Node.js/npm package)
npm install -g @raworc/cli

# Pull latest CLI version and Docker images
raworc pull

# Start services
raworc start

# Check health
raworc api health
```

## Key Commands 

### Published CLI Commands (End Users)

```bash
raworc [COMMAND]

Commands:
  start     Start services
  stop      Stop services
  pull      Pull latest CLI version and Docker images from registries
  reset     Clean up everything: stop services, remove containers, and prune Docker
  cleanup   Clean up all live sessions
  session   Start an interactive session for testing messages
  auth      Authentication management
  api       Execute API requests using saved authentication
  help      Print this message or the help of the given subcommand(s)
```

### Development Scripts (Contributors)

```bash
./scripts/build.sh     # Shell script: Build Docker images locally
./scripts/start.sh     # Shell script: Start services with local images
./scripts/stop.sh      # Shell script: Stop services
./scripts/restart.sh   # Shell script: Restart services
./scripts/reset.sh     # Shell script: Complete cleanup
./scripts/link.sh      # Shell script: Link CLI (Node.js from cli/ folder) for development
./scripts/push.sh      # Shell script: Push images to registry (publishing)
```

**Key Commands:**
- `raworc` - Shows help (default behavior)
- `raworc pull` - Pull latest CLI version and Docker images
- `raworc start` - Start orchestration services  
- `raworc auth` - Show authentication status
- `raworc auth login` - Authenticate with server
- `raworc session` - Interactive AI messaging
- `raworc api` - Direct REST API access

**New Workflow:**
The CLI (Node.js implementation from cli/ folder) has been updated with a streamlined interface:
- **Intuitive auth**: `raworc auth` shows status, `raworc auth login --token <token>` to authenticate
- **Direct messaging**: `raworc session` starts a session where you can type messages directly
- **Standalone API**: `raworc api <endpoint>` (GET) or `raworc api <endpoint>` works from any terminal with saved auth
- **Professional interface**: Removed "playground" terminology for production usage

## CLI Usage

### Published CLI (End Users)

Install the published CLI (Node.js/npm package) for production use:

```bash
# Install from npm
npm install -g @raworc/cli

# Pull latest CLI version and Docker images
raworc pull

# Start services
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

Use shell scripts (scripts/ folder) for development with locally built images:

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

# Link CLI for development (shell script links Node.js CLI from cli/ folder instead of npm)
./scripts/link.sh
```

**Key Differences:**
- **Published CLI**: Uses Docker Hub images, no build command, install Node.js package via npm
- **Development Scripts**: Uses locally built images, includes build command, run shell scripts from repo


### Pull Command

Use `raworc pull` to update the CLI and Docker images:

```bash
# Pull both CLI and Docker images (default)
raworc pull

# Only update the CLI, skip Docker images
raworc pull --cli-only

# Only pull Docker images, skip CLI update
raworc pull --images-only
```

**Pull Command Features:**
- **CLI Update**: Updates the @raworc/cli npm package to latest version
- **Docker Images**: Pulls latest Docker images from Docker Hub
- **Selective Updates**: Use flags to update only CLI or only images
- **Error Handling**: Continues with available updates if one component fails

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

### Computer Session Management

Use `raworc session` for interactive automation with your remote computer:

```bash
# Start a new computer session
raworc session

# In session interface:
You: Hello, can you help me automate some tasks?
⠋ Waiting for agent response...
Agent: Hello! I'm ready to help automate your work. I have full access to this computer and can help with file management, web tasks, development work, and more. What would you like to automate?

You: Please create a Python script to process CSV files in the current directory
⠋ Waiting for agent response...
Agent: I'll create a Python script to process CSV files. Let me check what's in the current directory and create an appropriate script for you...

You: /quit
👋 Ending session...
```

**Session Interface:**
- **Conversational Control**: Send natural language requests to control the computer
- **Clean Interface**: Simple "You:" prompt for requests, "Agent:" for computer responses
- **Real-time Feedback**: Shows spinner while the agent works on your computer (60s timeout)
- **Turn-based**: Send request → agent performs work → send next request
- **Session Cleanup**: Computer sessions are automatically cleaned up when you quit (via /quit, /q, or /exit)
- **Session Commands**: /status (show session info), /quit (end session)

### API Command

Use `raworc api` for direct REST API access with saved authentication:

```bash
# GET requests
raworc api health                  # Check server health
raworc api sessions                # List computer sessions

# POST requests with JSON body
raworc api sessions --method POST # Create new computer session

# Other HTTP methods
raworc api sessions/session-123 --method DELETE # Delete computer session

# Show response headers and pretty print JSON
raworc api sessions --headers --pretty
```

**API Command Features:**
- **Authentication status**: Shows auth status before each request
- **Auto-formatting**: Pretty prints JSON responses by default
- **Headers**: Optional response header display
- **All methods**: GET (default), POST, PUT, DELETE, PATCH supported via --method flag
- **Uses saved auth**: Automatically uses token from `raworc auth login`

### Computer Session Operations

```bash
# Create new computer session
raworc api sessions --method POST

# List all computer sessions  
raworc api sessions

# Close computer session (saves resources)
raworc api sessions/{id}/close

# Restore computer session
raworc api sessions/{id}/restore

# Terminate computer session
raworc api sessions/{id} --method DELETE
```

### Automating Work with Sessions

**Interactive Method (Recommended):**
```bash
# Start computer session and give automation requests
raworc session
# Then just type: Please generate a Python script to calculate fibonacci numbers and save it to fib.py
```

**API Method:**
```bash
# Send automation request to computer session
raworc api sessions/{session-id}/messages --method POST --body '{"content":"Generate a Python script to calculate fibonacci numbers and save it to fib.py"}'

# Check computer responses
raworc api sessions/{session-id}/messages

# View recent messages (limit to last 10)
raworc api sessions/{session-id}/messages?limit=10
```

The computer agent will receive your request, perform the work using full computer access (file operations, web browsing, software installation), and respond with results. In the interactive session interface, computer responses appear automatically in real-time. All interactions are persistent and viewable through the API.

### Session Operations via API

Computer sessions support the following operations:
- **Create**: Creates new session and spawns computer container
- **Close**: Closes session and stops computer to save resources
- **Restore**: Restores closed session and restarts computer
- **Delete**: Deletes session and destroys computer container

### Authentication

```bash
# Get admin token
raworc api auth/login --method POST --body '{"user":"admin","pass":"admin"}'

# Get operator token
raworc api auth/login --method POST --body '{"user":"operator","pass":"admin"}'

# Check current authentication
raworc api auth/me
```

### Common Automation Use Cases

#### Automating Development Tasks
```bash
# Start a computer session for development automation
raworc session
# Then: "Please set up a new Python project with virtual environment, install pandas and requests, and create a basic data processing script"
```

#### Web Automation and Data Processing
```bash
# Start session for web automation
raworc session
# Then: "Please visit example.com, extract all the product links, and save them to a CSV file"
```

#### System Administration and DevOps
```bash
# Start session for system administration
raworc session  
# Then: "Please check system resources, update the server packages, and generate a system health report"
```

## Troubleshooting

### Services won't start
```bash
# Check if ports are in use
lsof -i :9000
lsof -i :3307

# For published CLI users:
raworc pull              # Pull latest versions
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

### Computer session issues
```bash
# List all session containers
docker ps -a --filter "name=raworc_session_"

# Clean up all sessions (API + containers)
raworc cleanup --yes      # Delete all sessions from API
raworc stop --cleanup     # Clean up remaining containers

# For published CLI users:
raworc pull               # Pull latest versions first
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

# Check computer session logs inside container
docker exec raworc_session_{session-id} ls -la /session/logs/
```

### CLI not found or issues
```bash
# If raworc command is not found in development
# NEVER use node index.js directly - always link CLI first using shell script
./scripts/link.sh
raworc --help  # Now this should work (uses linked Node.js CLI from cli/ folder)

# If still having issues, check symlink
which raworc   # Should show /usr/local/bin/raworc
ls -la /usr/local/bin/raworc  # Should point to your project's cli/ folder

# For published CLI users
npm install -g @raworc/cli  # Install Node.js package
raworc --help
```

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

The codebase is organized as follows:

```
raworc/
├── src/           # Rust source code
│   ├── api/       # API server code
│   ├── operator/  # Operator service
│   └── host/      # Host (Computer Use Agent) for containers
├── cli/           # Node.js CLI implementation (npm package)
├── scripts/       # Shell scripts for development workflow
├── migrations/    # Database migrations
├── docker/        # Docker configurations
└── tests/         # Integration tests
```

### Key Files to Understand

- `cli/index.js` - Node.js CLI entry point (published as npm package)
- `scripts/` - Shell scripts for development workflow (build, start, stop, link, etc.)
- `src/api/server.rs` - Rust API server implementation
- `src/operator/main.rs` - Rust Operator service
- `src/host/` - Rust Host (Computer Use Agent)
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

- **Add new CLI command**: Modify Node.js CLI in `cli/` folder
- **Add API endpoint**: Update `src/api/routes.rs` (Rust)
- **Modify database schema**: Add migration in `migrations/`
- **Update Docker setup**: Edit `Dockerfile.*` files or shell scripts in `scripts/` folder
- **Development workflow**: Use shell scripts in `scripts/` folder for build, start, stop operations

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
- Clean restart: `raworc start --restart` (linked Node.js CLI) or `./scripts/restart.sh` (shell script for development)
- **Remember**: Always use linked `raworc` command (Node.js CLI), never `node index.js` directly

## Version

Current version: 0.3.0

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
