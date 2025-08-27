---
sidebar_position: 2
title: Bring Your Own Agent
---

# Bring Your Own Agent (BYOA)

Raworc's **Bring Your Own Agent** philosophy enables you to deploy any agent from any framework without modification. Whether you're using LangChain, CrewAI, AutoGen, or custom implementations, Raworc provides the universal runtime infrastructure to get your agents running in production.

## What is BYOA?

**BYOA (Bring Your Own Agents)** means you can:
- Deploy agents from any GitHub repository without platform dependencies
- Use any AI framework: LangChain, CrewAI, AutoGen, LangGraph, or custom code
- Support multiple programming languages: Python, Node.js, Rust
- Require zero Raworc-specific SDKs or modifications
- Mix different frameworks within the same space

## Agent Repository Structure

Your agent repository only needs a simple structure with a manifest file:

```
my-agent/
├── raworc.json         # Manifest (required)
├── requirements.txt    # Python dependencies (optional)
├── package.json        # Node.js dependencies (optional)
├── Cargo.toml          # Rust dependencies (optional)
├── main.py             # Your agent implementation
└── README.md           # Documentation (optional)
```

## Agent Manifest (raworc.json)

The `raworc.json` file is the only Raworc-specific requirement. It tells the runtime how to execute your agent:

### Basic Manifest

```json
{
  "runtime": "python3",
  "handler": "main.process_message",
  "description": "My custom agent"
}
```

### Complete Manifest Options

```json
{
  "runtime": "python3|node|rust",
  "handler": "main.process_message",
  "description": "Agent description",
  "purpose": "What this agent does (used for delegation)",
  "build_command": "pip install additional-package",
  "capabilities": ["data-analysis", "web-scraping", "report-generation"]
}
```

### Manifest Fields

- **`runtime`** (required): Execution environment (`python3`, `node`, `rust`)
- **`handler`** (required): Entry point function for message processing
- **`description`** (optional): Human-readable agent description
- **`purpose`** (optional): Used for intelligent agent delegation
- **`build_command`** (optional): Additional setup commands during space build
- **`capabilities`** (optional): List of agent capabilities for coordination

## Supported Runtimes

### Python Runtime

**Supported versions**: `python3`, `python3.11`

**Automatic setup**:
- Creates isolated virtual environment
- Installs dependencies from `requirements.txt`
- Sets up proper Python path

**Handler signature**:
```python
def process_message(message: str, context: dict) -> str:
    """
    Process incoming message and return response.
    
    Args:
        message: User message content
        context: Session context and metadata
        
    Returns:
        Agent response as string
    """
    return "Agent response"
```

### Node.js Runtime

**Supported versions**: `node`, `nodejs` (latest LTS)

**Automatic setup**:
- Runs `npm install` for `package.json`
- Sets up Node.js module resolution
- Configures proper execution environment

**Handler signature**:
```javascript
exports.process_message = async (message, context) => {
    /*
     * Process incoming message and return response.
     * 
     * @param {string} message - User message content
     * @param {object} context - Session context and metadata
     * @returns {string} Agent response
     */
    return "Agent response";
};
```

### Rust Runtime

**Supported**: Rust with Cargo

**Automatic setup**:
- Runs `cargo build --release`
- Creates optimized binary
- Sets up library linking if needed

**Handler signature**:
```rust
// For library-style agents (conceptual)
pub fn process_message(message: &str, context: &serde_json::Value) -> String {
    format!("Agent response to: {}", message)
}
```

## Framework Examples

### LangChain Agent

```python
# requirements.txt
langchain
openai
chromadb

# main.py
from langchain.agents import initialize_agent
from langchain.tools import Tool
from langchain.llms import OpenAI

def process_message(message: str, context: dict) -> str:
    # Initialize LangChain agent with tools
    llm = OpenAI(temperature=0)
    tools = [
        Tool(
            name="Calculator",
            description="Useful for math calculations",
            func=lambda x: str(eval(x))
        )
    ]
    
    agent = initialize_agent(
        tools, 
        llm, 
        agent_type="zero-shot-react-description",
        verbose=True
    )
    
    return agent.run(message)
```

### CrewAI Multi-Agent Team

```python
# requirements.txt
crewai
openai

# main.py
from crewai import Agent, Task, Crew
import os

def process_message(message: str, context: dict) -> str:
    # Define agents
    researcher = Agent(
        role='Researcher',
        goal='Research topics thoroughly',
        backstory='Expert at finding and analyzing information',
        verbose=True
    )
    
    writer = Agent(
        role='Writer',
        goal='Write engaging content',
        backstory='Skilled at creating compelling narratives',
        verbose=True
    )
    
    # Define task
    task = Task(
        description=f"Research and write about: {message}",
        agent=researcher
    )
    
    # Create crew
    crew = Crew(
        agents=[researcher, writer],
        tasks=[task]
    )
    
    result = crew.kickoff()
    return str(result)
```

