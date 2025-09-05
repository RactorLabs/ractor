---
sidebar_position: 5
title: Agent Names and Publishing
---

# Agent Names and Publishing

Raworc provides powerful agent management through **Named Agents** and **Agent Publishing** - enabling organized workflows, collaboration, and knowledge sharing across teams and the community.

## Agent Naming

### Why Use Named Agents?

Agent names transform anonymous UUIDs into memorable, meaningful identifiers:

```bash
# Without names - hard to remember
raworc agent wake 7f3e2a1b-4c8d-9e5f-1234-567890abcdef

# With names - intuitive and memorable  
raworc agent wake "customer-analysis-q3"
```

### Agent Name Benefits

- **Human-readable identification** - Use descriptive names instead of UUIDs
- **Cross-user accessibility** - Published agents can be found by name globally
- **Organized workflows** - Group related agents with consistent naming patterns
- **Easy agent management** - Wake, remix, and reference agents by name

### Naming Conventions

**Recommended naming patterns:**

```bash
# Project-based naming
raworc agent create "project-website-redesign"
raworc agent create "project-mobile-app-v2"

# Task-based naming
raworc agent create "data-analysis-monthly-sales"
raworc agent create "automation-invoice-processing"

# Team-based naming  
raworc agent create "marketing-content-generation"
raworc agent create "devops-deployment-scripts"

# Date-based naming
raworc agent create "report-2024-q3-analysis"
raworc agent create "backup-cleanup-jan-2024"
```

### Name Requirements

- **Unique within scope** - Names must be unique for your agents
- **URL-safe characters** - Use letters, numbers, hyphens, underscores
- **Descriptive length** - Aim for 3-50 characters
- **No spaces** - Use hyphens or underscores instead

## Agent Publishing

### What is Agent Publishing?

Publishing makes private agents **publicly accessible** for remixing and collaboration:

```bash
# Make agent publicly accessible
raworc agent publish "my-data-analysis"

# Anyone can now remix this agent (no authentication required)
raworc agent remix "my-data-analysis" --name "my-version"
```

### Publishing Benefits

- **Knowledge sharing** - Share useful agents with the community
- **Template creation** - Create reusable agent templates
- **Collaboration** - Enable team members to build on your work
- **Learning resources** - Provide examples for others to learn from

### Publishing Permissions

Control what gets shared when publishing agents:

```bash
# Publish with full permissions (default)
raworc agent publish "my-agent"

# Publish with selective permissions
raworc agent publish "my-agent" \
  --data true \
  --code true \
  --secrets false
```

**Permission Types:**

- **`data`** - Share data files and documents created during the agent session
- **`code`** - Share code, scripts, and configuration files
- **secrets** - Share environment variables and API keys (**⚠️ Generally not recommended**)

### Publishing Workflow

```bash
# 1. Create and work on your agent
raworc agent create "web-scraping-tutorial"
# ... do work in the agent ...

# 2. Publish for others to use
raworc agent publish "web-scraping-tutorial" \
  --data true \
  --code true \
  --secrets false

# 3. Others can discover and remix
raworc api published/agents  # List all published agents
raworc agent remix "web-scraping-tutorial" --name "my-scraper"
```

## Practical Use Cases

### 1. Template Agents

Create reusable agent templates for common workflows:

```bash
# Create base agent for data analysis
raworc agent create "data-analysis-template" \
  --instructions "You are a data scientist. Use pandas, matplotlib, and seaborn for analysis." \
  --setup "pip install pandas matplotlib seaborn jupyter plotly"

# Work with the agent to set up tools, create example notebooks
# ... 

# Publish as template
raworc agent publish "data-analysis-template" \
  --data true \
  --code true \
  --secrets false

# Team members can remix for new projects
raworc agent remix "data-analysis-template" --name "sales-analysis-q4"
raworc agent remix "data-analysis-template" --name "customer-churn-analysis"
```

### 2. Tutorial and Learning Agents

Share educational agents with the community:

```bash
# Create tutorial agent
raworc agent create "python-web-scraping-tutorial" \
  --instructions "Teach web scraping with Python using requests and BeautifulSoup" \
  --setup "pip install requests beautifulsoup4 pandas"

# Create comprehensive examples, documentation, and sample code
# ...

# Publish for others to learn from
raworc agent publish "python-web-scraping-tutorial"

# Learners can remix and experiment
raworc agent remix "python-web-scraping-tutorial" --name "my-scraping-practice"
```

