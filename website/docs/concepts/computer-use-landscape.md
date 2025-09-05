---
sidebar_position: 4
title: Computer Use Landscape
---

# Computer Use Landscape

The computer use market is emerging with several platforms offering different approaches to automating computer-based tasks. True computer use platforms provide an agent runtime that can control computers like humans do — using visual interfaces, running software, and performing complex multi-step workflows.

## True Computer Use Platforms

### **Local Models via Ollama**
- **Purpose**: Run open models locally for computer use agents
- **Strengths**:
  - Runs on your hardware (CPU/GPU) with Docker
  - Easy to manage models (pull/switch) and cache
  - No external API keys required
- **Limitations**: Performance depends on your machine and selected model
- **Best For**: Teams preferring local inference and data locality

### **Raworc (Computer Use Agents)**
- **Purpose**: Complete computer use platform with dedicated computers and built-in agent runtime
- **Strengths**:
  - **Dedicated computers** - Each agent gets a full environment
  - **Built-in agent runtime** - Pre-configured to use local models via Ollama (default: gpt-oss:20b)
  - **Agent persistence** - Sleep and wake long-running automation workflows
  - **Natural language control** - Describe any task and the agent executes it
  - **No integration required** - Ready to automate any computer-based work immediately
- **Best For**: Anyone needing to automate manual computer work without technical setup

### **BrowserUse (Browser Automation)**
- **Purpose**: Browser-focused computer use for web automation
- **Strengths**:
  - **Specialized browser control** - Optimized for web-based tasks
  - **Multi-agent orchestration** - Multiple agents working together
  - **Vision-based interaction** - Uses visual understanding for web navigation
  - **Python integration** - Easy to integrate with Python workflows
- **Limitations**: Browser-only automation, requires setup and integration work
- **Best For**: Developers needing browser automation with computer use capabilities

## Development-Focused Platforms

### **E2B (Code Execution Sandboxes)**
- **Purpose**: Secure sandboxes for AI code execution
- **Strengths**: Fast VM-based sandboxes (~150ms startup), enterprise security
- **Computer Use**: Limited to code execution only, no full computer control
- **Best For**: Developers building AI coding assistants

### **Daytona (Cloud Development Environments)**
- **Purpose**: Cloud-based development environments and workspace management
- **Strengths**: 
  - Fast workspace creation and management
  - Docker/Kubernetes automation
  - Development environment standardization
  - IDE integration (VS Code, JetBrains)
- **Computer Use**: Development-focused only, not general computer automation
- **Best For**: Development teams needing standardized cloud development environments

## Workflow Automation (Not Computer Use)

### **n8n**
- **Purpose**: Workflow automation with AI integration
- **Computer Use**: API-based automation only, no computer interface control
- **Best For**: Teams needing API workflow automation

### **Zapier**  
- **Purpose**: SaaS integration platform
- **Computer Use**: No computer interface control, only API connections
- **Best For**: Simple SaaS integrations and basic automation

## Enterprise Cloud Platforms (Limited Computer Use)

### **Azure AI Foundry Agent Service**
- **Purpose**: Enterprise agent runtime with Microsoft integration
- **Computer Use**: Limited to Microsoft ecosystem applications
- **Best For**: Microsoft-centric enterprises

### **Vertex AI Agent Engine (Google)**
- **Purpose**: Managed agent runtime on Google Cloud
- **Computer Use**: No general computer control, API-based only
- **Best For**: Google Cloud enterprises needing agent deployment

## Computer Use Comparison Matrix

| Platform | Computer Control | Dedicated Computers | Agent Persistence | Natural Language | Setup Required |
|----------|------------------|---------------------|-------------------|------------------|----------------|
| **Raworc** | ✅ Full computer control | ✅ Dedicated per agent | ✅ Sleep/wake workflows | ✅ Conversational interface | ❌ None |
| **Local AI (direct)** | ✅ Computer use model | ❌ User provides computer | ❌ No persistence | ✅ Natural language | ✅ Integration work |
| **E2B** | ⚠️ Code execution only | ✅ VM sandboxes | ⚠️ Limited to code | ❌ API integration | ✅ SDK setup |
| **Enterprise platforms** | ❌ API-based only | ❌ Shared infrastructure | ⚠️ Platform-specific | ⚠️ Platform UIs | ✅ Complex setup |

## Why Most Platforms Aren't True Computer Use

**API-Only Automation**: Most platforms only connect APIs and services, not actual computer interfaces.

**Limited Scope**: Platforms like E2B focus on specific use cases (code execution) rather than general computer use.

**No Visual Interface Control**: True computer use requires understanding and controlling visual interfaces, not just APIs.

**Integration Complexity**: Even advanced AI models require significant integration work to provide computer environments.

## Raworc's Unique Position

### **Complete Computer Use Solution**
```
Other Platforms: [AI Model] + [Complex Setup] + [Limited Scope]
Raworc:         [Agent Runtime] + [Dedicated Computer] + [Any Task]
```

### **Key Differentiators**

1. **Instant Computer Use**: Get an agent with a dedicated computer in seconds, no setup
2. **Universal Automation**: Automate any computer-based task, not just specific workflows  
3. **Agent Persistence**: Long-running automation that survives sleeps/wakes
4. **Natural Language Control**: Conversational interface for any automation task
5. **Zero Integration**: No APIs, SDKs, or complex setup required

### **Target Market**

**Primary**: Organizations with manual computer work that needs automation
- Administrative tasks, data entry, document processing
- Web research, form filling, content management
- System administration, monitoring, reporting

**Not For**: Simple API integrations or basic workflow automation (use Zapier/n8n instead)

## Computer Use vs. Traditional Automation

### **Traditional Automation Limitations**
- **API Dependencies**: Only works if APIs exist and are accessible
- **Brittle Integration**: Breaks when software updates change APIs
- **Limited Scope**: Can't handle visual interfaces or human-like interactions
- **Technical Expertise**: Requires programming and integration skills

### **Computer Use Advantages**
- **Universal Compatibility**: Works with any software, even legacy applications
- **Visual Interface Control**: Can handle any visual interface like humans do
- **Robust Automation**: Adapts to interface changes and unexpected scenarios  
- **Natural Language**: Describe what you want; the agent figures out how to do it

## Getting Started

Ready to automate manual computer work with agents?

```bash
# Install Raworc
npm install -g @raworc/cli

# Start core services  
raworc start mysql server operator

# Create your first agent
raworc agent create

# Describe any manual work you want automated
You: "Help me organize these files and create a summary report"
```

## Next Steps

- **[Getting Started](/docs/getting-started)** - Set up your first agent
- **[Agents](/docs/concepts/agents)** - Understand agent management
- **[CLI Usage](/docs/guides/cli-usage)** - Master all commands for agent control
