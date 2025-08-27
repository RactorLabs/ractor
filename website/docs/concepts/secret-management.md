---
sidebar_position: 5
title: Secret Management
---

# Secret Management System

Raworc provides a comprehensive secret management system for securely storing and accessing sensitive information like API keys, database credentials, and configuration values. Secrets are scoped to spaces and automatically injected into session containers as environment variables.

## Architecture Overview

### Secret Management Components

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Secret Management Flow                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐  │
│  │   REST API      │────│     MySQL       │────│ Session Container│  │
│  │ (CRUD Secrets)  │    │ (Encrypted      │    │ (Environment    │  │
│  │                 │    │  Storage)       │    │  Variables)     │  │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘  │
│           │                       │                       │         │
│           │                       │                       │         │
│    ┌──────▼─────┐         ┌───────▼──────┐       ┌────────▼───────┐ │
│    │    RBAC    │         │ Encryption   │       │  Host Agent    │ │
│    │Permissions │         │    Layer     │       │   Access       │ │
│    └────────────┘         └──────────────┘       └────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Components:**
- **REST API**: CRUD operations with RBAC enforcement
- **MySQL Storage**: Encrypted secret values with metadata
- **Injection System**: Automatic environment variable injection
- **RBAC Layer**: Permission-based access control
- **Encryption**: Secure storage and transmission (planned)

## Secret Storage Schema

### Database Design

**Secrets Table:**
```sql
CREATE TABLE space_secrets (
    space VARCHAR(255) NOT NULL,
    key_name VARCHAR(255) NOT NULL,
    encrypted_value TEXT NOT NULL,  -- TODO: Implement actual encryption
    description TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    created_by VARCHAR(255) NOT NULL,
    PRIMARY KEY (space, key_name),
    CONSTRAINT fk_space_secrets_space FOREIGN KEY (space) REFERENCES spaces(name) ON DELETE CASCADE,
    INDEX idx_space_secrets_space (space),
    INDEX idx_space_secrets_created_at (created_at)
);
```

