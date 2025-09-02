---
sidebar_position: 5
title: Session Names and Publishing
---

# Session Names and Publishing

Raworc provides powerful session management through **Named Sessions** and **Session Publishing** - enabling organized workflows, collaboration, and knowledge sharing across teams and the community.

## Session Naming

### Why Use Named Sessions?

Session names transform anonymous UUIDs into memorable, meaningful identifiers:

```bash
# Without names - hard to remember
raworc session restore 7f3e2a1b-4c8d-9e5f-1234-567890abcdef

# With names - intuitive and memorable  
raworc session restore "customer-analysis-q3"
```

### Session Name Benefits

- **Human-readable identification** - Use descriptive names instead of UUIDs
- **Cross-user accessibility** - Published sessions can be found by name globally
- **Organized workflows** - Group related sessions with consistent naming patterns
- **Easy session management** - Restore, remix, and reference sessions by name

### Naming Conventions

**Recommended naming patterns:**

```bash
# Project-based naming
raworc session --name "project-website-redesign"
raworc session --name "project-mobile-app-v2"

# Task-based naming
raworc session --name "data-analysis-monthly-sales"
raworc session --name "automation-invoice-processing"

# Team-based naming  
raworc session --name "marketing-content-generation"
raworc session --name "devops-deployment-scripts"

# Date-based naming
raworc session --name "report-2024-q3-analysis"
raworc session --name "backup-cleanup-jan-2024"
```

### Name Requirements

- **Unique within scope** - Names must be unique for your sessions
- **URL-safe characters** - Use letters, numbers, hyphens, underscores
- **Descriptive length** - Aim for 3-50 characters
- **No spaces** - Use hyphens or underscores instead

## Session Publishing

### What is Session Publishing?

Publishing makes private sessions **publicly accessible** for remixing and collaboration:

```bash
# Make session publicly accessible
raworc session publish "my-data-analysis"

# Anyone can now remix this session (no authentication required)
raworc session remix "my-data-analysis" --name "my-version"
```

### Publishing Benefits

- **Knowledge sharing** - Share useful sessions with the community
- **Template creation** - Create reusable session templates
- **Collaboration** - Enable team members to build on your work
- **Learning resources** - Provide examples for others to learn from

### Publishing Permissions

Control what gets shared when publishing sessions:

```bash
# Publish with full permissions (default)
raworc session publish "my-session"

# Publish with selective permissions
raworc session publish "my-session" \
  --data true \
  --code true \
  --secrets false
```

**Permission Types:**

- **`data`** - Share data files and documents created during the session
- **`code`** - Share code, scripts, and configuration files
- **secrets** - Share environment variables and API keys (**⚠️ Generally not recommended**)

### Publishing Workflow

```bash
# 1. Create and work on your session
raworc session --name "web-scraping-tutorial"
# ... do work in the session ...

# 2. Publish for others to use
raworc session publish "web-scraping-tutorial" \
  --data true \
  --code true \
  --secrets false

# 3. Others can discover and remix
raworc api published/sessions  # List all published sessions
raworc session remix "web-scraping-tutorial" --name "my-scraper"
```

## Practical Use Cases

### 1. Template Sessions

Create reusable session templates for common workflows:

```bash
# Create base session for data analysis
raworc session --name "data-analysis-template" \
  --instructions "You are a data scientist. Use pandas, matplotlib, and seaborn for analysis." \
  --setup "pip install pandas matplotlib seaborn jupyter plotly"

# Work on the session to set up tools, create example notebooks
# ... 

# Publish as template
raworc session publish "data-analysis-template" \
  --data true \
  --code true \
  --secrets false

# Team members can remix for new projects
raworc session remix "data-analysis-template" --name "sales-analysis-q4"
raworc session remix "data-analysis-template" --name "customer-churn-analysis"
```

### 2. Tutorial and Learning Sessions

Share educational sessions with the community:

```bash
# Create tutorial session
raworc session --name "python-web-scraping-tutorial" \
  --instructions "Teach web scraping with Python using requests and BeautifulSoup" \
  --setup "pip install requests beautifulsoup4 pandas"

# Create comprehensive examples, documentation, and sample code
# ...

# Publish for others to learn from
raworc session publish "python-web-scraping-tutorial"

# Learners can remix and experiment
raworc session remix "python-web-scraping-tutorial" --name "my-scraping-practice"
```