### AutoGen Conversation

```python
# requirements.txt
autogen-agentchat

# main.py
import autogen

def process_message(message: str, context: dict) -> str:
    config_list = [{
        "model": "gpt-3.5-turbo",
        "api_key": os.environ.get("OPENAI_API_KEY")
    }]
    
    user_proxy = autogen.UserProxyAgent(
        name="user_proxy",
        human_input_mode="NEVER",
        code_execution_config={"work_dir": "coding"}
    )
    
    assistant = autogen.AssistantAgent(
        name="assistant",
        llm_config={"config_list": config_list}
    )
    
    user_proxy.initiate_chat(
        assistant,
        message=message
    )
    
    # Get last message from assistant
    last_message = assistant.last_message()
    return last_message.get("content", "No response")
```

### LangGraph Workflow

```python
# requirements.txt
langgraph
openai

# main.py
from langgraph.graph import StateGraph, END
from typing import TypedDict

class State(TypedDict):
    message: str
    response: str

def analyze_message(state: State) -> State:
    # Analyze the incoming message
    state["response"] = f"Analyzed: {state['message']}"
    return state

def generate_response(state: State) -> State:
    # Generate final response
    state["response"] = f"Response to {state['message']}"
    return state

def process_message(message: str, context: dict) -> str:
    # Build LangGraph workflow
    workflow = StateGraph(State)
    workflow.add_node("analyze", analyze_message)
    workflow.add_node("generate", generate_response)
    
    workflow.set_entry_point("analyze")
    workflow.add_edge("analyze", "generate")
    workflow.add_edge("generate", END)
    
    app = workflow.compile()
    
    result = app.invoke({"message": message, "response": ""})
    return result["response"]
```

### Custom Agent Implementation

```python
# requirements.txt
requests
beautifulsoup4

# main.py
import requests
from bs4 import BeautifulSoup

def process_message(message: str, context: dict) -> str:
    """Custom web scraping agent"""
    
    if "search" in message.lower():
        # Extract search query
        query = message.replace("search", "").strip()
        
        # Perform web search (simplified)
        search_url = f"https://httpbin.org/json"
        response = requests.get(search_url)
        
        if response.status_code == 200:
            return f"Found results for: {query}\n{response.text[:500]}"
        else:
            return f"Search failed for: {query}"
    
    elif "analyze" in message.lower():
        # Perform analysis
        return f"Analysis complete for: {message}"
    
    else:
        return f"Echo: {message}"
```

## Environment Variables and Secrets

Agents automatically receive space secrets as environment variables:

```python
import os

def process_message(message: str, context: dict) -> str:
    # Access secrets set in the space
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    database_url = os.environ.get("DATABASE_URL")
    
    if not api_key:
        return "Error: ANTHROPIC_API_KEY not configured"
    
    # Use the API key in your agent
    # ... agent logic here
    
    return "Agent response"
```

## Agent Context

The `context` parameter provides session and space information:

```python
def process_message(message: str, context: dict) -> str:
    session_id = context.get("session_id")
    space_name = context.get("space")
    user = context.get("created_by")
    
    return f"Hello {user}! Session {session_id} in space {space_name}"
```

## Deployment Process

### 1. Create Agent Repository

```bash
# Create new repository
mkdir my-agent && cd my-agent

# Create manifest
cat > raworc.json << 'EOF'
{
  "runtime": "python3",
  "handler": "main.process_message",
  "description": "My custom agent"
}
EOF

# Create implementation
cat > main.py << 'EOF'
def process_message(message: str, context: dict) -> str:
    return f"Echo: {message}"
EOF

# Create dependencies
cat > requirements.txt << 'EOF'
requests
EOF

# Commit to GitHub
git init
git add .
git commit -m "Initial agent implementation"
git remote add origin https://github.com/yourusername/my-agent.git
git push -u origin main
```

### 2. Deploy to Raworc

```bash
# Add agent to space
raworc api spaces/default/agents -m post -b '{
  "name": "my-agent",
  "description": "My custom agent implementation",
  "purpose": "Custom message processing and analysis",
  "source_repo": "github.com/yourusername/my-agent",
  "source_branch": "main"
}'

# Build space (compiles all agents)
raworc api spaces/default/build -m post

# Check build status
raworc api spaces/default/build/latest

# Create session to test
raworc api sessions -m post -b '{"space":"default"}'

# Save session ID from response and send a test message
export SESSION_ID="your-session-id-here"
raworc api sessions/$SESSION_ID/messages -m post -b '{"content":"Hello, test my custom agent"}'
```

### 3. Test Your Agent

```bash
# In the interactive session, test your agent:
Hello, this is a test message
search for AI agent frameworks
analyze this data: [1,2,3,4,5]
```

## Build Process

When you add an agent to a space, Raworc automatically:

