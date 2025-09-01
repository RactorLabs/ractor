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

For development setup, prerequisites, and repository structure, see [README.md](README.md).

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

## Key Commands 

For complete command reference and development scripts, see [README.md](README.md#development-scripts).

**Key Development Commands:**
- `./scripts/link.sh` - Link CLI for development (REQUIRED FIRST STEP)
- `./scripts/build.sh` - Build Docker images locally
- `./scripts/start.sh` - Start services with local images  
- `raworc session` - Interactive computer automation
- `raworc api <endpoint>` - Direct REST API access

## CLI Usage

For complete CLI usage documentation, authentication, and troubleshooting, refer to the published documentation and README.

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
