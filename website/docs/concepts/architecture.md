---
sidebar_position: 1
title: Architecture Overview
---

# Raworc Architecture Overview

Raworc is built using a **Kubernetes-inspired control plane pattern** for Computer Use Agent orchestration, providing dedicated computers for each agent with enterprise-grade operations.

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
                │ │  Agent +    │ │  Agent +    │ │
                │ │  Computer   │ │  Computer   │ │
                │ └─────────────┘ └─────────────┘ │
                └─────────────────────────────────┘
```

## Core Components

### Control Plane

The control plane manages the lifecycle and orchestration of Computer Use Agents:

#### API Server (`raworc_server`)
- **REST API** - Complete REST API for all operations
- **Authentication** - JWT-based authentication with RBAC
- **Agent Management** - Create, wake, sleep, delete agents
- **Message Routing** - Handle messages between CLI and Agent runtime
- **Auto-Wake** - Automatically wake sleeping agents when messages are sent

#### MySQL Database
- **Agent State** - Persistent agent metadata and state
- **Message History** - Complete message logs for each agent
- **Authentication** - User credentials and JWT tokens
- **RBAC Data** - Roles, permissions, and access control

#### Operator (`raworc_operator`)
- **Agent Orchestration** - Manage agent container lifecycle
- **Auto-Sleep** - Monitor idle agents and auto-sleep based on timeout
- **Container Management** - Create, start, stop, cleanup agent containers
- **Health Monitoring** - Monitor agent health and handle failures

### Computer Use Agents

Each agent provides a dedicated computer use environment with full computer access:

#### Agent Runtime (`raworc_agent`)
- **Computer Use Implementation** - Uses computers like humans do
- **Ollama Integration** - Local model inference via Ollama (`gpt-oss:20b` by default)
- **Full OS Access** - Access to a Linux environment for automation
- **Persistent State** - All files and state preserved between sleeps/wakes

#### Agent Container
- **Isolated Environment** - Each agent runs in a dedicated Docker container
- **Resource Limits** - CPU, memory, and storage constraints
- **Persistent Volumes** - Data survives container restarts
- **Network Isolation** - Secure network boundaries

## Key Architectural Patterns

### Kubernetes-Inspired Control Plane
- **Declarative State Management** - Agents declared as desired state
- **Controller Pattern** - Operator reconciles actual vs desired state
- **Event-Driven Architecture** - React to state changes and lifecycle events
- **Resource Management** - Efficient container and resource allocation

### Agent Lifecycle Management
- **State Machine** - Well-defined state transitions (init → idle → busy → slept → deleted)
- **Persistent Agents** - Sleep/wake without losing state
- **Auto-Wake** - Seamless wake when sending messages to sleeping agents
- **Timeout Management** - Automatic resource cleanup for idle agents

### Security Model
- **JWT Authentication** - Secure token-based authentication
- **RBAC Authorization** - Role-based access control for fine-grained permissions
- **Container Isolation** - Each agent runs in an isolated container
- **Encrypted Secrets** - Secure secret management for agent environment

## Data Flow

### Agent Creation Flow
1. **CLI Request** - User creates agent via CLI or API
2. **API Validation** - Server validates request and creates agent record
3. **Operator Detection** - Operator detects new agent in `init` state
4. **Container Creation** - Operator spawns Docker container with Agent runtime
5. **Agent Initialization** - Agent starts and transitions to `idle` state
6. **Ready for Messages** - Agent can now receive and process messages

### Message Processing Flow
1. **Message Reception** - API server receives message from CLI
2. **State Transition** - Agent transitions to `busy` state
3. **Agent Processing** - Agent runtime receives message and executes using computer use
4. **Response Generation** - Agent generates response and updates state to `idle`
5. **Result Delivery** - Response delivered back to CLI user

### Auto-Wake Flow
1. **Sleeping Agent Message** - Message sent to agent in `slept` state
2. **Wake Task** - Background task queued for agent wake
3. **Container Recreation** - Operator creates/starts container with preserved state
4. **Message Processing** - Message processed after wake completes

## Deployment Architecture

### Docker Stack
- **raworc_server** - API server container
- **raworc_operator** - Agent operator container
- **raworc_mysql** - MySQL database container
- **Agent Containers** - Dynamic containers for agent runtimes

### Network Architecture
- **raworc_network** - Isolated Docker network for all components
- **Port 9000** - API server HTTP endpoint
- **Port 3306** - MySQL database (internal only)
- **Agent Ports** - Dynamic port allocation for agent containers

### Storage Architecture
- **Agent Volumes** - Persistent Docker volumes per agent
- **Database Volume** - MySQL data persistence
- **Log Volumes** - Centralized logging for debugging

## Scalability Design

### Horizontal Scaling
- **Multiple Agents** - Run many concurrent agents per user
- **Resource Isolation** - Each agent has dedicated resources
- **Independent Scaling** - Scale control plane and agents independently

### Vertical Scaling
- **Configurable Limits** - Adjust CPU, memory, storage per session
- **Resource Pools** - Efficient resource allocation and cleanup
- **Performance Tuning** - Optimize for specific workload patterns

### Multi-Tenant Architecture
- **User Isolation** - Agents isolated per user/operator
- **Resource Quotas** - Configurable limits per tenant
- **Access Control** - RBAC for multi-tenant security

## Performance Characteristics

### Fast Agent Startup
- **Pre-built Images** - Ready-to-use Docker images for instant startup
- **Direct Agent Runtime Execution** - No compilation or build steps required
- **Efficient Resource Usage** - Minimal overhead for session management

### Persistent State Management
- **Volume Persistence** - All agent data survives container restarts
- **Message Continuity** - Complete message history preserved
- **State Synchronization** - Reliable state management across restarts

### Resource Efficiency
- **Auto-Close** - Automatic cleanup of idle sessions
- **Shared Images** - Common base images reduce storage overhead
- **Connection Pooling** - Efficient database connection management

## Next Steps

- [Agents](/docs/concepts/agents) - Deep dive into agent lifecycle and management
- [Computer Use Agents](/docs/concepts/computer-use-agents) - Understanding agent runtime capabilities
- [RBAC System](/docs/concepts/authentication-users) - Security and access control
- [API Reference](/docs/api/rest-api-reference) - Complete REST API documentation
