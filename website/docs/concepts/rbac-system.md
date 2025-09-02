---
sidebar_position: 5
title: RBAC & Authentication
---

# Role-Based Access Control (RBAC) & Authentication

Raworc implements a comprehensive RBAC system providing fine-grained access control for Host sessions. The system supports both user accounts and operators with JWT-based authentication, focusing on session-based permissions for remote computer management.

## Architecture Overview

### RBAC Components

```
┌─────────────────────────────────────────────────────────────────────┐
│                           RBAC Architecture                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐  │
│  │   JWT Token     │────│  RBAC Engine    │────│   Permissions   │  │
│  │ (Claims + Sub)  │    │ (Authorization) │    │ (Allow/Deny)    │  │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘  │
│           │                       │                       │         │
│           │                       │                       │         │
│    ┌──────▼─────┐         ┌───────▼──────┐       ┌────────▼───────┐ │
│    │ Principal  │         │Role Bindings │       │     Rules      │ │
│    │  (User/Op) │         │(Who + What)  │       │ (API + Verbs)  │ │
│    └────────────┘         └──────────────┘       └────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Components:**
- **JWT Claims**: Identity and authorization information
- **Principals**: Users and Operators
- **Roles**: Collections of permissions (rules)
- **Role Bindings**: Mapping principals to roles globally
- **Rules**: API permissions (resource + verbs)

## Authentication System

### JWT Token Structure

**Token Claims:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacClaims {
    pub sub: String,               // Subject name (user or operator)
    pub sub_type: SubjectType,     // Subject type identifier
    pub exp: usize,                // Expiration timestamp
    pub iat: usize,                // Issued at timestamp
    pub iss: String,               // Issuer (usually "raworc")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubjectType {
    Subject,         // Human user
    Operator,  // Automated service
}
```

**Token Generation:**
```rust
pub async fn create_jwt_token(&self, principal: &str, sub_type: SubjectType) -> Result<String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let expiration = now + (24 * 60 * 60); // 24 hours
    
    let claims = RbacClaims {
        sub: principal.to_string(),
        sub_type,
        exp: expiration,
        iat: now,
        iss: "raworc".to_string(),
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(self.jwt_secret.as_ref()),
    ).map_err(|e| anyhow::anyhow!("JWT encoding failed: {}", e))
}
```

### Authentication Methods

#### 1. Internal Authentication (Development)

**User Login:**
```bash
raworc auth login --user admin --pass admin
```

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_at": "2025-08-22T10:00:00Z",
  "principal": "admin",
  "principal_type": "Subject"
}
```

#### 2. Operator Authentication

**Operator Token Generation:**
```rust
async fn create_operator_token(&self, operator_name: &str) -> Result<String> {
    // Verify operator exists
    let operator = self.get_operator(operator_name).await?;
    
    // Create JWT token for operator
    self.create_jwt_token(
        &format!("system:operator:{}", operator_name),
        SubjectType::Operator
    ).await
}
```

#### 3. External Authentication (Planned)

```rust
// Planned: OIDC integration
pub struct OidcAuthProvider {
    client_id: String,
    client_secret: String,
    issuer_url: String,
    redirect_uri: String,
}

impl OidcAuthProvider {
    async fn validate_oidc_token(&self, token: &str) -> Result<TokenClaims> {
        // Validate OIDC token with external provider
        // Convert to internal JWT claims
    }
}
```

## RBAC Model

### Subjects (Principals)

**User Subjects:**
```sql
-- Users are implicit - no separate user table
-- Identity comes from authentication provider
```

**Operators:**
```sql
CREATE TABLE operators (
    name VARCHAR(255) NOT NULL PRIMARY KEY,
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(255) NOT NULL,
    metadata JSON
);
```

### Roles

**Role Definition:**
```sql
CREATE TABLE roles (
    name VARCHAR(255) NOT NULL PRIMARY KEY,
    description TEXT,
    rules JSON NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(255) NOT NULL,
    metadata JSON
);
```

**Rule Structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: Option<String>,
    pub rules: Vec<Rule>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub api_groups: Vec<String>,             // ["api", "rbac"] 
    pub resources: Vec<String>,              // ["sessions", "agents", "*"]
    pub verbs: Vec<String>,                  // ["get", "list", "create", "update", "delete"]
    pub resource_names: Option<Vec<String>>, // Optional specific resource names
}
```