### 3. Team Collaboration

Share work within teams for collaboration:

```bash
# Team lead creates base agent
raworc agent create "product-launch-analysis" \
  --instructions "Analyze product launch metrics and create reports"

# Work on initial analysis
# ...

# Publish for team access
raworc agent publish "product-launch-analysis" \
  --data true \
  --code true \
  --secrets false

# Team members create specialized versions
raworc agent remix "product-launch-analysis" --name "marketing-metrics-deep-dive"
raworc agent remix "product-launch-analysis" --name "technical-performance-analysis"
```

### 4. Project Milestones

Preserve important project states:

```bash
# Create agent for project milestone
raworc agent create "website-redesign-milestone-1" \
  --instructions "Website redesign project - Phase 1 complete"

# Complete milestone work
# ...

# Publish milestone for team reference
raworc agent publish "website-redesign-milestone-1"

# Continue with next phase
raworc agent remix "website-redesign-milestone-1" --name "website-redesign-phase-2"
```

## Finding and Using Published Agents

### Discovery

```bash
# List all published agents
raworc api published/agents

# Get details about a published agent
raworc api published/agents/data-analysis-template

# Search published agents (use grep to filter)
raworc api published/agents | grep -i "analysis"
```

### Remixing Published Agents

```bash
# Remix with new name
raworc agent remix "published-agent-name" --name "my-version"

# Remix with selective copying
raworc agent remix "published-agent-name" \
  --name "code-only-version" \
  --data false \
  --code true \
  --secrets false

# Remix and start immediately with prompt
raworc agent remix "data-analysis-template" \
  --name "quarterly-sales-analysis" \
  --prompt "Analyze Q3 sales data and create executive summary"
```

## Agent Management Commands

### Naming Operations

```bash
# Create named agent
raworc agent create "my-agent"

# Wake by name
raworc agent wake "my-agent"

# Use agent name in API calls
raworc api agents/my-agent
raworc api agents/my-agent/messages
```

### Publishing Operations

```bash
# Publish agent
raworc agent publish "my-agent"

# Publish with permissions
raworc agent publish "my-agent" --data true --code true --secrets false

# Unpublish agent
raworc agent unpublish "my-agent"

# List published agents
raworc api published/agents

# Get published agent info
raworc api published/agents/agent-name
```

## Best Practices

### Naming Best Practices

1. **Be descriptive** - Use clear, meaningful names
2. **Use consistent patterns** - Establish team naming conventions
3. **Include context** - Add project, team, or purpose information
4. **Avoid sensitive information** - Don't include secrets or private data in names

### Publishing Best Practices

1. **Review content first** - Ensure no sensitive data before publishing
2. **Set appropriate permissions** - Generally exclude secrets from public agents
3. **Add documentation** - Include clear instructions and examples
4. **Update regularly** - Keep published templates current and useful

### Security Considerations

- **Never publish secrets** - Use `--secrets false` when publishing
- **Review data files** - Ensure no sensitive information in data
- **Use private agents for sensitive work** - Keep confidential work unpublished
- **Clean up before publishing** - Remove temporary files and sensitive logs

## Advanced Use Cases

### Agent Hierarchies

Create organized agent families:

```bash
# Base template
raworc agent create "ecommerce-analysis-base"
raworc agent publish "ecommerce-analysis-base"

# Specialized versions
raworc agent remix "ecommerce-analysis-base" --name "ecommerce-customer-segmentation"
raworc agent remix "ecommerce-analysis-base" --name "ecommerce-sales-forecasting" 
raworc agent remix "ecommerce-analysis-base" --name "ecommerce-inventory-optimization"
```

### Iterative Development

Version your agent work:

```bash
# Initial version
raworc agent create "ml-model-v1"
raworc agent publish "ml-model-v1"

# Improved version
raworc agent remix "ml-model-v1" --name "ml-model-v2"
raworc agent publish "ml-model-v2"

# Production version
raworc agent remix "ml-model-v2" --name "ml-model-production"
```

## Next Steps

- **[Agents Concepts](/docs/concepts/agents)** - Core agent architecture
- **[CLI Usage Guide](/docs/guides/cli-usage)** - Complete command reference
- **[Getting Started](/docs/getting-started)** - Basic agent setup
- **[API Reference](/docs/api/rest-api-reference)** - REST API for agent management
