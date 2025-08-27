# Raworc Development

Universal AI Agent Runtime - Development repository.

## Quick Development Setup

```bash
# Install all dependencies
./scripts/install.sh

# Link CLI for development (makes 'raworc' command available)
./scripts/link.sh

# Build and start services
./scripts/build.sh
./scripts/start.sh
```

## Development Workflow

### Building

```bash
# Build Rust binaries and Docker images
./scripts/build.sh

# Build specific components
./scripts/build.sh server operator
./scripts/build.sh --tag 1.0.0 --no-cache
```

### Testing

```bash
# Start services
./scripts/start.sh

# Test with linked CLI (after ./scripts/link.sh)
raworc auth login --user admin --pass admin
raworc session

# Stop services when done
./scripts/stop.sh

# Restart services if needed
./scripts/restart.sh

# Complete reset if needed
./scripts/reset.sh
```

### Publishing

```bash
# Push Docker images to registry
./scripts/push.sh --tag v1.0.0 --registry raworc

# Publish npm CLI package
./scripts/publish.sh
./scripts/publish.sh --dry-run  # test first
```

### Development Scripts

- **`./scripts/install.sh`** - Install all dependencies (Rust + npm)
- **`./scripts/link.sh`** - Link CLI for development (makes `raworc` command available)
- **`./scripts/build.sh`** - Build Rust binaries and Docker images
- **`./scripts/start.sh`** - Start development services
- **`./scripts/stop.sh`** - Stop development services
- **`./scripts/restart.sh`** - Restart all services  
- **`./scripts/reset.sh`** - Complete cleanup
- **`./scripts/push.sh`** - Push Docker images to registry
- **`./scripts/publish.sh`** - Publish npm CLI package

### Repository Structure

```
raworc/
├── src/           # Rust services (server, operator, host)
├── cli/           # NPM CLI package (@raworc/cli)
├── scripts/       # Development workflow scripts
├── website/       # Documentation site
└── db/           # Database migrations
```

### Service Binaries

- **`raworc-server`** - API server (runs in Docker)
- **`raworc-operator`** - Container orchestration (runs in Docker)
- **`raworc-host`** - Session agent (runs in session containers)

### Prerequisites

- **Rust** - For building services
- **Node.js 16+** - For CLI development
- **Docker** - For containerization