### Role Bindings

**Role Binding Definition:**
```sql
CREATE TABLE role_bindings (
    name VARCHAR(255) NOT NULL PRIMARY KEY,
    role_name VARCHAR(255) NOT NULL,
    subject_name VARCHAR(255) NOT NULL,
    subject_type ENUM('Subject', 'Operator') NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(255) NOT NULL,
    metadata JSON,
    CONSTRAINT fk_role_bindings_role FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
    INDEX idx_role_bindings_subject (subject_name, subject_type),
    INDEX idx_role_bindings_role (role_name)
);
```

**Role Binding Structure:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBinding {
    pub name: String,
    pub role_name: String,
    pub subject_name: String,
    pub subject_type: SubjectType,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub metadata: serde_json::Value,
}
```

## Permission System

### Resource Hierarchy

**API Resources:**
```rust
// Host session permissions
pub const SESSION_LIST: &str = "session.list";
pub const SESSION_GET: &str = "session.get";
pub const SESSION_CREATE: &str = "session.create";
pub const SESSION_UPDATE: &str = "session.update";
pub const SESSION_DELETE: &str = "session.delete";
pub const SESSION_MESSAGE_CREATE: &str = "session.message.create";
pub const SESSION_MESSAGE_LIST: &str = "session.message.list";

// RBAC permissions
pub const RBAC_ROLE_LIST: &str = "rbac.role.list";
pub const RBAC_ROLE_GET: &str = "rbac.role.get";
pub const RBAC_ROLE_CREATE: &str = "rbac.role.create";
pub const RBAC_ROLE_UPDATE: &str = "rbac.role.update";
pub const RBAC_ROLE_DELETE: &str = "rbac.role.delete";

pub const RBAC_ROLE_BINDING_LIST: &str = "rbac.role_binding.list";
pub const RBAC_ROLE_BINDING_GET: &str = "rbac.role_binding.get";
pub const RBAC_ROLE_BINDING_CREATE: &str = "rbac.role_binding.create";
pub const RBAC_ROLE_BINDING_UPDATE: &str = "rbac.role_binding.update";
pub const RBAC_ROLE_BINDING_DELETE: &str = "rbac.role_binding.delete";

pub const RBAC_OPERATOR_LIST: &str = "rbac.operator.list";
pub const RBAC_OPERATOR_GET: &str = "rbac.operator.get";
pub const RBAC_OPERATOR_CREATE: &str = "rbac.operator.create";
pub const RBAC_OPERATOR_UPDATE: &str = "rbac.operator.update";
pub const RBAC_OPERATOR_DELETE: &str = "rbac.operator.delete";
```

### Authorization Engine

**Permission Check Implementation:**
```rust
pub struct RbacEngine {
    pool: sqlx::MySqlPool,
}