**Secret Metadata:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSecret {
    pub space: String,
    pub key_name: String,
    pub encrypted_value: String,  // Currently plaintext, encryption planned
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}
```

### Current Implementation Status

**Encryption Status:**
- **Current**: Values stored as plaintext in `encrypted_value` field
- **Planned**: AES-256 encryption with space-specific keys
- **Key Management**: Key derivation from master secret planned

```rust
// Current implementation (plaintext)
async fn store_secret(&self, space: &str, key_name: &str, value: &str) -> Result<()> {
    sqlx::query!(r#"
        INSERT INTO space_secrets (space, key_name, encrypted_value, created_by)
        VALUES (?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE 
            encrypted_value = VALUES(encrypted_value),
            updated_at = NOW()
    "#, space, key_name, value, created_by)
    .execute(&self.pool).await?;
    
    Ok(())
}

// Planned implementation (encrypted)
async fn store_secret_encrypted(&self, space: &str, key_name: &str, value: &str) -> Result<()> {
    let space_key = self.derive_space_key(space).await?;
    let encrypted_value = self.encrypt_value(value, &space_key)?;
    
    sqlx::query!(r#"
        INSERT INTO space_secrets (space, key_name, encrypted_value, created_by)
        VALUES (?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE 
            encrypted_value = VALUES(encrypted_value),
            updated_at = NOW()
    "#, space, key_name, encrypted_value, created_by)
    .execute(&self.pool).await?;
    
    Ok(())
}
```

## RBAC Permission Model

### Secret Permissions

Raworc implements fine-grained permissions for secret operations:

```rust
// Secret permission levels
pub const SPACE_SECRET_LIST: &str = "space.secret.list";           // View secret names
pub const SPACE_SECRET_GET: &str = "space.secret.get";             // View secret metadata
pub const SPACE_SECRET_READ_VALUES: &str = "space.secret.read_values"; // View actual values
pub const SPACE_SECRET_CREATE: &str = "space.secret.create";       // Create new secrets
pub const SPACE_SECRET_UPDATE: &str = "space.secret.update";       // Update existing secrets
pub const SPACE_SECRET_DELETE: &str = "space.secret.delete";       // Delete secrets
```

### Permission Enforcement

**API Endpoint Protection:**
```rust
async fn list_secrets(
    Extension(auth): Extension<AuthContext>,
    Path(space): Path<String>,
    Query(params): Query<ListSecretsQuery>,
) -> Result<Json<ListSecretsResponse>, ApiError> {
    // Check list permission
    auth.check_permission(&space, permissions::SPACE_SECRET_LIST).await?;
    
    let secrets = if params.show_values.unwrap_or(false) {
        // Viewing values requires additional permission
        auth.check_permission(&space, permissions::SPACE_SECRET_READ_VALUES).await?;
        secret_service.list_secrets_with_values(&space).await?
    } else {
        // Just metadata
        secret_service.list_secrets_metadata(&space).await?
    };
    
    Ok(Json(ListSecretsResponse { secrets }))
}
```

**Role-Based Access Examples:**
```rust
// Developer role - read-only access
let developer_rules = vec![
    Rule {
        api_groups: vec!["api".to_string()],
        resources: vec!["space.secret".to_string()],
        verbs: vec!["list".to_string(), "get".to_string()],
        resource_names: None,
    }
];

// Admin role - full access
let admin_rules = vec![
    Rule {
        api_groups: vec!["api".to_string()],
        resources: vec!["space.secret".to_string()],
        verbs: vec!["*".to_string()], // All operations
        resource_names: None,
    }
];

// Service account - specific secrets only
let service_rules = vec![
    Rule {
        api_groups: vec!["api".to_string()],
        resources: vec!["space.secret".to_string()],
        verbs: vec!["get".to_string(), "read_values".to_string()],
        resource_names: Some(vec!["API_KEY".to_string(), "DATABASE_URL".to_string()]),
    }
];
```

## Secret Operations

### Creating Secrets

**CLI Command:**
```bash
raworc api spaces/default/secrets -m post -b '{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-your-actual-key",
  "description": "Claude API key for AI agent interactions"
}'
```

**Implementation:**
```rust
async fn create_secret(
    Extension(auth): Extension<AuthContext>,
    Path(space): Path<String>,
    Json(request): Json<CreateSecretRequest>,
) -> Result<Json<SecretResponse>, ApiError> {
    // Check create permission
    auth.check_permission(&space, permissions::SPACE_SECRET_CREATE).await?;
    
    // Validate inputs
    validate_secret_key_name(&request.key_name)?;
    validate_secret_value(&request.value)?;
    
    // Create secret
    let secret = secret_service.create_secret(
        &space,
        &request.key_name,
        &request.value,
        request.description.as_deref(),
        &auth.principal_name(),
    ).await?;
    
    // Return metadata (no value)
    Ok(Json(SecretResponse {
        space: secret.space,
        key_name: secret.key_name,
        description: secret.description,
        created_at: secret.created_at,
        updated_at: secret.updated_at,
        created_by: secret.created_by,
        value: None, // Never return in response
    }))
}
```

### Reading Secrets

**List Secret Names:**
```bash
raworc api spaces/default/secrets
```

**Response:**
```json
{
  "secrets": [
    {
      "space": "default",
      "key_name": "ANTHROPIC_API_KEY",
      "description": "Claude API key for AI agent interactions",
      "created_at": "2025-08-21T10:00:00Z",
      "updated_at": "2025-08-21T10:00:00Z",
      "created_by": "admin"
    },
    {
      "space": "default", 
      "key_name": "DATABASE_URL",
      "description": "Production database connection string",
      "created_at": "2025-08-21T10:05:00Z",
      "updated_at": "2025-08-21T10:05:00Z",
      "created_by": "admin"
    }
  ]
}
```

**Get Secret with Value:**
```bash
raworc api "spaces/default/secrets/ANTHROPIC_API_KEY?show_values=true"
```

**Response:**
```json
{
  "space": "default",
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-api03-your-actual-key",
  "description": "Claude API key for AI agent interactions",
  "created_at": "2025-08-21T10:00:00Z",
  "updated_at": "2025-08-21T10:00:00Z",
  "created_by": "admin"
}
```

### Updating Secrets

**Update Secret Value:**
```bash
raworc api spaces/default/secrets/ANTHROPIC_API_KEY -m put -b '{
  "value": "sk-ant-api03-new-rotated-key",
  "description": "Rotated Claude API key"
}'
```

**Implementation:**
```rust
async fn update_secret(
    Extension(auth): Extension<AuthContext>,
    Path((space, key_name)): Path<(String, String)>,
    Json(request): Json<UpdateSecretRequest>,
) -> Result<Json<SecretResponse>, ApiError> {
    // Check update permission
    auth.check_permission(&space, permissions::SPACE_SECRET_UPDATE).await?;
    
    // Check if secret exists
    let existing = secret_service.get_secret(&space, &key_name).await
        .map_err(|_| ApiError::NotFound(format!("Secret {} not found", key_name)))?;
    
    // Update secret
    let updated_secret = secret_service.update_secret(
        &space,
        &key_name,
        request.value.as_deref(),
        request.description.as_deref(),
    ).await?;
    
    Ok(Json(SecretResponse::from_secret(updated_secret, false)))
}
```

### Deleting Secrets

**Delete Secret:**
```bash
raworc api spaces/default/secrets/ANTHROPIC_API_KEY -m delete
```

**Implementation:**
```rust
async fn delete_secret(
    Extension(auth): Extension<AuthContext>,
    Path((space, key_name)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    // Check delete permission
    auth.check_permission(&space, permissions::SPACE_SECRET_DELETE).await?;
    
    // Delete secret
    secret_service.delete_secret(&space, &key_name).await?;
    
    Ok(StatusCode::NO_CONTENT)
}
```

## Secret Injection

### Environment Variable Injection

Secrets are automatically injected into session containers as environment variables:

```rust
async fn get_space_secrets_for_injection(&self, space: &str) -> Result<Vec<String>> {
    let secrets = sqlx::query!(r#"
        SELECT key_name, encrypted_value 
        FROM space_secrets 
        WHERE space = ?
    "#, space).fetch_all(&self.pool).await?;
    
    let mut env_vars = Vec::new();
    for secret in secrets {
        // TODO: Implement actual decryption
        let decrypted_value = self.decrypt_value(&secret.encrypted_value).await?;
        env_vars.push(format!("{}={}", secret.key_name, decrypted_value));
    }
    
    Ok(env_vars)
}
```

### Container Creation with Secrets

```rust
async fn create_session_container(&self, session_id: &str, space: &str) -> Result<String> {
    // Get space secrets
    let secret_env_vars = self.get_space_secrets_for_injection(space).await?;
    
    // Combine with system environment variables
    let mut env_vars = vec![
        format!("RAWORC_SESSION_ID={}", session_id),
        format!("RAWORC_SPACE={}", space),
        format!("RAWORC_API_URL=http://raworc_server:9000"),
    ];
    env_vars.extend(secret_env_vars);
    
    // Create container with secrets as environment variables
    let container_config = bollard::container::Config {
        image: Some(space_image),
        env: Some(env_vars),
        // ... other configuration
    };
    
    self.docker.create_container(
        Some(CreateContainerOptions { name: container_name }),
        container_config
    ).await?;
    
    Ok(container_name)
}
```

### Host Agent Access

Within session containers, agents can access secrets through environment variables:

**Python Agent Example:**
```python
import os

def process_message(message: str, context: dict) -> str:
    # Access injected secrets
    api_key = os.getenv('ANTHROPIC_API_KEY')
    db_url = os.getenv('DATABASE_URL')
    
    if not api_key:
        return "Error: ANTHROPIC_API_KEY not configured"
    
    # Use secrets in agent logic
    client = anthropic.Anthropic(api_key=api_key)
    # ... agent implementation
```

**Rust Agent Example:**
```rust
use std::env;

pub fn process_message_sync(message: &str, context: &serde_json::Value) -> String {
    // Access injected secrets
    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable not set");
    
    let db_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable not set");
    
    // Use secrets in agent logic
    // ... agent implementation
}
```

**Node.js Agent Example:**
```javascript
function processMessage(message, context) {
    // Access injected secrets
    const apiKey = process.env.ANTHROPIC_API_KEY;
    const dbUrl = process.env.DATABASE_URL;
    
    if (!apiKey) {
        return "Error: ANTHROPIC_API_KEY not configured";
    }
    
    // Use secrets in agent logic
    // ... agent implementation
}

module.exports = { processMessage };
```

## Security Considerations

### Current Security Model

**Storage Security:**
```rust
// Current implementation - plaintext storage
pub struct SecretService {
    pool: sqlx::MySqlPool,
    // TODO: Add encryption key management
}

impl SecretService {
    // Current: values stored as plaintext
    async fn store_secret_value(&self, value: &str) -> Result<String> {
        // TODO: Implement encryption
        Ok(value.to_string())
    }
    
    // Current: values retrieved as plaintext
    async fn retrieve_secret_value(&self, encrypted_value: &str) -> Result<String> {
        // TODO: Implement decryption
        Ok(encrypted_value.to_string())
    }
}
```

### Planned Security Enhancements

**Encryption Implementation:**
```rust
// Planned implementation with encryption
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::Rng;

pub struct SecretService {
    pool: sqlx::MySqlPool,
    master_key: Key<Aes256Gcm>,
}

impl SecretService {
    async fn encrypt_secret_value(&self, value: &str, space: &str) -> Result<String> {
        let space_key = self.derive_space_key(space).await?;
        let cipher = Aes256Gcm::new(&space_key);
        
        let nonce = self.generate_nonce();
        let ciphertext = cipher.encrypt(&nonce, value.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
        // Combine nonce + ciphertext for storage
        let mut encrypted_data = nonce.to_vec();
        encrypted_data.extend_from_slice(&ciphertext);
        
        Ok(base64::encode(&encrypted_data))
    }
    
    async fn decrypt_secret_value(&self, encrypted_data: &str, space: &str) -> Result<String> {
        let space_key = self.derive_space_key(space).await?;
        let cipher = Aes256Gcm::new(&space_key);
        
        let data = base64::decode(encrypted_data)?;
        let (nonce, ciphertext) = data.split_at(12); // 96-bit nonce
        
        let plaintext = cipher.decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
        
        Ok(String::from_utf8(plaintext)?)
    }
    
    async fn derive_space_key(&self, space: &str) -> Result<Key<Aes256Gcm>> {
        // Derive space-specific key from master key using HKDF
        use hkdf::Hkdf;
        use sha2::Sha256;
        
        let hk = Hkdf::<Sha256>::new(None, &self.master_key);
        let mut space_key = [0u8; 32];
        hk.expand(space.as_bytes(), &mut space_key)
            .map_err(|e| anyhow::anyhow!("Key derivation failed: {}", e))?;
        
        Ok(Key::<Aes256Gcm>::from_slice(&space_key).clone())
    }
}
```

### Access Control

**Audit Logging:**
```rust
async fn log_secret_access(&self, action: &str, space: &str, key_name: &str, principal: &str) -> Result<()> {
    sqlx::query!(r#"
        INSERT INTO audit_logs (
            timestamp, action, resource_type, resource_id, principal, space, metadata
        ) VALUES (NOW(), ?, 'secret', ?, ?, ?, ?)
    "#, 
        action,
        format!("{}:{}", space, key_name),
        principal,
        space,
        serde_json::json!({
            "secret_key": key_name,
            "action": action
        }).to_string()
    ).execute(&self.pool).await?;
    
    Ok(())
}
```

**Rate Limiting:**
```rust
// Planned: Rate limiting for secret access
use governor::{Quota, RateLimiter};

pub struct SecretService {
    pool: sqlx::MySqlPool,
    rate_limiter: RateLimiter<String, _, _>, // Per-principal rate limiting
}

impl SecretService {
    async fn check_rate_limit(&self, principal: &str) -> Result<()> {
        if self.rate_limiter.check_key(principal).is_err() {
            return Err(anyhow::anyhow!("Rate limit exceeded for secret access"));
        }
        Ok(())
    }
}
```

## Common Use Cases

### API Key Management

**Setting up API Keys:**
```bash
# Set Claude API key
raworc api spaces/default/secrets -m post -b '{
  "key_name": "ANTHROPIC_API_KEY",
  "value": "sk-ant-api03-your-key",
  "description": "Claude API key for agent interactions"
}'

# Set OpenAI API key  
raworc api spaces/default/secrets -m post -b '{
  "key_name": "OPENAI_API_KEY", 
  "value": "sk-your-openai-key",
  "description": "OpenAI API key for GPT models"
}'

# Set custom service API key
raworc api spaces/default/secrets -m post -b '{
  "key_name": "CUSTOM_SERVICE_KEY",
  "value": "your-service-key",
  "description": "API key for custom service integration"
}'
```

### Database Credentials

**Database Connection Strings:**
```bash
# Production database
raworc api spaces/production/secrets -m post -b '{
  "key_name": "DATABASE_URL",
  "value": "mysql://user:password@prod-db:3306/app_prod",
  "description": "Production database connection"
}'

# Staging database
raworc api spaces/staging/secrets -m post -b '{
  "key_name": "DATABASE_URL", 
  "value": "mysql://user:password@staging-db:3306/app_staging",
  "description": "Staging database connection"
}'

