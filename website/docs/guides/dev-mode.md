---
sidebar_position: 2
title: Dev Mode
---

# Dev Mode - Code Development Environment

Dev Mode provides direct access to the `/session/code` folder in your Host sessions, enabling you to write code, create scripts, build agents, perform analysis, and develop applications using any programming language or framework.

## What is Dev Mode?

Dev Mode gives you access to a persistent code development environment within your Host sessions:

### **Code Folder Access**
- Full read/write access to `/session/code` directory
- Persistent storage across session close/restore cycles  
- Create files, scripts, applications, and projects
- Install packages and dependencies as needed

### **Development Capabilities**
- **Agent Development** - Build AI agents using LangGraph, CrewAI, AutoGen
- **Script Creation** - Write automation scripts and utilities
- **Data Analysis** - Create analysis notebooks and visualization scripts
- **Application Development** - Build web apps, APIs, and tools
- **Project Management** - Organize code into structured projects

## Code Folder Structure

Your session's code folder is available at `/session/code` with full development access:

```
/session/code/
├── scripts/          # Automation scripts and utilities
├── agents/           # AI agent implementations  
├── analysis/         # Data analysis and notebooks
├── apps/             # Web applications and APIs
├── projects/         # Larger development projects
├── libs/             # Shared libraries and utilities
├── config/           # Configuration files
└── docs/             # Project documentation
```

## Getting Started with Dev Mode

### Enable Dev Mode

```bash
# Start session with development focus
raworc session
```

All sessions have access to the code folder - there's no special "dev mode" to enable. Simply start working with files in `/session/code`.

### Basic Development Commands

```bash
# In any Host session, navigate to code folder
You: "Let's work in the code folder. Show me what's there."

# Create a new Python script
You: "Create a Python script in /session/code/scripts/data_processor.py"

# Install packages for development  
You: "Install pandas, numpy, and matplotlib in this session"

# Run your code
You: "Execute the data_processor.py script with the sample data"
```

## Development Use Cases

### 1. AI Agent Development

Build intelligent agents using popular frameworks:

```bash
# Create LangGraph agent
You: "Create a LangGraph agent in /session/code/agents/ that processes customer emails and routes them to appropriate teams"

# Build CrewAI system
You: "Set up a CrewAI project in /session/code/projects/content-crew that has agents for research, writing, and editing"

# AutoGen multi-agent system
You: "Create an AutoGen multi-agent conversation system for code review in /session/code/agents/code-review"
```

### 2. Script Development

Create automation scripts and utilities:

```bash
# Data processing script
You: "Write a Python script that processes CSV files and generates summary reports"

# Web scraping utility
You: "Create a web scraping script that extracts product data from e-commerce sites"

# System automation
You: "Build a bash script that monitors logs and sends alerts when errors occur"
```

### 3. Data Analysis Projects

Develop analysis tools and notebooks:

```bash
# Analysis notebook
You: "Create a Jupyter notebook for sales data analysis with visualizations"

# Statistical analysis
You: "Build a Python script that performs statistical analysis on survey data"

# Machine learning project
You: "Create an ML project for customer churn prediction using scikit-learn"
```

### 4. Web Application Development

Build web applications and APIs:

```bash
# Flask web app
You: "Create a Flask web application with user authentication and data dashboard"

# FastAPI service
You: "Build a FastAPI service that provides REST endpoints for our data processing pipeline"

# React frontend
You: "Create a React application that consumes our API and displays interactive charts"
```

## Code Persistence and Management

### Session Persistence
- **Code survives session close/restore** - Your code folder persists when you close and restore sessions
- **Version control ready** - Initialize git repositories in your code folder
- **Package installations persist** - Installed packages remain available after restore

### Project Organization
```bash
# Organize your development work
You: "Create a project structure for a customer analytics platform with separate folders for backend, frontend, and data processing"

# Set up development environment
You: "Initialize a Python virtual environment and install all dependencies from requirements.txt"

# Version control setup
You: "Initialize a git repository in the project folder and create initial commit"
```