impl RbacEngine {
    pub async fn check_permission(&self, principal: &str, sub_type: SubjectType, permission: &str) -> Result<bool> {
        // Get all role bindings for this principal
        let role_bindings = self.get_role_bindings_for_principal(principal, sub_type).await?;
        
        for binding in role_bindings {
            // Get role rules
            let role = self.get_role(&binding.role_name).await?;
            
            // Check if any rule grants this permission
            if self.check_rules_allow_permission(&role.rules, permission).await? {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    async fn check_rules_allow_permission(&self, rules: &[Rule], permission: &str) -> Result<bool> {
        let (api_group, resource, verb) = self.parse_permission(permission)?;
        
        for rule in rules {
            // Check API group match
            if !rule.api_groups.contains(&api_group) && !rule.api_groups.contains(&"*".to_string()) {
                continue;
            }
            
            // Check resource match
            if !rule.resources.contains(&resource) && !rule.resources.contains(&"*".to_string()) {
                continue;
            }
            
            // Check verb match
            if !rule.verbs.contains(&verb) && !rule.verbs.contains(&"*".to_string()) {
                continue;
            }
            
            // All checks passed - permission granted
            return Ok(true);
        }
        
        Ok(false)
    }
    
    fn parse_permission(&self, permission: &str) -> Result<(String, String, String)> {
        let parts: Vec<&str> = permission.split('.').collect();
        
        match parts.len() {
            2 => {
                // Format: "resource.verb" (e.g., "session.create")
                Ok(("api".to_string(), parts[0].to_string(), parts[1].to_string()))
            },
            3 => {
                // Format: "api_group.resource.verb" (e.g., "rbac.role.create")
                Ok((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
            },
            _ => Err(anyhow::anyhow!("Invalid permission format: {}", permission))
        }
    }
}
```

### Middleware Integration

**Authentication Middleware:**
```rust
pub async fn auth_middleware(
    mut req: Request<Body>,
    next: Next<Body>,
) -> Result<Response, ApiError> {
    // Extract JWT token from Authorization header
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    
    let token = auth_header
        .ok_or_else(|| ApiError::Unauthorized)?;
    
    // Validate and decode JWT
    let claims = decode::<RbacClaims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &Validation::default(),
    ).map_err(|_| ApiError::Unauthorized)?;
    
    // Create auth context
    let auth_context = AuthContext {
        principal: claims.claims.sub,
        subject_type: claims.claims.sub_type,
        rbac_engine: rbac_engine.clone(),
    };
    
    // Add to request extensions
    req.extensions_mut().insert(auth_context);
    
    Ok(next.run(req).await)
}
```

**RBAC Enforcement:**
```rust
pub struct AuthContext {
    pub principal: String,
    pub subject_type: SubjectType,
    pub rbac_engine: Arc<RbacEngine>,
}

impl AuthContext {
    pub async fn check_permission(&self, permission: &str) -> Result<(), ApiError> {
        // Check RBAC permission
        let allowed = self.rbac_engine
            .check_permission(&self.principal, self.subject_type, permission)
            .await
            .map_err(|e| ApiError::Internal(e))?;
        
        if !allowed {
            return Err(ApiError::Forbidden(format!(
                "Principal '{}' does not have permission '{}'",
                self.principal, permission
            )));
        }
        
        Ok(())
    }
    
    pub fn principal_name(&self) -> String {
        self.principal.clone()
    }
}
```

## Predefined Roles

### Admin Role

**Full System Access:**
```rust
let admin_role = Role {
    name: "admin".to_string(),
    description: Some("Full system administrator access".to_string()),
    rules: vec![
        Rule {
            api_groups: vec!["*".to_string()],
            resources: vec!["*".to_string()],
            verbs: vec!["*".to_string()],
            resource_names: None,
        }
    ],
    // ... metadata
};
```

### Developer Role

**Read/Write Application Resources:**
```rust
let developer_role = Role {
    name: "developer".to_string(),
    description: Some("Developer access to Host session resources".to_string()),
    rules: vec![
        // Host session management
        Rule {
            api_groups: vec!["api".to_string()],
            resources: vec!["session".to_string()],
            verbs: vec!["list".to_string(), "get".to_string(), "create".to_string(), "delete".to_string()],
            resource_names: None,
        },
    ],
    // ... metadata
};
```

### Viewer Role

**Read-Only Access:**
```rust
let viewer_role = Role {
    name: "viewer".to_string(),
    description: Some("Read-only access to Host sessions".to_string()),
    rules: vec![
        Rule {
            api_groups: vec!["api".to_string()],
            resources: vec!["session".to_string()],
            verbs: vec!["list".to_string(), "get".to_string()],
            resource_names: None,
        }
    ],
    // ... metadata
};
```

### Operator Roles

**CI/CD Operator:**
```rust
let automation_role = Role {
    name: "automation".to_string(),
    description: Some("Automation service access".to_string()),
    rules: vec![
        // Host session management for automation
        Rule {
            api_groups: vec!["api".to_string()],
            resources: vec!["session".to_string()],
            verbs: vec!["create".to_string(), "update".to_string(), "delete".to_string()],
            resource_names: None,
        },
    ],
    // ... metadata
};
```

## RBAC Management Operations

### Creating Roles

**CLI Request:**
```bash
raworc api roles -m post -b '{
  "name": "data-analyst",
  "description": "Data analysis team access",
  "rules": [
    {
      "api_groups": ["api"],
      "resources": ["session"],
      "verbs": ["list", "get", "create", "delete"]
    },
    {
      "api_groups": ["api"],
      "resources": ["sessions"],
      "verbs": ["list", "get"]
    }
  ]
}'
```

### Creating Role Bindings

**Bind User to Role:**
```bash
raworc api role-bindings -m post -b '{
    "name": "john-data-analyst",
      "role_name": "data-analyst",
    "subject_name": "john@company.com",
    "subject_type": "Subject"
  }'
```

**Bind Operator to Role:**
```bash
raworc api role-bindings -m post -b '{
    "name": "backup-service-binding",
      "role_name": "backup-operator",
    "subject_name": "backup-service",
    "subject_type": "Operator"
  }'
```

### Managing Operators

**Create Operator:**
```bash
raworc api operators -m post -b '{
    "name": "monitoring-agent",
      "description": "Operator for monitoring and alerting"
  }'
