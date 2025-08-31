use crate::shared::models::{AppState, DatabaseError};
use crate::shared::rbac::{Role, RoleBinding, ServiceAccount, SubjectType};
use chrono::Utc;
use std::sync::Arc;
use sqlx::{query, Row};
use tracing::{error, info, warn};

impl AppState {
    // RBAC Operations
    // Service Account operations
    pub async fn create_service_account(
        &self,
        user: &str,
        space: Option<String>,
        pass_hash: &str,
        description: Option<String>,
    ) -> Result<ServiceAccount, DatabaseError> {
        let created_at = Utc::now().to_rfc3339();
        
        query(r#"
            INSERT INTO service_accounts (name, space, password_hash, description)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(user)
        .bind(space)
        .bind(pass_hash)
        .bind(&description)
        .execute(&*self.db)
        .await?;

        Ok(ServiceAccount {
            id: None,
            user: user.to_string(),
            pass_hash: pass_hash.to_string(),
            description,
            created_at: created_at.clone(),
            updated_at: created_at,
            active: true,
            last_login_at: None,
        })
    }

    pub async fn get_service_account(
        &self,
        user: &str,
    ) -> Result<Option<ServiceAccount>, DatabaseError> {
        tracing::debug!("Fetching service account for user: {}", user);
        
        let row = query(r#"
            SELECT name, password_hash, description, created_at, updated_at, active, last_login_at
            FROM service_accounts
            WHERE name = ?
            "#
        )
        .bind(user)
        .fetch_optional(&*self.db)
        .await
        .map_err(|e| {
            tracing::error!("SQL error fetching service account {}: {:?}", user, e);
            DatabaseError::from(e)
        })?;

        Ok(row.map(|r| {
            tracing::debug!("Found service account, mapping fields");
            ServiceAccount {
                id: None,
                user: r.get("name"),
                pass_hash: r.get("password_hash"),
                description: r.get("description"),
                created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
                active: r.get("active"),
                last_login_at: r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_login_at")
                    .map(|dt| dt.to_rfc3339()),
            }
        }))
    }

    pub async fn get_all_service_accounts(&self) -> Result<Vec<ServiceAccount>, DatabaseError> {
        let rows = sqlx::query(r#"
            SELECT name, password_hash, description, created_at, updated_at, active, last_login_at
            FROM service_accounts
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.db)
        .await?;

        Ok(rows.into_iter().map(|r| ServiceAccount {
            id: None,
            user: r.get("name"),
            pass_hash: r.get("password_hash"),
            description: r.get("description"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            active: r.get("active"),
            last_login_at: r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_login_at")
                .map(|dt| dt.to_rfc3339()),
        }).collect())
    }

    pub async fn delete_service_account(
        &self,
        user: &str,
    ) -> Result<bool, DatabaseError> {
        let result = query(r#"
            DELETE FROM service_accounts
            WHERE name = ?
            "#
        )
        .bind(user)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Removed: delete_service_account_by_id - we use name as primary key now

    pub async fn update_service_account_password(
        &self,
        user: &str,
        new_pass_hash: &str,
    ) -> Result<bool, DatabaseError> {
        let result = query(r#"
            UPDATE service_accounts
            SET password_hash = ?, updated_at = NOW()
            WHERE name = ?
            "#
        )
        .bind(new_pass_hash)
        .bind(user)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Removed: update_service_account_password_by_id - we use name as primary key now

    pub async fn update_service_account(
        &self,
        name: &str,
        space: Option<String>,
        description: Option<String>,
        active: Option<bool>,
    ) -> Result<bool, DatabaseError> {
        // Build dynamic update query based on provided fields
        let result = if let (Some(ns), Some(desc), Some(act)) = (&space, &description, &active) {
            query(r#"
                UPDATE service_accounts
                SET space = ?, description = ?, active = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(ns)
            .bind(desc)
            .bind(act)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let (Some(ns), Some(desc)) = (&space, &description) {
            query(r#"
                UPDATE service_accounts
                SET space = ?, description = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(ns)
            .bind(desc)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let (Some(ns), Some(act)) = (&space, &active) {
            query(r#"
                UPDATE service_accounts
                SET space = ?, active = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(ns)
            .bind(act)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let (Some(desc), Some(act)) = (&description, &active) {
            query(r#"
                UPDATE service_accounts
                SET description = ?, active = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(desc)
            .bind(act)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let Some(ns) = space {
            query(r#"
                UPDATE service_accounts
                SET space = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(ns)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let Some(desc) = description {
            query(r#"
                UPDATE service_accounts
                SET description = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(desc)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else if let Some(act) = active {
            query(r#"
                UPDATE service_accounts
                SET active = ?, updated_at = NOW()
                WHERE name = ?
                "#
            )
            .bind(act)
            .bind(name)
            .execute(&*self.db)
            .await?
        } else {
            // No fields to update
            return Ok(false);
        };
        
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_last_login(
        &self,
        user: &str,
    ) -> Result<bool, DatabaseError> {
        let result = query(r#"
            UPDATE service_accounts
            SET last_login_at = NOW()
            WHERE name = ?
            "#
        )
        .bind(user)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Role operations
    pub async fn create_role(&self, role: &Role) -> Result<Role, DatabaseError> {
        let rules_json = serde_json::to_value(&role.rules)?;
        
        query(r#"
            INSERT INTO roles (name, rules, description)
            VALUES (?, ?, ?)
            "#
        )
        .bind(&role.name)
        .bind(&rules_json)
        .bind(&role.description)
        .execute(&*self.db)
        .await?;

        Ok(Role {
            id: None,
            ..role.clone()
        })
    }

    pub async fn get_role(
        &self,
        name: &str,
    ) -> Result<Option<Role>, DatabaseError> {
        let row = query(r#"
            SELECT name, rules, description, created_at
            FROM roles
            WHERE name = ?
            "#
        )
        .bind(name)
        .fetch_optional(&*self.db)
        .await?;

        Ok(row.map(|r| Role {
            id: None,
            name: r.get("name"),
            rules: serde_json::from_value(r.get("rules")).unwrap_or_default(),
            description: r.get("description"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
        }))
    }

    pub async fn get_all_roles(&self) -> Result<Vec<Role>, DatabaseError> {
        let rows = query(r#"
            SELECT name, rules, description, created_at
            FROM roles
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.db)
        .await?;

        Ok(rows.into_iter().map(|r| Role {
            id: None,
            name: r.get("name"),
            rules: serde_json::from_value(r.get("rules")).unwrap_or_default(),
            description: r.get("description"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
        }).collect())
    }

    pub async fn delete_role(
        &self,
        name: &str,
    ) -> Result<bool, DatabaseError> {
        let result = query(r#"
            DELETE FROM roles
            WHERE name = ?
            "#
        )
        .bind(name)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Role Binding operations
    pub async fn create_role_binding(
        &self,
        role_binding: &RoleBinding,
    ) -> Result<RoleBinding, DatabaseError> {
        // Convert SubjectType enum to string for database
        let principal_type_str = match role_binding.principal_type {
            SubjectType::ServiceAccount => "ServiceAccount",
            SubjectType::Subject => "User",
        };
        
        // Use '*' for global space (NULL -> '*')
        let space = role_binding.space.as_deref().unwrap_or("*");
        
        query(r#"
            INSERT INTO role_bindings (role_name, principal, principal_type, space_id)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(&role_binding.role_name)
        .bind(&role_binding.principal)
        .bind(principal_type_str)
        .bind(space)
        .execute(&*self.db)
        .await?;

        Ok(RoleBinding {
            id: None,
            ..role_binding.clone()
        })
    }

    pub async fn get_role_binding(
        &self,
        role_name: &str,
        space: Option<&str>,
    ) -> Result<Option<RoleBinding>, DatabaseError> {
        let ws = space.unwrap_or("*");
        let row = query(r#"
            SELECT role_name, principal, principal_type, space_id, created_at
            FROM role_bindings
            WHERE role_name = ? AND space_id = ?
            LIMIT 1
            "#
        )
        .bind(role_name)
        .bind(ws)
        .fetch_optional(&*self.db)
        .await?;

        Ok(row.map(|r| {
            let principal_type_str: String = r.get("principal_type");
            let principal_type = match principal_type_str.as_str() {
                "ServiceAccount" => SubjectType::ServiceAccount,
                _ => SubjectType::Subject,
            };
            
            let space_str: String = r.get("space_id");
            RoleBinding {
                id: None,
                role_name: r.get("role_name"),
                principal: r.get("principal"),
                principal_type,
                space: if space_str == "*" { None } else { Some(space_str) },
                created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            }
        }))
    }

    pub async fn get_all_role_bindings(&self) -> Result<Vec<RoleBinding>, DatabaseError> {
        let rows = query(r#"
            SELECT role_name, principal, principal_type, space_id, created_at
            FROM role_bindings
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.db)
        .await?;

        Ok(rows.into_iter().map(|r| {
            let principal_type_str: String = r.get("principal_type");
            let principal_type = match principal_type_str.as_str() {
                "ServiceAccount" => SubjectType::ServiceAccount,
                _ => SubjectType::Subject,
            };
            
            let space_str: String = r.get("space_id");
            RoleBinding {
                id: None,
                role_name: r.get("role_name"),
                principal: r.get("principal"),
                principal_type,
                space: if space_str == "*" { None } else { Some(space_str) },
                created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            }
        }).collect())
    }

    #[allow(dead_code)]
    pub async fn get_role_bindings_for_subject(
        &self,
        subject_name: &str,
        subject_type: SubjectType,
        space: Option<&str>,
    ) -> Result<Vec<RoleBinding>, DatabaseError> {
        let principal_type_str = match subject_type {
            SubjectType::Subject => "User",
            SubjectType::ServiceAccount => "ServiceAccount",
        };
        
        let rows = if let Some(ns) = space {
            query(r#"
                SELECT role_name, principal, principal_type, space_id, created_at
                FROM role_bindings
                WHERE principal = ?
                AND principal_type = ?
                AND (space_id = ? OR space_id = '*')
                "#
            )
            .bind(subject_name)
            .bind(principal_type_str)
            .bind(ns)
            .fetch_all(&*self.db)
            .await?
        } else {
            query(r#"
                SELECT role_name, principal, principal_type, space_id, created_at
                FROM role_bindings
                WHERE principal = ?
                AND principal_type = ?
                "#
            )
            .bind(subject_name)
            .bind(principal_type_str)
            .fetch_all(&*self.db)
            .await?
        };

        Ok(rows.into_iter().map(|r| {
            let principal_type_str: String = r.get("principal_type");
            let principal_type = match principal_type_str.as_str() {
                "ServiceAccount" => SubjectType::ServiceAccount,
                _ => SubjectType::Subject,
            };
            
            let space_str: String = r.get("space_id");
            RoleBinding {
                id: None,
                role_name: r.get("role_name"),
                principal: r.get("principal"),
                principal_type,
                space: if space_str == "*" { None } else { Some(space_str) },
                created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            }
        }).collect())
    }

    pub async fn delete_role_binding(
        &self,
        name: &str,
        space: Option<&str>,
    ) -> Result<bool, DatabaseError> {
        let ws = space.unwrap_or("*");
        let result = query(r#"
            DELETE FROM role_bindings
            WHERE role_name = ? AND space_id = ?
            "#
        )
        .bind(name)
        .bind(ws)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

// Database connection utilities
pub async fn init_database(
    database_url: &str,
    jwt_secret: String,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Initializing database connection");
    
    let db = Arc::new(
        sqlx::MySqlPool::connect(database_url)
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to database: {}", e);
                e
            })?
    );

    tracing::info!("Database connected, running migrations");
    
    // Skip migrations if env var is set
    if std::env::var("SKIP_MIGRATIONS").is_ok() {
        info!("Skipping migrations (SKIP_MIGRATIONS set)");
    } else {
        match sqlx::migrate!("./db/migrations").run(&*db).await {
            Ok(_) => info!("Database migrations completed"),
            Err(e) => {
                error!("Migration failed: {}", e);
                if e.to_string().contains("applied before") || e.to_string().contains("Dirty database") {
                    warn!("Migration already applied or dirty state, continuing...");
                    
                    // Check if tables exist anyway
                    let table_check = sqlx::query_scalar::<_, i64>(
                        "SELECT COUNT(*) FROM information_schema.tables 
                         WHERE table_schema = DATABASE() AND table_name = 'sessions'"
                    )
                    .fetch_one(&*db)
                    .await
                    .unwrap_or(0);
                    
                    if table_check == 0 {
                        error!("Database tables do not exist and migrations failed");
                        return Err(Box::new(e));
                    }
                    info!("Tables exist, continuing despite migration error");
                }
            }
        }
    }

    Ok(AppState {
        db,
        jwt_secret,
    })
}

