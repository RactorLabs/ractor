---
sidebar_position: 2
title: Dev Mode
---

# Dev Mode - Coding Agent

Dev Mode enables the **Coding Agent** within the Raworc Computer Use Agent. This specialized mode allows your Host to write code, execute programs, and build agents using frameworks like LangGraph, CrewAI, AutoGen through conversational interfaces.

## What is Dev Mode?

Dev Mode enables the Coding Agent within your Computer Use Agent, providing specialized development capabilities:

### **Computer Use Agent (Base)**
- Controls the computer, manages files, runs software
- Handles all computer interface operations
- Manages development environment and tools
- Maintains session state and persistence

### **Coding Agent (Dev Mode)**
When Dev Mode is enabled, the Host gains coding capabilities to develop anything:
- **Scripts and automation tools** - Create custom scripts for any task
- **API integrations** - Build connections to external services
- **Agent development** - Build specialized agents using LangGraph, CrewAI, AutoGen, LangChain
- **Web applications** - Create websites, dashboards, and web services
- **Data processing tools** - Build custom analysis and reporting tools
- **Repository integration** - Clone and extend existing codebases

## How Dev Mode Works

Your Computer Use Agent with Coding Agent enabled can handle complex development workflows:

```bash
# Start Dev Mode session  
raworc session --secrets '{"ANTHROPIC_API_KEY":"your-key"}'

You: "I need to build a customer service agent using LangGraph"

Host (with Coding Agent): I'll handle this development task:
1. [Host] Setting up development environment
2. [Coding Agent] Cloning LangGraph repository and analyzing examples
3. [Coding Agent] Writing customer service agent code
4. [Host] Installing dependencies and testing the agent
5. [Coding Agent] Creating deployment configuration

[Host manages computer operations while Coding Agent handles all development work]
```

## Dev Mode Capabilities

### Conversational Framework Development

Your Host with Coding Agent can build using any framework through conversation:

```bash
# Start Dev Mode session
raworc session --secrets '{"ANTHROPIC_API_KEY":"your-key"}'

You: "Create a Python script that scrapes product data from Amazon and analyzes pricing trends"

Host (with Coding Agent): I'll handle this development task:
1. [Host] Setting up Python environment and installing libraries
2. [Coding Agent] Writing web scraping script with error handling
3. [Coding Agent] Creating data analysis and trend visualization
4. [Host] Testing the complete pipeline and saving results

[Host manages computer while Coding Agent writes the code]
```

#### **Framework Development Examples**
```bash
You: "Clone the LangGraph examples repo and help me build a customer service agent"

Host (with Coding Agent): I'll build this for you:
1. [Host] Cloning LangGraph repository and analyzing documentation
2. [Coding Agent] Setting up development environment and dependencies
3. [Coding Agent] Writing customer service agent using LangGraph patterns
4. [Host] Testing the agent and validating functionality

You: "Add memory capabilities to remember customer interactions"
[Coding Agent] Implementing persistent memory using LangGraph state management
[Host] Testing memory functionality and validating persistence
```

#### **API Integration Development**
```bash
You: "Create a script that posts our daily sales report to Slack automatically"

Host (with Coding Agent): I'll build this script:
1. [Host] Setting up Python environment and Slack API access
2. [Coding Agent] Writing script to generate sales report from database
3. [Coding Agent] Adding Slack API integration to post formatted reports
4. [Host] Testing the script and saving it for scheduled execution

Result: Script ready to run on your servers or automation platform
```

#### **Data Processing Script Development**
```bash
You: "Create a script that scrapes competitor prices and generates a pricing analysis report"

Host (with Coding Agent): I'll build this data processing script:
1. [Host] Setting up development environment and web scraping tools
2. [Coding Agent] Writing web scraping scripts for competitor websites
3. [Coding Agent] Creating price comparison and analysis logic
4. [Coding Agent] Building report generation with charts and insights
5. [Host] Testing the complete script and saving output files

Result: Script that generates pricing analysis reports on demand
```

## Dev Mode Development Types

### **What You Can Develop with Coding Agent**

The Coding Agent within your Host can develop anything:

