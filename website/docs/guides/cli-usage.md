---
sidebar_position: 1
title: CLI Usage (Docker-Only)
---

# Using the Raworc CLI

As of 0.6.0, the Raworc CLI focuses on Docker-based service management only.

Removed in 0.6.0
- API and agent commands: login, auth, token, api, agent, etc.
- Use the REST API directly (curl/Postman) for authentication and agent lifecycle.

## Prerequisites

- Node.js 16+
- Docker Engine 20.10+ (and Buildx)

## Install

```bash
npm install -g @raworc/cli
```

## Service Management

Start and manage Raworc services with Docker:

```bash
# Start core services (idempotent)
raworc start

# Pull latest images first
raworc start --pull

# Stop services
raworc stop

# Stop and clean agent containers
raworc stop --cleanup

# Check version
raworc --version
```

## Pull Prebuilt Images and CLI

```bash
# Pull latest CLI and images
raworc pull

# CLI only
raworc pull --cli-only

# Images only (Docker)
raworc pull --images-only
```

## Diagnostics

```bash
# Check host readiness, Docker/GPU
raworc doctor
```

## Next Steps

- API usage: see API overview and REST reference for authentication, agent creation, and lifecycle.
- Docker images: `raworc/raworc_server`, `raworc/raworc_controller`, `raworc/raworc_agent`.