### 3. Team Collaboration

Share work within teams for collaboration:

```bash
# Team lead creates base session
raworc session --name "product-launch-analysis" \
  --instructions "Analyze product launch metrics and create reports"

# Work on initial analysis
# ...

# Publish for team access
raworc session publish "product-launch-analysis" \
  --data true \
  --code true \
  --secrets false

# Team members create specialized versions
raworc session remix "product-launch-analysis" --name "marketing-metrics-deep-dive"
raworc session remix "product-launch-analysis" --name "technical-performance-analysis"
```

### 4. Project Milestones

Preserve important project states:

```bash
# Create session for project milestone
raworc session --name "website-redesign-milestone-1" \
  --instructions "Website redesign project - Phase 1 complete"

# Complete milestone work
# ...

# Publish milestone for team reference
raworc session publish "website-redesign-milestone-1"

# Continue with next phase
raworc session remix "website-redesign-milestone-1" --name "website-redesign-phase-2"
```

## Finding and Using Published Sessions

### Discovery

```bash
# List all published sessions
raworc api published/sessions

# Get details about a published session
raworc api published/sessions/data-analysis-template

# Search published sessions (use grep to filter)
raworc api published/sessions | grep -i "analysis"
```

### Remixing Published Sessions

```bash
# Remix with new name
raworc session remix "published-session-name" --name "my-version"

# Remix with selective copying
raworc session remix "published-session-name" \
  --name "code-only-version" \
  --data false \
  --code true \
  --secrets false

# Remix and start immediately with prompt
raworc session remix "data-analysis-template" \
  --name "quarterly-sales-analysis" \
  --prompt "Analyze Q3 sales data and create executive summary"
```

## Session Management Commands

### Naming Operations

```bash
# Create named session
raworc session --name "my-session"

# Restore by name
raworc session restore "my-session"

# Use session name in API calls
raworc api sessions/my-session
raworc api sessions/my-session/messages
```

### Publishing Operations

```bash
# Publish session
raworc session publish "my-session"

# Publish with permissions
raworc session publish "my-session" --data true --code true --secrets false

# Unpublish session
raworc session unpublish "my-session"

# List published sessions
raworc api published/sessions

# Get published session info
raworc api published/sessions/session-name
```

## Best Practices

### Naming Best Practices

1. **Be descriptive** - Use clear, meaningful names
2. **Use consistent patterns** - Establish team naming conventions
3. **Include context** - Add project, team, or purpose information
4. **Avoid sensitive information** - Don't include secrets or private data in names

### Publishing Best Practices

1. **Review content first** - Ensure no sensitive data before publishing
2. **Set appropriate permissions** - Generally exclude secrets from public sessions
3. **Add documentation** - Include clear instructions and examples
4. **Update regularly** - Keep published templates current and useful

### Security Considerations

- **Never publish secrets** - Use `--secrets false` when publishing
- **Review data files** - Ensure no sensitive information in data
- **Use private sessions for sensitive work** - Keep confidential work unpublished
- **Clean up before publishing** - Remove temporary files and sensitive logs

## Advanced Use Cases

### Session Hierarchies

Create organized session families:

```bash
# Base template
raworc session --name "ecommerce-analysis-base"
raworc session publish "ecommerce-analysis-base"

# Specialized versions
raworc session remix "ecommerce-analysis-base" --name "ecommerce-customer-segmentation"
raworc session remix "ecommerce-analysis-base" --name "ecommerce-sales-forecasting" 
raworc session remix "ecommerce-analysis-base" --name "ecommerce-inventory-optimization"
```

### Iterative Development

Version your session work:

```bash
# Initial version
raworc session --name "ml-model-v1"
raworc session publish "ml-model-v1"

# Improved version
raworc session remix "ml-model-v1" --name "ml-model-v2"
raworc session publish "ml-model-v2"

# Production version
raworc session remix "ml-model-v2" --name "ml-model-production"
```

## Next Steps

- **[Sessions Concepts](/docs/concepts/sessions)** - Core session architecture
- **[CLI Usage Guide](/docs/guides/cli-usage)** - Complete command reference
- **[Getting Started](/docs/getting-started)** - Basic session setup
- **[API Reference](/docs/api/rest-api-reference)** - REST API for session management