- **Custom Scripts** - Python, JavaScript, Bash scripts for data processing and automation
- **API Integration Scripts** - Scripts that connect to external services and APIs
- **Static Web Applications** - Websites, dashboards, documentation sites
- **Agent Development** - Build agents using LangGraph, CrewAI, AutoGen, LangChain for deployment elsewhere
- **Data Analysis Tools** - Scripts for analysis, reporting, and data processing
- **Development Tools** - Code generators, testing utilities, deployment scripts

## Available Development Environments

Raworc sessions provide pre-configured environments with common tools and runtimes:

### Python Environment

**What's included**:
- Python 3.11 with pip, venv
- Common data science libraries can be installed
- Full filesystem access for file operations
- Web browsing capabilities via requests/selenium

**Example Usage**:
```bash
# Start Python development session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key"}' \
  --setup "pip install langchain openai pandas numpy matplotlib jupyter"

# In session:  
You: Create a LangChain RAG pipeline for document analysis
Assistant: I'll help you create a RAG pipeline...
[Assistant creates and runs the code]
```

### Node.js Environment  

**What's included**:
- Node.js LTS with npm, yarn
- Access to full npm ecosystem
- File system and network access
- Browser automation capabilities

**Example Usage**:
```bash
# Start Node.js development session
raworc session \
  --secrets '{"ANTHROPIC_API_KEY":"your-key","OPENAI_API_KEY":"your-openai-key"}' \
  --setup "npm install -g typescript && npm init -y && npm install langchain openai"

# In session:
You: Build a ChatGPT-style chatbot using LangChain
Assistant: I'll create a chatbot for you...
[Assistant builds the Node.js application]
```

### System Tools

**What's available**:
- Git for version control
- curl, wget for API calls
- Text editors (nano, vim)
- Process management tools
- Docker client (for containerized workflows)

## Framework Examples

#### **Repository Integration**

The Coding Agent can work with existing codebases:

```bash
# Start Dev Mode 
raworc session --secrets '{"ANTHROPIC_API_KEY":"your-key"}'

You: "Clone the LangGraph repository and show me how to build a customer service agent"
Host: I'll clone the LangGraph repo and guide you through building a customer service agent...
[Agent clones repo, analyzes structure, creates example]

You: "Now deploy this as a REST API"
Host: I'll create a FastAPI wrapper for your LangGraph agent...
[Agent builds deployment code]
```

#### **Framework Exploration**

Explore and learn frameworks interactively:

```bash
You: "Check out the CrewAI repository and build a content creation team"
Host: I'll explore CrewAI and build a content creation team for you...
[Agent clones CrewAI, studies examples, builds custom team]

You: "Compare this approach with AutoGen for the same task"
Host: Let me clone AutoGen and show you how to accomplish the same task...
[Agent compares frameworks and shows differences]
```

## Session Persistence & Remix

One of Raworc's key advantages is persistent sessions:

### Session Lifecycle

```bash
# Start development session
raworc session --secrets '{"ANTHROPIC_API_KEY":"your-key"}' 

# Work on your project...
You: Build a data analysis pipeline
# Session creates files in /session/code and /session/data

# Close session to save resources (optional)
raworc api sessions/{session-id}/close -m POST

# Restore later to continue work
raworc session --restore {session-id}

# Your files and environment are preserved!
```

### Session Remix

Create variations of your work:

```bash
# Remix existing session with modifications
raworc session --remix {parent-session-id}

# Selective remix (copy only specific content)
raworc session --remix {parent-session-id} --data false --code true
```

## Best Practices

1. **Environment Setup**: Use setup scripts for reproducible environments
2. **File Organization**: Keep code in `/session/code`, data in `/session/data`
3. **Secret Management**: Pass secrets via the `--secrets` parameter
4. **Session Remix**: Use remix to branch and experiment with different approaches
5. **Instructions**: Provide clear instructions to guide the AI assistant

## Next Steps

- **[CLI Usage Guide](/docs/guides/cli-usage)** - Master all session commands
- **[Sessions Concepts](/docs/concepts/sessions)** - Understand session architecture  
- **[API Reference](/docs/api/rest-api)** - Direct API access for automation
