use serde::{Deserialize, Serialize};
use uuid::Uuid;

// RBAC Subject - External user identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub name: String, // External subject identifier (e.g., "user@example.com", "system:operator:name")
}

// Operator - Global account with credentials (can work across organizations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operator {
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
    Admin,
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
    Operator(Operator),
}

impl AuthPrincipal {
    pub fn name(&self) -> &str {
        match self {
            AuthPrincipal::Subject(s) => &s.name,
            AuthPrincipal::Operator(op) => &op.user,
        }
    }

    pub fn subject_type(&self) -> SubjectType {
        match self {
            AuthPrincipal::Subject(_) => SubjectType::Subject,
            AuthPrincipal::Operator(_) => SubjectType::Admin,
        }
    }
}

// JWT Claims for RBAC authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacClaims {
    pub sub: String,           // Subject name
    pub sub_type: SubjectType, // Subject type
    pub exp: usize,            // Expiration time
    pub iat: usize,            // Issued at
    pub iss: String,           // Issuer
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
        roles: &[Role],
        role_bindings: &[RoleBinding],
        context: &PermissionContext,
    ) -> bool {
        // Fast path: if no bindings, deny
        if role_bindings.is_empty() {
            return false;
        }

        // Build role lookup map by name
        use std::collections::HashMap;
        let role_map: HashMap<&str, &Role> = roles.iter().map(|r| (r.name.as_str(), r)).collect();

        // Helper for wildcard matching
        fn contains_match(list: &[String], needle: &str) -> bool {
            list.iter().any(|v| v == "*" || v.eq_ignore_ascii_case(needle))
        }

        // Evaluate each binding's role rules
        for rb in role_bindings {
            if let Some(role) = role_map.get(rb.role_name.as_str()) {
                for rule in &role.rules {
                    if contains_match(&rule.api_groups, &context.api_group)
                        && contains_match(&rule.resources, &context.resource)
                        && contains_match(&rule.verbs, &context.verb)
                    {
                        return true;
                    }
                }
            }
        }

        false
    }
}
