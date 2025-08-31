use serde::{Deserialize, Serialize};
use uuid::Uuid;

// RBAC Subject - External user identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub name: String, // External subject identifier (e.g., "user@example.com", "system:serviceaccount:name")
}

// Service Account - Global account with credentials (can work across organizations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAccount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub user: String,
    pub pass_hash: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
    pub last_login_at: Option<String>,
}


// Permission Rule - Fine-grained access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub api_groups: Vec<String>,             // e.g., ["", "api", "rbac"]
    pub resources: Vec<String>,              // e.g., ["users", "roles", "*"]
    pub verbs: Vec<String>,                  // e.g., ["get", "list", "create", "update", "delete"]
    pub resource_names: Option<Vec<String>>, // Optional specific resource names
}

// Role - Global collection of permissions (can be bound to specific organizations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub name: String,
    pub rules: Vec<Rule>,
    pub description: Option<String>,
    pub created_at: String,
}


// Subject type for role bindings
#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum SubjectType {
    Subject,
    ServiceAccount,
}

// Role Binding Subject
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBindingSubject {
    pub kind: SubjectType,
    pub name: String,
}

// Role Binding - Links roles to subjects and specifies WHERE they apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBinding {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub role_name: String,
    pub principal: String,
    pub principal_type: SubjectType,
    pub created_at: String,
}


// Role Reference for bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRef {
    pub kind: String, // "Role" or "ClusterRole"
    pub name: String,
    pub api_group: String, // API group for permissions, typically "rbac"
}

// Authentication Principal - Represents authenticated entity
#[derive(Debug, Clone)]
pub enum AuthPrincipal {
    Subject(Subject),
    ServiceAccount(ServiceAccount),
}

impl AuthPrincipal {
    pub fn name(&self) -> &str {
        match self {
            AuthPrincipal::Subject(s) => &s.name,
            AuthPrincipal::ServiceAccount(sa) => &sa.user,
        }
    }

    pub fn subject_type(&self) -> SubjectType {
        match self {
            AuthPrincipal::Subject(_) => SubjectType::Subject,
            AuthPrincipal::ServiceAccount(_) => SubjectType::ServiceAccount,
        }
    }
}

// JWT Claims for RBAC authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacClaims {
    pub sub: String,               // Subject name
    pub sub_type: SubjectType,     // Subject type
    pub exp: usize,                // Expiration time
    pub iat: usize,                // Issued at
    pub iss: String,               // Issuer
}

// Input types removed - were unused

// Token generation response
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub expires_at: String,
}

// Permission check context
#[derive(Debug)]
pub struct PermissionContext {
    pub api_group: String,
    pub resource: String,
    pub verb: String,
}

// RBAC Authorization service
pub struct RbacAuthz;

impl RbacAuthz {
    pub fn has_permission(
        _principal: &AuthPrincipal,
        _roles: &[Role],
        _role_bindings: &[RoleBinding],
        _context: &PermissionContext,
    ) -> bool {
        // Simplified implementation - for now just return true
        // TODO: Implement proper RBAC when needed
        true
    }
}

