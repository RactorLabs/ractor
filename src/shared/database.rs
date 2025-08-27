use crate::shared::models::{AppState, DatabaseError, Space, SpaceSecretWithValue};
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

    // Space operations
    pub async fn get_space(&self, name: &str) -> Result<Option<Space>, DatabaseError> {
        let row = query(r#"
            SELECT name, description, settings, active, created_at, updated_at, created_by
            FROM spaces
            WHERE name = ?
            LIMIT 1
            "#
        )
        .bind(name)
        .fetch_optional(&*self.db)
        .await?;

        Ok(row.map(|r| Space {
            name: r.get("name"),
            description: r.get("description"),
            settings: serde_json::from_value(r.get("settings")).unwrap_or_default(),
            active: r.get("active"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            created_by: r.get("created_by"),
        }))
    }

    pub async fn get_all_spaces(&self) -> Result<Vec<Space>, DatabaseError> {
        let rows = query(r#"
            SELECT name, description, settings, active, created_at, updated_at, created_by
            FROM spaces
            WHERE active = true
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&*self.db)
        .await?;

        Ok(rows.into_iter().map(|r| Space {
            name: r.get("name"),
            description: r.get("description"),
            settings: serde_json::from_value(r.get("settings")).unwrap_or_default(),
            active: r.get("active"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            created_by: r.get("created_by"),
        }).collect())
    }

    pub async fn create_space(
        &self,
        name: &str,
        description: Option<&str>,
        settings: serde_json::Value,
        created_by: &str,
    ) -> Result<Space, DatabaseError> {
        query(r#"
            INSERT INTO spaces (name, description, settings, created_by)
            VALUES (?, ?, ?, ?)
            "#
        )
        .bind(name)
        .bind(description)
        .bind(&settings)
        .bind(created_by)
        .execute(&*self.db)
        .await?;

        // Return the created space
        self.get_space(name)
            .await?
            .ok_or(DatabaseError::Internal("Failed to retrieve created space".to_string()))
    }

    pub async fn update_space(
        &self,
        space: &str,
        new_name: Option<&str>,
        description: Option<&str>,
        settings: Option<serde_json::Value>,
        active: Option<bool>,
    ) -> Result<bool, DatabaseError> {
        let result = match (new_name, description, settings, active) {
            (Some(n), Some(d), Some(s), Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, description = ?, settings = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(d)
                .bind(&s)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), Some(d), Some(s), None) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, description = ?, settings = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(d)
                .bind(&s)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), Some(d), None, Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, description = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(d)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), None, Some(s), Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, settings = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(&s)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, Some(d), Some(s), Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET description = ?, settings = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(d)
                .bind(&s)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), Some(d), None, None) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(d)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), None, Some(s), None) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, settings = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(&s)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), None, None, Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, Some(d), Some(s), None) => {
                query(r#"
                    UPDATE spaces
                    SET description = ?, settings = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(d)
                .bind(&s)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, Some(d), None, Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET description = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(d)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, None, Some(s), Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET settings = ?, active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(&s)
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (Some(n), None, None, None) => {
                query(r#"
                    UPDATE spaces
                    SET name = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(n)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, Some(d), None, None) => {
                query(r#"
                    UPDATE spaces
                    SET description = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(d)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, None, Some(s), None) => {
                query(r#"
                    UPDATE spaces
                    SET settings = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(&s)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, None, None, Some(a)) => {
                query(r#"
                    UPDATE spaces
                    SET active = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE name = ?
                    "#
                )
                .bind(a)
                .bind(space)
                .execute(&*self.db)
                .await?
            }
            (None, None, None, None) => {
                // No fields to update
                return Ok(false);
            }
        };

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_space(&self, name: &str) -> Result<bool, DatabaseError> {
        let result = query(r#"
            DELETE FROM spaces
            WHERE name = ?
            "#
        )
        .bind(name)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // Space secrets operations
    pub async fn get_space_secrets(&self, space: &str) -> Result<Vec<SpaceSecretWithValue>, DatabaseError> {
        let rows = query(r#"
            SELECT space, key_name, encrypted_value, description, created_at, updated_at, created_by
            FROM space_secrets
            WHERE space = ?
            ORDER BY key_name
            "#
        )
        .bind(space)
        .fetch_all(&*self.db)
        .await?;

        Ok(rows.into_iter().map(|r| SpaceSecretWithValue {
            space: r.get("space"),
            key_name: r.get("key_name"),
            encrypted_value: r.get("encrypted_value"),
            description: r.get("description"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            created_by: r.get("created_by"),
        }).collect())
    }

    pub async fn get_space_secret(&self, space: &str, key_name: &str) -> Result<Option<SpaceSecretWithValue>, DatabaseError> {
        let row = query(r#"
            SELECT space, key_name, encrypted_value, description, created_at, updated_at, created_by
            FROM space_secrets
            WHERE space = ? AND key_name = ?
            LIMIT 1
            "#
        )
        .bind(space)
        .bind(key_name)
        .fetch_optional(&*self.db)
        .await?;

        Ok(row.map(|r| SpaceSecretWithValue {
            space: r.get("space"),
            key_name: r.get("key_name"),
            encrypted_value: r.get("encrypted_value"),
            description: r.get("description"),
            created_at: r.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
            updated_at: r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at").to_rfc3339(),
            created_by: r.get("created_by"),
        }))
    }

    pub async fn create_space_secret(
        &self,
        space: &str,
        key_name: &str,
        encrypted_value: &str,
        description: Option<&str>,
        created_by: &str,
    ) -> Result<SpaceSecretWithValue, DatabaseError> {
        query(r#"
            INSERT INTO space_secrets (space, key_name, encrypted_value, description, created_by)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(space)
        .bind(key_name)
        .bind(encrypted_value)
        .bind(description)
        .bind(created_by)
        .execute(&*self.db)
        .await?;

        // Return the created secret
        self.get_space_secret(space, key_name)
            .await?
            .ok_or(DatabaseError::Internal("Failed to retrieve created secret".to_string()))
    }

    pub async fn update_space_secret(
        &self,
        space: &str,
        key_name: &str,
        encrypted_value: Option<&str>,
        description: Option<&str>,
    ) -> Result<bool, DatabaseError> {
        let result = if let (Some(value), Some(desc)) = (encrypted_value, description) {
            query(r#"
                UPDATE space_secrets
                SET encrypted_value = ?, description = ?, updated_at = CURRENT_TIMESTAMP
                WHERE space = ? AND key_name = ?
                "#
            )
            .bind(value)
            .bind(desc)
            .bind(space)
            .bind(key_name)
            .execute(&*self.db)
            .await?
        } else if let Some(value) = encrypted_value {
            query(r#"
                UPDATE space_secrets
                SET encrypted_value = ?, updated_at = CURRENT_TIMESTAMP
                WHERE space = ? AND key_name = ?
                "#
            )
            .bind(value)
            .bind(space)
            .bind(key_name)
            .execute(&*self.db)
            .await?
        } else if let Some(desc) = description {
            query(r#"
                UPDATE space_secrets
                SET description = ?, updated_at = CURRENT_TIMESTAMP
                WHERE space = ? AND key_name = ?
                "#
            )
            .bind(desc)
            .bind(space)
            .bind(key_name)
            .execute(&*self.db)
            .await?
        } else {
            // No fields to update
            return Ok(false);
        };

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_space_secret(&self, space: &str, key_name: &str) -> Result<bool, DatabaseError> {
        let result = query(r#"
            DELETE FROM space_secrets
            WHERE space = ? AND key_name = ?
            "#
        )
        .bind(space)
        .bind(key_name)
        .execute(&*self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

// Database connection utilities
pub async fn init_database(
    database_url: &str,
    jwt_secret: String,
) -> Result<AppState, Box<dyn std::error::Error>> {
    use sqlx::mysql::MySqlPoolOptions;
    
    let db = Arc::new(
        MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?,
    );

    // Run migrations (skip if SKIP_MIGRATIONS is set)
    if std::env::var("SKIP_MIGRATIONS").is_err() {
        info!("Running database migrations...");
        
        // Run migrations from the migrations directory
        match sqlx::migrate!("./db/migrations").run(&*db).await {
            Ok(_) => info!("Database migrations completed successfully"),
            Err(e) => {
                // Check if this is just a "migrations already applied" error
                if e.to_string().contains("already applied") {
                    info!("Migrations already applied, continuing...");
                } else {
                    warn!("Migration error: {}", e);
                    
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
    } else {
        info!("Skipping migrations (SKIP_MIGRATIONS set)");
    }

    Ok(AppState {
        db,
        jwt_secret,
    })
}

