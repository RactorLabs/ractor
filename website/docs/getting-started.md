---
sidebar_position: 2
title: Getting Started
---

# Getting Started with Raworc

Get started with the Universal AI Agent Runtime in just a few commands. Raworc provides the simplest developer experience for deploying AI agents with enterprise-grade reliability.

## Prerequisites

- **Node.js 16+**: For the Raworc CLI
- **Docker**: Docker Engine 20.10+ and Docker Compose v2+ 
- **Anthropic API Key**: Required for AI functionality - get one at [console.anthropic.com](https://console.anthropic.com)

## Quick Start (60 seconds)

### 1. Install Raworc CLI

```bash
npm install -g @raworc/cli
```

### 2. Start Services

```bash
raworc start
```

### 3. Authenticate

```bash
raworc auth login --user admin --pass admin
```

### 4. Set up API Key

```bash
raworc api spaces/default/secrets -m POST -b '{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-your-actual-key"
}'
```

### 5. Add Demo Agents

```bash
# Python agent that speaks English
raworc api spaces/default/agents -m POST -b '{
  "name": "raworc-agent-python-demo",
  "source_repo": "Raworc/raworc-agent-python-demo",
  "purpose": "Python agent that speaks English"
}'

# Rust agent that speaks in pirate  
raworc api spaces/default/agents -m POST -b '{
  "name": "raworc-agent-rust-demo",
  "source_repo": "Raworc/raworc-agent-rust-demo",
  "purpose": "Rust agent that speaks in pirate"
}'

# Node.js agent that speaks Klingon
raworc api spaces/default/agents -m POST -b '{
  "name": "raworc-agent-node-demo",
  "source_repo": "Raworc/raworc-agent-node-demo",
  "purpose": "Node.js agent that speaks Klingon"
}'

# Build space (wait for completion)
raworc api spaces/default/build -m POST

# Check build status (repeat until "completed")
raworc api spaces/default/build/latest
```

### 6. Start Your First Session

```bash
raworc session
```

Try these sample messages to see the value:
```
Say hi in English
Say hi in Klingon  
Say hi in Pirate
```

## Next Steps

For complete CLI usage, advanced features, and detailed commands, see the **[CLI Usage Guide](/docs/guides/cli-usage)**.

## Support

- **📚 Documentation**: [raworc.com/docs](https://raworc.com/docs)
- **🌐 Website**: [raworc.com](https://raworc.com)
- **🐦 Community**: Follow us on [X/Twitter](https://x.com/raworc)

## Session Restore

Raworc supports **reliable session persistence** - close sessions and restore them later with full state preservation:

```bash
# Close session (preserves state)
raworc api sessions/{session-id}/close

# Restore session later  
raworc api sessions/{session-id}/restore

# Continue with new messages
raworc session --restore {session-id}
```

**Key Features:**
- ✅ **No message reprocessing** - Restored sessions only handle new messages
- ✅ **Persistent storage** - All files and state preserved between restarts
- ✅ **Reliable message loop** - Second and subsequent messages process correctly
- ✅ **Fast restoration** - Sessions resume quickly with minimal overhead

## Session Remix

Create new sessions based on existing ones to branch your workflow:

```bash
# Create remix from existing session
raworc session --remix {source-session-id}

# Remix preserves all files and state from the source session
# but creates an independent new session for further development
```

**Use Cases:**
- 🔄 **Experiment branching** - Try different approaches from the same starting point
- 📋 **Template sessions** - Create base sessions and remix them for new projects
- 🧪 **A/B testing** - Compare different agent configurations from same baseline
- 🎯 **Checkpoint workflows** - Save progress and create multiple paths forward
