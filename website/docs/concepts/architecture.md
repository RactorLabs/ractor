---
sidebar_position: 3
title: Raworc Architecture
---

# Raworc Architecture

Raworc's system architecture is designed to provide containerized sessions, enterprise-grade operations, and universal framework support. This technical overview covers the core components, data flow, and infrastructure design that enables Raworc to accelerate AI agent development from prototype to production.

## System Architecture

## Core Components

### 1. REST API Interface
HTTP-based API for all operations:
- **Session Management** - Create and manage agent sessions via HTTP
- **Authentication** - JWT token-based authentication system
- **Space Operations** - Create, update, and manage isolated workspaces
- **Agent Deployment** - Deploy agents from GitHub repositories

### 2. API Server
Rust-based REST API server that handles all platform operations:
- **Authentication** - JWT-based with configurable secrets
- **RBAC Enforcement** - Space-scoped permissions and role validation
- **Resource Management** - Sessions, spaces, secrets, and agents
- **Real-time Communication** - Message polling for agent interactions

### 3. Operator
Container lifecycle controller that manages agent execution:
- **Session Orchestration** - Create, close, restore, and destroy agent containers
- **Space Building** - Compile agents into immutable deployment images
- **Resource Control** - CPU, memory, and storage limits per session
- **State Management** - Track session state transitions and cleanup

### 4. Database (MySQL)
Persistent storage for all platform state:
- **Sessions** - Session metadata, state, and container assignments
- **Messages** - Complete conversation history between users and agents
- **Spaces** - Isolated environments for organizing agent projects
- **Secrets** - Encrypted API keys and credentials per space
- **RBAC** - Service accounts, roles, and permissions

### 5. Agent Containers
Isolated execution environments where AI agents run:
- **Containerized Sessions** - Dedicated container per agent session
- **Persistent Storage** - Data survives container close/restore cycles
- **Computer-Use** - File system, web browser, and system-level access
- **Multi-Agent** - Multiple agents can collaborate within sessions

## Key Capabilities

### BYOA: Bring Your Own Agents
Deploy agents from any framework without modification:
- **LangChain** - RAG agents, chains, and tools
- **CrewAI** - Multi-agent collaborative teams  
- **AutoGen** - Conversational multi-agent systems
- **LangGraph** - State machine workflows
- **Custom** - Any Python/Node.js/Rust implementation
- **Zero Dependencies** - No Raworc-specific SDKs required

### Containerized Sessions
Each agent session runs in its own secure container:
- **Isolation** - Agents cannot access host system or other sessions
- **Persistence** - Work state survives container restarts
- **Resource Limits** - Configurable CPU, memory, and storage controls
- **Computer-Use** - Safe access to filesystem, browser, and system tools

### Universal Runtime
Framework-agnostic infrastructure for any agent:
- **Git-Based Deployment** - Deploy directly from GitHub repositories
- **Multi-Language Support** - Python, Node.js, Rust with dependency management
- **Pre-Compilation** - Agents built once during space creation, not runtime
- **Instant Startup** - Sessions launch immediately from pre-built images

### Session Management
Professional workflow control for complex agent tasks:
- **State Machine** - `init → idle → busy → closed → error`
- **Close/Restore** - Stop and restart workflows without losing context
- **Session Forking** - Create child sessions from parent sessions
- **Data Lineage** - Track relationships between related sessions

### Enterprise Security
Production-ready security and access control:
- **RBAC System** - Role-based permissions with space isolation
- **Encrypted Secrets** - Secure storage of API keys and credentials
- **Service Accounts** - Machine-to-machine authentication
- **Audit Trails** - Complete operation tracking and attribution

## Agent Execution Flow

1. **Session Creation** - User creates session in a space via REST API
2. **Container Spawn** - Operator creates isolated container with pre-built agents
3. **Message Processing** - Agent receives messages and executes using AI capabilities
4. **Computer-Use Tasks** - Agent performs file operations, web browsing, code execution
5. **Response Generation** - Results sent back through secure API channels
6. **State Persistence** - Session state and data preserved across container lifecycle

## Data Flow

### Session Lifecycle
```
Create → Active → Close → Restore → Delete
```

**Create**: Spawn container with pre-built agents and persistent volume
**Active**: Agent processes messages and performs computer-use tasks
**Close**: Stop container to save resources while preserving state
**Restore**: Restart container from previous state with full context
**Delete**: Clean up container and session data

### Message Flow
```
User → HTTP/REST → API Server → Session Container → AI Agent → Results
```

Messages flow securely through the API server to isolated agent containers, where agents use AI capabilities to process requests and perform computer-use tasks.

## Technology Stack

### Core Infrastructure
- **Language**: Rust for performance and memory safety
- **Database**: MySQL 8.0 for production reliability
- **Containers**: Docker for isolation and portability
- **Authentication**: JWT tokens with RBAC
- **API**: REST-based JSON interface

### Deployment Architecture
- **Control Plane**: API Server + Operator + Database
- **Agent Nodes**: Containerized execution environments
- **Networking**: Secure container networking with access controls
- **Storage**: Persistent volumes for session data

## Scaling and Performance

### Resource Efficiency
- **Close/Restore** - Sessions only consume resources when active
- **Pre-Compilation** - Agents built once, deployed instantly
- **Container Reuse** - Efficient resource utilization strategies
- **Connection Pooling** - Optimized database connections

### Multi-Tenancy
- **Space Isolation** - Complete separation between teams/projects
- **Resource Quotas** - Configurable limits per space and session
- **RBAC Enforcement** - Fine-grained access control
- **Audit Logging** - Complete operational visibility

## Production Deployment

Raworc provides enterprise-grade features for production AI agent operations:

### Security
- Container isolation prevents agents from accessing host systems
- Encrypted secret storage for API keys and credentials
- Role-based access control with space-scoped permissions
- Complete audit trails for compliance and monitoring

### Reliability
- Session persistence ensures work survives infrastructure changes
- Resource limits prevent runaway agents from consuming all resources
- State machine validation ensures consistent session lifecycle
- Professional monitoring and logging for operational visibility

### Scalability
- Stateless API server design enables horizontal scaling
- Container-based architecture supports multi-host deployment
- Efficient resource utilization with close/restore capabilities
- Space isolation enables secure multi-tenant deployments

## Next Steps

- [Why Use Agent Runtime?](/docs/concepts/agent-runtime#why-use-agent-runtime) - Business case for agent runtimes
- [Agent Runtime Concept](/docs/concepts/agent-runtime) - Deep dive into runtime architecture
- [Try Community Edition](/docs/community-edition) - Deploy your first agent in 30 seconds
- [API Reference](/docs/api/overview) - Complete REST API documentation