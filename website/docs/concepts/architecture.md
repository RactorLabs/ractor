---
sidebar_position: 1
title: Architecture Overview
---

# Raworc Architecture Overview

Raworc is built using a **Kubernetes-inspired control plane pattern** for Computer Use Agent orchestration, providing dedicated computers for each session with enterprise-grade operations.

## System Architecture

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

## Core Components

### Control Plane

The control plane manages the lifecycle and orchestration of Computer Use Agent sessions:

#### API Server (`raworc_server`)
- **REST API** - Complete REST API for all operations
- **Authentication** - JWT-based authentication with RBAC
- **Session Management** - Create, restore, close, delete sessions
- **Message Routing** - Handle messages between CLI and Host
- **Auto-Restore** - Automatically restore closed sessions when messages are sent

#### MySQL Database
- **Session State** - Persistent session metadata and state
- **Message History** - Complete message logs for each session
- **Authentication** - User credentials and JWT tokens
- **RBAC Data** - Roles, permissions, and access control

#### Operator (`raworc_operator`)
- **Session Orchestration** - Manage session container lifecycle
- **Auto-Close** - Monitor idle sessions and auto-close based on timeout
- **Container Management** - Create, start, stop, cleanup session containers
- **Health Monitoring** - Monitor session health and handle failures

### Computer Use Agents

Each session provides a dedicated computer use agent with full computer access:

#### Host (`raworc_host`)
- **Computer Use Implementation** - Uses computers like humans do
- **Anthropic Integration** - Claude-powered intelligent automation
- **Full OS Access** - Complete access to Linux desktop environment
- **Persistent State** - All files and state preserved between sessions

#### Session Container
- **Isolated Environment** - Each session runs in dedicated Docker container
- **Resource Limits** - CPU, memory, and storage constraints
- **Persistent Volumes** - Data survives container restarts
- **Network Isolation** - Secure network boundaries

## Key Architectural Patterns

### Kubernetes-Inspired Control Plane
- **Declarative State Management** - Sessions declared as desired state
- **Controller Pattern** - Operator reconciles actual vs desired state
- **Event-Driven Architecture** - React to state changes and lifecycle events
- **Resource Management** - Efficient container and resource allocation

### Session Lifecycle Management
- **State Machine** - Well-defined state transitions (init → idle → busy → closed)
- **Persistent Sessions** - Close/restore without losing state
- **Auto-Restore** - Seamless restoration when sending messages to closed sessions
- **Timeout Management** - Automatic resource cleanup for idle sessions

### Security Model
- **JWT Authentication** - Secure token-based authentication
- **RBAC Authorization** - Role-based access control for fine-grained permissions
- **Container Isolation** - Each session runs in isolated container
- **Encrypted Secrets** - Secure secret management for session environment

## Data Flow

### Session Creation Flow
1. **CLI Request** - User creates session via CLI or API
2. **API Validation** - Server validates request and creates session record
3. **Operator Detection** - Operator detects new session in `init` state
4. **Container Creation** - Operator spawns Docker container with Host
5. **Host Initialization** - Host starts and sets session to `idle` state
6. **Ready for Messages** - Session can now receive and process messages

### Message Processing Flow
1. **Message Reception** - API server receives message from CLI
2. **State Transition** - Session transitions to `busy` state
3. **Host Processing** - Host receives message and executes using computer use
4. **Response Generation** - Host generates response and updates state to `idle`
5. **Result Delivery** - Response delivered back to CLI user

### Auto-Restore Flow
1. **Closed Session Message** - Message sent to session in `closed` state
2. **Immediate Response** - API returns 200 OK without delay
3. **Restore Task** - Background task queued for session restoration
4. **Container Recreation** - Operator creates new container with preserved state
5. **Message Processing** - Message processed after restoration completes

## Deployment Architecture

### Docker Compose Stack
- **raworc_server** - API server container
- **raworc_operator** - Session operator container
- **raworc_mysql** - MySQL database container
- **Session Containers** - Dynamic containers for Host sessions

### Network Architecture
- **raworc_network** - Isolated Docker network for all components
- **Port 9000** - API server HTTP endpoint
- **Port 3306** - MySQL database (internal only)
- **Session Ports** - Dynamic port allocation for session containers

### Storage Architecture
- **Session Volumes** - Persistent Docker volumes per session
- **Database Volume** - MySQL data persistence
- **Log Volumes** - Centralized logging for debugging

## Scalability Design

### Horizontal Scaling
- **Multiple Sessions** - Run unlimited concurrent sessions per user
- **Resource Isolation** - Each session has dedicated resources
- **Independent Scaling** - Scale control plane and sessions independently

### Vertical Scaling
- **Configurable Limits** - Adjust CPU, memory, storage per session
- **Resource Pools** - Efficient resource allocation and cleanup
- **Performance Tuning** - Optimize for specific workload patterns

### Multi-Tenant Architecture
- **User Isolation** - Sessions isolated per user/operator
- **Resource Quotas** - Configurable limits per tenant
- **Access Control** - RBAC for multi-tenant security

## Performance Characteristics

### Fast Session Startup
- **Pre-built Images** - Ready-to-use Docker images for instant startup
- **Direct Host Execution** - No compilation or build steps required
- **Efficient Resource Usage** - Minimal overhead for session management

### Persistent State Management
- **Volume Persistence** - All session data survives container restarts
- **Message Continuity** - Complete message history preserved
- **State Synchronization** - Reliable state management across restarts

### Resource Efficiency
- **Auto-Close** - Automatic cleanup of idle sessions
- **Shared Images** - Common base images reduce storage overhead
- **Connection Pooling** - Efficient database connection management

## Next Steps

- [Sessions](/docs/concepts/sessions) - Deep dive into session lifecycle and management
- [Computer Use Agents](/docs/concepts/computer-use-agents) - Understanding Host capabilities
- [RBAC System](/docs/concepts/authentication-users) - Security and access control
- [API Reference](/docs/api/rest-api-reference) - Complete REST API documentation