# Redis connection
raworc api spaces/default/secrets -m post -b '{
  "key_name": "REDIS_URL",
  "value": "redis://redis-host:6379/0",
  "description": "Redis cache connection"
}'
```

### Environment Configuration

**Environment-Specific Settings:**
```bash
# Application configuration
raworc api spaces/default/secrets -m post -b '{
  "key_name": "APP_CONFIG",
  "value": "{\"debug\": false, \"log_level\": \"info\"}",
  "description": "Application configuration JSON"
}'

# Feature flags
raworc api spaces/default/secrets -m post -b '{
  "key_name": "FEATURE_FLAGS",
  "value": "new_ui=true,beta_features=false",
  "description": "Feature flag configuration"
}'
```

## Secret Rotation

### Manual Rotation

**Rotating API Keys:**
```bash
# Update existing secret with new value
raworc api spaces/default/secrets/ANTHROPIC_API_KEY -m put -b '{
  "value": "sk-ant-api03-new-rotated-key",
  "description": "Rotated Claude API key - 2025-08-21"
}'

# Verify the update
raworc api spaces/default/secrets/ANTHROPIC_API_KEY
```

### Rotation Best Practices

```rust
// Planned: Automatic rotation support
pub struct SecretRotationService {
    secret_service: SecretService,
    rotation_policies: HashMap<String, RotationPolicy>,
}

