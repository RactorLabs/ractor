use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub space: String,
    pub description: Option<String>,
    pub purpose: Option<String>, // Space-specific purpose
    pub source_repo: String,
    pub source_branch: Option<String>, // defaults to 'main' if None
    pub status: String, // configured, building, running, stopped, error
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
}

impl FromRow<'_, sqlx::mysql::MySqlRow> for Agent {
    fn from_row(row: &sqlx::mysql::MySqlRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        Ok(Agent {
            name: row.try_get("name")?,
            space: row.try_get("space")?,
            description: row.try_get("description")?,
            purpose: row.try_get("purpose")?,
            source_repo: row.try_get("source_repo")?,
            source_branch: row.try_get("source_branch")?,
            status: row.try_get("status")?,
            created_at: row.try_get::<DateTime<Utc>, _>("created_at")?.to_rfc3339(),
            updated_at: row.try_get::<DateTime<Utc>, _>("updated_at")?.to_rfc3339(),
            created_by: row.try_get("created_by")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
    pub purpose: Option<String>, // Space-specific purpose
    pub source_repo: String,
    #[serde(default = "default_main_branch")]
    pub source_branch: Option<String>, // defaults to 'main' if None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentRequest {
    pub description: Option<String>,
    pub purpose: Option<String>,
    pub source_repo: Option<String>,
    pub source_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusUpdate {
    pub status: String,
}

fn default_main_branch() -> Option<String> {
    Some("main".to_string())
}

// Database queries
impl Agent {
    pub async fn find_all(pool: &sqlx::MySqlPool, space: Option<&str>) -> Result<Vec<Agent>, sqlx::Error> {
        let query = if let Some(ws_name) = space {
            sqlx::query_as::<_, Agent>(
                r#"
                SELECT name, space, description, purpose,
                       source_repo, source_branch,
                       status, created_at, updated_at, created_by
                FROM agents
                WHERE space = ?
                ORDER BY created_at DESC
                "#
            )
            .bind(ws_name)
        } else {
            sqlx::query_as::<_, Agent>(
                r#"
                SELECT name, space, description, purpose,
                       source_repo, source_branch,
                       status, created_at, updated_at, created_by
                FROM agents
                ORDER BY created_at DESC
                "#
            )
        };
        
        query.fetch_all(pool).await
    }

    pub async fn find_by_name(pool: &sqlx::MySqlPool, space: &str, name: &str) -> Result<Option<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, space, description, purpose,
                   source_repo, source_branch,
                   status, created_at, updated_at, created_by
            FROM agents
            WHERE space = ? AND name = ?
            "#
        )
        .bind(space)
        .bind(name)
        .fetch_optional(pool)
        .await
    }

    pub async fn create(
        pool: &sqlx::MySqlPool,
        space: &str,
        req: CreateAgentRequest,
        created_by: &str,
    ) -> Result<Agent, sqlx::Error> {
        // Check if agent name is unique within space
        if Self::find_by_name(pool, space, &req.name).await?.is_some() {
            return Err(sqlx::Error::Protocol(format!(
                "Agent with name '{}' already exists in space '{}'",
                req.name, space
            )));
        }

        sqlx::query(
            r#"
            INSERT INTO agents (
                name, space, description, purpose,
                source_repo, source_branch, status, created_by
            )
            VALUES (?, ?, ?, ?, ?, ?, 'configured', ?)
            "#
        )
        .bind(&req.name)
        .bind(space)
        .bind(&req.description)
        .bind(&req.purpose)
        .bind(&req.source_repo)
        .bind(&req.source_branch)
        .bind(created_by)
        .execute(pool)
        .await?;

        Self::find_by_name(pool, space, &req.name)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)
    }

    pub async fn update(
        pool: &sqlx::MySqlPool,
        space: &str,
        name: &str,
        req: UpdateAgentRequest,
    ) -> Result<Option<Agent>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE agents SET");
        let mut updates = Vec::new();

        if req.description.is_some() {
            updates.push(" description = ?");
        }
        if req.purpose.is_some() {
            updates.push(" purpose = ?");
        }
        if req.source_repo.is_some() {
            updates.push(" source_repo = ?");
        }
        if req.source_branch.is_some() {
            updates.push(" source_branch = ?");
        }

        if updates.is_empty() {
            return Err(sqlx::Error::Protocol("No fields to update".to_string()));
        }

        query_builder.push_str(&updates.join(","));
        query_builder.push_str(", updated_at = CURRENT_TIMESTAMP WHERE space = ? AND name = ?");

        let mut query = sqlx::query(&query_builder);

        if let Some(description) = req.description {
            query = query.bind(description);
        }
        if let Some(purpose) = req.purpose {
            query = query.bind(purpose);
        }
        if let Some(source_repo) = req.source_repo {
            query = query.bind(source_repo);
        }
        if let Some(source_branch) = req.source_branch {
            query = query.bind(source_branch);
        }

        query = query.bind(space).bind(name);

        let result = query.execute(pool).await?;
        
        if result.rows_affected() > 0 {
            Self::find_by_name(pool, space, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn update_status(
        pool: &sqlx::MySqlPool,
        space: &str,
        name: &str,
        status_update: AgentStatusUpdate,
    ) -> Result<Option<Agent>, sqlx::Error> {
        let mut query_builder = String::from("UPDATE agents SET status = ?, updated_at = CURRENT_TIMESTAMP");
        
        
        
        query_builder.push_str(" WHERE space = ? AND name = ?");

        let mut query = sqlx::query(&query_builder)
            .bind(&status_update.status);



        query = query.bind(space).bind(name);

        let result = query.execute(pool).await?;
        
        if result.rows_affected() > 0 {
            Self::find_by_name(pool, space, name).await
        } else {
            Ok(None)
        }
    }

    pub async fn delete(pool: &sqlx::MySqlPool, space: &str, name: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM agents WHERE space = ? AND name = ?")
            .bind(space)
            .bind(name)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn find_running_agents(pool: &sqlx::MySqlPool, space: &str) -> Result<Vec<Agent>, sqlx::Error> {
        sqlx::query_as::<_, Agent>(
            r#"
            SELECT name, space, description, purpose,
                   source_repo, source_branch,
                   status, created_at, updated_at, created_by
            FROM agents
            WHERE space = ? AND status = 'running'
            ORDER BY created_at DESC
            "#
        )
        .bind(space)
        .fetch_all(pool)
        .await
    }
}