```

**Generate Operator Token:**
```bash
raworc api operators/monitoring-agent/token -m post -b '{
      "description": "Token for monitoring dashboard"
  }'
```

## Security Best Practices

### Principle of Least Privilege

**Role Design:**
```rust
// Good: Specific permissions
let specific_role = Role {
    rules: vec![
        Rule {
            api_groups: vec!["api".to_string()],
            resources: vec!["session".to_string()],
            verbs: vec!["list".to_string(), "get".to_string()],
            resource_names: None,
        }
    ],
    // ...
};

// Avoid: Overly broad permissions
let broad_role = Role {
    rules: vec![
        Rule {
            api_groups: vec!["*".to_string()],
            resources: vec!["*".to_string()],
            verbs: vec!["*".to_string()],
            resource_names: None,
        }
    ],
    // ...
};
```

### Token Management

**Token Rotation:**
```rust
// Planned: Automatic token rotation
pub struct TokenRotationService {
    rbac_engine: Arc<RbacEngine>,
    rotation_interval: Duration,
}

impl TokenRotationService {
    async fn rotate_operator_tokens(&self) -> Result<()> {
        let expired_tokens = self.get_expired_tokens().await?;
        
        for token in expired_tokens {
            // Generate new token
            let new_token = self.generate_new_token(&token.operator).await?;
            
            // Revoke old token
            self.revoke_token(&token.token_id).await?;
            
            // Notify services of new token
            self.notify_token_rotation(&token.operator, &new_token).await?;
        }
        
        Ok(())
    }
}
```

### Audit Logging

**RBAC Operations Audit:**
```rust
async fn audit_rbac_operation(&self, operation: &str, resource: &str, principal: &str, success: bool) -> Result<()> {
    sqlx::query!(r#"
        INSERT INTO audit_logs (
            timestamp, action, resource_type, resource_id, principal, success, metadata
        ) VALUES (NOW(), ?, 'rbac', ?, ?, ?, ?)
    "#,
        operation,
        resource,
        principal,
        success,
        serde_json::json!({
            "operation": operation,
            "resource": resource,
            "success": success
        }).to_string()
    ).execute(&self.pool).await?;
    
    Ok(())
}
```

## Common RBAC Patterns

### Multi-Environment Setup

**Environment-Specific Roles:**
```bash
# Production environment - restricted access
raworc api roles -m post -b '{
  "name": "prod-operator",
  "rules": [
    {
      "api_groups": ["api"],
      "resources": ["session"],
      "verbs": ["list", "get"]
    }
  ]
}'

# Development environment - broader access
raworc api roles -m post -b '{
  "name": "dev-user",
 
  "rules": [
    {
      "api_groups": ["api"],
      "resources": ["*"],
      "verbs": ["*"]
    }
  ]
}'
```

### Team-Based Access

**Team Role Structure:**
```bash
# Data team role
raworc api roles -m post -b '{
  "name": "data-team",
  "rules": [
    {
      "api_groups": ["api"],
      "resources": ["sessions"],
      "verbs": ["list", "get", "create", "delete"]
    }
  ]
}'

# Backend team role  
raworc api roles -m post -b '{
  "name": "backend-team",
  "rules": [
    {
      "api_groups": ["api"],
      "resources": ["sessions", "session_messages"],
      "verbs": ["*"]
    }
  ]
}'
```

## Future Enhancements

### Planned Features
- **OIDC Integration**: External identity provider support
- **Dynamic Permissions**: Context-aware permission evaluation
- **Permission Inheritance**: Hierarchical permission structures
- **Audit Dashboard**: Real-time RBAC monitoring and reporting

### Advanced Capabilities
- **Attribute-Based Access Control (ABAC)**: Policy-based permissions
- **Just-In-Time Access**: Temporary elevated permissions
- **Permission Analytics**: Usage patterns and optimization
- **Multi-Factor Authentication**: Enhanced security for sensitive operations
- **Session-Based Permissions**: Dynamic permissions based on session context