## Working with Different Languages

### Python Development
```bash
You: "Set up a Python development environment with pytest, black, and flake8"
You: "Create a Python package structure with __init__.py files and setup.py"
You: "Install and configure pre-commit hooks for code quality"
```

### JavaScript/Node.js Development  
```bash
You: "Initialize a Node.js project with package.json and install Express framework"
You: "Set up a TypeScript configuration with proper build pipeline"
You: "Create a Next.js application with API routes and database integration"
```

### Other Languages
```bash
You: "Set up a Rust development environment and create a CLI application"
You: "Install Go and create a microservice with HTTP endpoints"
You: "Set up a Java Spring Boot project with Maven dependencies"
```

## Development Workflows

### Agent Development Workflow
1. **Design** - Plan your agent architecture and capabilities
2. **Setup** - Install frameworks (LangGraph, CrewAI, AutoGen)
3. **Implement** - Create agent classes and conversation flows
4. **Test** - Run agents with sample inputs and scenarios
5. **Deploy** - Package agents for production use

### Application Development Workflow  
1. **Planning** - Define requirements and architecture
2. **Environment** - Set up development environment and dependencies
3. **Development** - Implement features iteratively
4. **Testing** - Create and run test suites
5. **Documentation** - Write API docs and usage guides

### Analysis Project Workflow
1. **Data exploration** - Load and examine datasets
2. **Preprocessing** - Clean and prepare data for analysis
3. **Analysis** - Perform statistical analysis and modeling
4. **Visualization** - Create charts and interactive dashboards
5. **Reporting** - Generate summary reports and insights

## Advanced Development Features

### Package Management
```bash
# Python packages
You: "Create a requirements.txt with all our project dependencies"

# Node.js packages  
You: "Set up package.json with development and production dependencies"

# System packages
You: "Install system-level tools needed for our development environment"
```

### Code Quality Tools
```bash
# Python code quality
You: "Set up black, flake8, and mypy for code formatting and type checking"

# JavaScript code quality
You: "Configure ESLint and Prettier for consistent code style"

# Testing frameworks
You: "Set up pytest for Python testing with coverage reports"
```

### Development Utilities
```bash
# Database tools
You: "Install and configure database clients for PostgreSQL and MongoDB"

# API testing
You: "Set up Postman collections for testing our API endpoints"

# Performance monitoring
You: "Install monitoring tools to track application performance"
```

## Tips for Effective Development

### Project Organization
- **Use clear folder structures** - Organize code by functionality
- **Follow naming conventions** - Use consistent file and variable naming
- **Document your code** - Add README files and inline documentation
- **Version control** - Use git for tracking changes

### Development Best Practices  
- **Start with small iterations** - Build and test incrementally
- **Use virtual environments** - Isolate project dependencies
- **Write tests early** - Create tests alongside your code
- **Handle errors gracefully** - Implement proper error handling

### Session Management
- **Use descriptive session names** - Name sessions by project or purpose
- **Close unused sessions** - Free up resources when not actively developing
- **Backup important work** - Export code or push to external repositories
- **Use session remixing** - Create variants for experimentation

## Getting Help

Your Host is an expert developer that can help with:

```bash
# Code review and suggestions
You: "Review this Python code and suggest improvements for performance and readability"

# Debugging assistance  
You: "Help me debug this error in my Flask application"

# Architecture advice
You: "What's the best way to structure this multi-agent system for scalability?"

# Learning new technologies
You: "Teach me how to use LangGraph to build conversational AI agents"
```

## Next Steps

- **[Getting Started](/docs/getting-started)** - Set up your first development session
- **[CLI Usage Guide](/docs/guides/cli-usage)** - Master session management commands
- **[Sessions](/docs/concepts/sessions)** - Understand session persistence and lifecycle
- **[API Reference](/docs/api/rest-api-reference)** - Integrate development workflows with APIs