1. **Clones Repository**: Downloads your agent from GitHub
2. **Detects Runtime**: Identifies Python, Node.js, or Rust based on files
3. **Installs Dependencies**: Runs `pip install`, `npm install`, or `cargo build`
4. **Runs Build Command**: Executes optional `build_command` from manifest
5. **Creates Space Image**: Packages everything into `raworc_space_{name}:{build-id}`
6. **Ready for Sessions**: New sessions use the pre-built image for instant startup

### Build Logs

```bash
# Check build status
raworc api spaces/default/build/latest

# View operator logs for build details
docker logs raworc_operator --tail 100

# Check for build errors
docker logs raworc_operator | grep -A 10 -B 10 "ERROR"
```

## Agent Logging

Your agent's output is automatically captured:

```python
def process_message(message: str, context: dict) -> str:
    print(f"Processing: {message}")  # Captured in stdout logs
    
    try:
        result = perform_task(message)
        print(f"Success: {result}")
        return result
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)  # Captured in stderr logs
        return f"Error processing message: {e}"
```

**Log Files**:
- `stdout`: `/session/logs/{agent}_{timestamp}_stdout.log`
- `stderr`: `/session/logs/{agent}_{timestamp}_stderr.log`

**Viewing Logs**:
```bash
# List log files
docker exec raworc_session_{session-id} ls -la /session/logs/

# View specific logs
docker exec raworc_session_{session-id} cat /session/logs/my-agent_20250122_143052_123_stdout.log
```

## Multi-Agent Coordination

Raworc supports intelligent delegation between agents in the same space:

### Agent Purpose Declaration

```json
{
  "runtime": "python3",
  "handler": "main.process_message",
  "purpose": "Data analysis and visualization using pandas and matplotlib",
  "capabilities": ["data-analysis", "visualization", "statistics"]
}
```

### Delegation Example

When a user sends a message to a session with multiple agents, Raworc's LLM-powered delegation routes the message to the most appropriate agent based on:

- Agent `purpose` descriptions
- Agent `capabilities` lists
- Message content analysis
- Context and conversation history

## Best Practices

### Repository Organization

```
my-agent/
├── raworc.json              # Manifest
├── main.py                  # Entry point
├── requirements.txt         # Dependencies
├── src/                     # Source code
│   ├── __init__.py
│   ├── agent.py
│   └── tools/
├── tests/                   # Unit tests
├── README.md               # Documentation
└── .gitignore             # Git ignore
```

### Error Handling

```python
def process_message(message: str, context: dict) -> str:
    try:
        # Agent logic here
        result = process_user_request(message)
        return result
        
    except ValueError as e:
        return f"Invalid input: {e}"
        
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        return "Sorry, I encountered an error processing your request."
```

### Performance Optimization

```python
# Cache expensive operations
from functools import lru_cache

@lru_cache(maxsize=128)
def expensive_computation(input_data):
    # Expensive operation here
    return result

def process_message(message: str, context: dict) -> str:
    # Use cached computation
    result = expensive_computation(message)
    return f"Result: {result}"
```

### Security Considerations

```python
import os
import re

def process_message(message: str, context: dict) -> str:
    # Validate input
    if not message or len(message) > 10000:
        return "Invalid message length"
    
    # Sanitize input for security
    safe_message = re.sub(r'[<>"\']', '', message)
    
    # Use environment variables for secrets
    api_key = os.environ.get("API_KEY")
    if not api_key:
        return "Configuration error: API key not set"
    
    # Process safely
    return process_safely(safe_message, api_key)
```

## Troubleshooting

### Common Build Issues

**Dependencies not found**:
```bash
# Check requirements.txt syntax
cat requirements.txt

# Verify package names on PyPI
pip search package-name
```

**Build timeout**:
```json
{
  "build_command": "pip install --timeout 300 -r requirements.txt"
}
```

**Permission errors**:
```bash
# Ensure your repository is public or accessible
# Check GitHub repository URL is correct
```

### Runtime Issues

**Handler not found**:
```json
{
  "handler": "main.process_message"  // Correct: module.function
}
```

**Import errors**:
```python
# Ensure all dependencies in requirements.txt
# Use absolute imports
from mypackage.module import function
```

**Environment variables**:
```bash
# Check secrets are set in space
raworc api spaces/default/secrets

# Verify secret names match your code
```

## Demo Repositories

Explore these working examples:

- **Python**: [raworc-agent-python-demo](https://github.com/Raworc/raworc-agent-python-demo)
- **Node.js**: [raworc-agent-js-demo](https://github.com/Raworc/raworc-agent-js-demo)  
- **Rust**: [raworc-agent-rust-demo](https://github.com/Raworc/raworc-agent-rust-demo)

## Next Steps

- [CLI Usage](/docs/guides/cli-usage) - Learn comprehensive CLI usage
- [Spaces and Sessions](/docs/concepts/spaces-and-sessions) - Understand data models
- [Architecture Overview](/docs/concepts/architecture) - System design details
- [API Reference](/docs/api/overview) - Complete CLI and API documentation