#[derive(Debug, Clone)]
pub struct RotationPolicy {
    pub rotation_interval: Duration,
    pub rotation_strategy: RotationStrategy,
    pub notification_webhook: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RotationStrategy {
    Manual,                    // Require manual intervention
    External(String),          // Call external API for new value
    Generated(GenerationRule), // Generate new value automatically
}

impl SecretRotationService {
    async fn check_rotation_needed(&self, space: &str, key_name: &str) -> Result<bool> {
        let secret = self.secret_service.get_secret(space, key_name).await?;
        let policy = self.rotation_policies.get(key_name);
        
        if let Some(policy) = policy {
            let age = chrono::Utc::now() - secret.updated_at;
            Ok(age > policy.rotation_interval)
        } else {
            Ok(false) // No rotation policy
        }
    }
}
```

## Monitoring and Audit

### Secret Usage Analytics

**Database Queries for Insights:**
```sql
-- Most frequently accessed secrets
SELECT s.space, s.key_name, COUNT(a.id) as access_count
FROM space_secrets s
LEFT JOIN audit_logs a ON a.resource_id = CONCAT(s.space, ':', s.key_name)
WHERE a.action = 'secret.read'
  AND a.timestamp > DATE_SUB(NOW(), INTERVAL 30 DAY)
GROUP BY s.space, s.key_name
ORDER BY access_count DESC;

-- Secrets by age
SELECT space, key_name, 
       DATEDIFF(NOW(), updated_at) as days_since_update,
       created_by
FROM space_secrets
ORDER BY updated_at ASC;

-- Recent secret operations
SELECT timestamp, action, resource_id, principal, space
FROM audit_logs
WHERE resource_type = 'secret'
  AND timestamp > DATE_SUB(NOW(), INTERVAL 7 DAY)
ORDER BY timestamp DESC;
```

### Security Monitoring

**Anomaly Detection (Planned):**
```rust
// Planned: Detect unusual secret access patterns
pub struct SecretSecurityMonitor {
    access_patterns: HashMap<String, AccessPattern>,
    alert_thresholds: SecurityThresholds,
}

#[derive(Debug)]
pub struct AccessPattern {
    pub principal: String,
    pub secret_key: String,
    pub access_times: Vec<DateTime<Utc>>,
    pub access_ips: HashSet<String>,
}

impl SecretSecurityMonitor {
    async fn analyze_access_pattern(&self, access: &SecretAccess) -> Result<Vec<SecurityAlert>> {
        let mut alerts = Vec::new();
        
        // Check for unusual access frequency
        if self.is_high_frequency_access(access).await? {
            alerts.push(SecurityAlert::HighFrequencyAccess {
                principal: access.principal.clone(),
                secret: access.secret_key.clone(),
                count: access.frequency,
            });
        }
        
        // Check for access from new IP addresses
        if self.is_new_ip_access(access).await? {
            alerts.push(SecurityAlert::NewIpAccess {
                principal: access.principal.clone(),
                secret: access.secret_key.clone(),
                ip: access.source_ip.clone(),
            });
        }
        
        // Check for off-hours access
        if self.is_off_hours_access(access).await? {
            alerts.push(SecurityAlert::OffHoursAccess {
                principal: access.principal.clone(),
                secret: access.secret_key.clone(),
                timestamp: access.timestamp,
            });
        }
        
        Ok(alerts)
    }
}
```

## Best Practices

### Secret Management
- Use descriptive names with consistent naming conventions
- Include meaningful descriptions for all secrets
- Regularly rotate sensitive credentials
- Monitor secret access patterns
- Implement proper RBAC permissions

### Security
- Never log secret values
- Use HTTPS for all API communications
- Implement proper encryption at rest
- Regular security audits of secret access
- Monitor for unauthorized access attempts

### Organization
- Group related secrets by space
- Use consistent naming conventions (e.g., `API_KEY_SERVICE_NAME`)
- Document secret dependencies in agent code
- Implement secret validation in agents
- Plan for secret rotation workflows

### Performance
- Cache secret metadata (not values) when possible
- Use connection pooling for database access
- Implement proper indexing for secret queries
- Monitor database performance under load
- Consider read replicas for high-frequency access

## Future Enhancements

### Planned Features
- **Encryption at Rest**: AES-256 encryption with key management
- **Secret Versioning**: Track secret value history
- **Automatic Rotation**: Policy-based rotation schedules
- **External Key Management**: Integration with AWS KMS, HashiCorp Vault
- **Secret Sharing**: Cross-space secret sharing with permissions

### Advanced Capabilities
- **Secret Templates**: Predefined secret configurations
- **Compliance Reporting**: Audit reports for compliance requirements
- **Integration APIs**: Webhook notifications for secret changes
- **Secret Scanning**: Detect secrets in code repositories
- **Dynamic Secrets**: Generate temporary credentials on-demand