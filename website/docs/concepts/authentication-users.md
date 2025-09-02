---
sidebar_position: 5
title: Authentication & Users
---

# Authentication & User Management

Raworc uses a simple authentication system with two types of users: **Operators** and **Users**. Authentication is handled through JWT tokens for secure API access.

## User Types

### Operators
**System administrators with full access to all functionality:**

- **Full system control** - Manage all sessions, users, and system settings
- **User management** - Create and manage user accounts 
- **Service management** - Control Raworc services and configuration
- **All API access** - Complete access to all REST API endpoints

**Default operator account:**
- Username: `admin`
- Password: `admin`
- Created automatically when Raworc starts

### Users
**Regular users with session-focused access:**

- **Session management** - Create, manage, and use their own sessions
- **Limited API access** - Access to session-related endpoints only
- **No admin functions** - Cannot manage other users or system settings
- **Personal workspace** - Work within their own session scope

## Authentication Flow

Raworc uses a **two-step authentication process**:

### Step 1: Generate Token (Login)
```bash
# Generate authentication token
raworc login --user admin --pass admin
```

**Response:**
```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "token_type": "Bearer",
  "expires_at": "2025-09-02T03:26:35Z",
  "user": "admin",
  "role": "admin"
}
```

### Step 2: Authenticate CLI
```bash
# Authenticate CLI with the token
raworc auth -t <jwt-token-from-step-1>
```

**Check authentication status:**
```bash
raworc auth
```

## User Management

### Creating Users
**Operators can create user accounts:**

```bash
# Create user token (operators only)
raworc token --principal myuser --type User

# Via API
raworc api auth/token -m POST -b '{
  "principal": "myuser",
  "type": "User"
}'
```

### Creating Operators
**Existing operators can create new operator accounts:**

```bash
# Create operator token
raworc token --principal newoperator --type Operator

# Via API  
raworc api auth/token -m POST -b '{
  "principal": "newoperator", 
  "type": "Operator"
}'
```

## Access Control

### What Operators Can Do
- ✅ Create and manage all sessions
- ✅ View all user sessions
- ✅ Create user and operator accounts
- ✅ Access all API endpoints
- ✅ Manage system configuration
- ✅ Publish and unpublish any sessions

### What Users Can Do  
- ✅ Create and manage their own sessions
- ✅ Restore their own sessions
- ✅ Remix published sessions
- ✅ Access session-related API endpoints
- ❌ View other users' sessions
- ❌ Create user accounts
- ❌ Access system management APIs

## API Authentication

### Using Tokens
All API requests require a Bearer token:

```bash
# Set token in Authorization header
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...

# CLI automatically handles authentication after raworc auth
raworc api sessions
raworc api sessions -m POST -b '{}'
```

### Token Validation
```bash
# Check if token is valid
raworc api auth

# Response for valid token
{
  "user": "admin",
  "type": "Operator"
}
```

## Security Best Practices

### Token Management
- **Keep tokens secure** - Don't share or commit tokens to version control
- **Token expiration** - Tokens expire automatically for security
- **Re-authenticate** - Generate new tokens when they expire
- **Logout when done** - Clear tokens with `raworc logout`

### Password Security  
- **Change default passwords** - Update the default admin password
- **Use strong passwords** - Choose complex, unique passwords
- **Limit access** - Only create accounts for users who need access

### Environment Security
- **Secure API keys** - Keep ANTHROPIC_API_KEY as environment variable
- **Network security** - Run Raworc on secure networks
- **Regular updates** - Keep Raworc updated to latest version

## Common Authentication Commands

```bash
# Login and authenticate (full flow)
raworc login --user admin --pass admin
raworc auth -t <token-from-login>

# Check authentication status
raworc auth

# Create user token (operators only)
raworc token --principal newuser --type User

# Clear authentication
raworc logout

# Test API access
raworc api version
raworc api sessions
```

## Troubleshooting

### Authentication Errors
```bash
# 401 Unauthorized - Token expired or invalid
raworc login --user admin --pass admin
raworc auth -t <new-token>

# 403 Forbidden - User lacks permissions
# Solution: Use operator account or request access
```

### Token Issues
```bash
# Invalid token format
Error: Invalid JWT token

# Solution: Check token string is complete
raworc auth -t "complete-token-here"

# Expired token
Error: Token expired

# Solution: Generate new token
raworc login --user admin --pass admin
```

## Next Steps

- **[Getting Started](/docs/getting-started)** - Set up authentication and first session
- **[CLI Usage Guide](/docs/guides/cli-usage)** - Complete authentication command reference  
- **[Sessions](/docs/concepts/sessions)** - Understand session management and access
- **[API Reference](/docs/api/rest-api-reference)** - REST API authentication details