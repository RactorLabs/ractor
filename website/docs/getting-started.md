---
sidebar_position: 2
title: Getting Started
---

# Getting Started with Raworc

Get started with the Universal AI Agent Runtime in just a few commands. Raworc provides the simplest developer experience for deploying AI agents.

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