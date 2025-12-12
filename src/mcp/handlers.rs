use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::mcp::client;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::models::{
    AuthPayload, InvocationResponse, InvokeRequest, McpToolDescriptor, ServerInput, ServerResponse,
    ToolResponse,
};
use crate::mcp::state::McpState;

#[derive(Debug, Deserialize)]
pub struct ToolQuery {
    pub server: Option<String>,
    pub server_id: Option<Uuid>,
}

pub async fn list_servers(State(state): State<McpState>) -> McpResult<Json<Vec<ServerResponse>>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, base_url, auth_type, auth_payload, status, last_seen_at, created_at, updated_at
        FROM mcp_servers
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&*state.db)
    .await?;

    let servers = rows
        .into_iter()
        .filter_map(map_server_row)
        .collect::<Vec<_>>();

    Ok(Json(servers))
}

pub async fn upsert_server(
    State(state): State<McpState>,
    Json(payload): Json<ServerInput>,
) -> McpResult<(StatusCode, Json<ServerResponse>)> {
    if payload.name.trim().is_empty() {
        return Err(McpError::BadRequest("name is required".to_string()));
    }
    if payload.base_url.trim().is_empty() {
        return Err(McpError::BadRequest("base_url is required".to_string()));
    }

    let id = Uuid::new_v4();
    let auth_payload = payload.auth_payload.clone().unwrap_or_else(|| json!({}));
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO mcp_servers (id, name, base_url, auth_type, auth_payload, status, last_seen_at, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'unknown', NULL, ?, ?)
        ON DUPLICATE KEY UPDATE base_url = VALUES(base_url),
            auth_type = VALUES(auth_type),
            auth_payload = VALUES(auth_payload),
            updated_at = VALUES(updated_at)
        "#,
    )
    .bind(id.to_string())
    .bind(&payload.name)
    .bind(&payload.base_url)
    .bind(&payload.auth_type)
    .bind(&auth_payload)
    .bind(now)
    .bind(now)
    .execute(&*state.db)
    .await?;

    let server_row = sqlx::query(
        r#"
        SELECT id, name, base_url, auth_type, auth_payload, status, last_seen_at, created_at, updated_at
        FROM mcp_servers
        WHERE name = ?
        "#,
    )
    .bind(&payload.name)
    .fetch_one(&*state.db)
    .await?;

    let mut server = map_server_row(server_row)
        .ok_or_else(|| McpError::Internal(anyhow::anyhow!("failed to read server after upsert")))?;

    // Optionally sync tools immediately
    if payload.sync {
        let auth_payload_typed = payload
            .auth_payload
            .as_ref()
            .and_then(|v| serde_json::from_value::<AuthPayload>(v.clone()).ok());
        let synced = sync_tools_for_server_internal(
            &state,
            server.id,
            &server.name,
            &server.base_url,
            server.auth_type.as_deref(),
            auth_payload_typed.as_ref(),
        )
        .await?;
        if synced > 0 {
            server.status = "synced".to_string();
            let _ = sqlx::query(
                r#"
                UPDATE mcp_servers SET status = 'synced', last_seen_at = NOW() WHERE id = ?
                "#,
            )
            .bind(server.id.to_string())
            .execute(&*state.db)
            .await;
        }
    }

    Ok((StatusCode::CREATED, Json(server)))
}

pub async fn sync_server(
    State(state): State<McpState>,
    Path(id): Path<Uuid>,
) -> McpResult<Json<Vec<ToolResponse>>> {
    let row = sqlx::query(
        r#"
        SELECT id, name, base_url, auth_type, auth_payload
        FROM mcp_servers
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(&*state.db)
    .await?;

    let Some(server) = row else {
        return Err(McpError::NotFound("server not found".to_string()));
    };

    let server_id = parse_uuid(&server, "id")?;
    let name: String = server.try_get("name")?;
    let base_url: String = server.try_get("base_url")?;
    let auth_type: Option<String> = server.try_get("auth_type")?;
    let auth_payload_val: Option<serde_json::Value> = server.try_get("auth_payload")?;
    let auth_payload_typed = auth_payload_val
        .as_ref()
        .and_then(|v| serde_json::from_value::<AuthPayload>(v.clone()).ok());

    sync_tools_for_server_internal(
        &state,
        server_id,
        &name,
        &base_url,
        auth_type.as_deref(),
        auth_payload_typed.as_ref(),
    )
    .await?;

    let tools = load_tools_for_server(&state, server_id).await?;
    Ok(Json(tools))
}

pub async fn list_tools(
    State(state): State<McpState>,
    Query(filter): Query<ToolQuery>,
) -> McpResult<Json<Vec<ToolResponse>>> {
    let mut where_clause = String::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(id) = filter.server_id {
        where_clause.push_str("WHERE t.server_id = ?");
        params.push(id.to_string());
    } else if let Some(name) = filter.server {
        where_clause.push_str("WHERE s.name = ?");
        params.push(name);
    }

    let sql = format!(
        r#"
        SELECT t.id, t.server_id, s.name as server_name, t.name, t.description, t.input_schema, t.output_schema, t.metadata, t.version, t.created_at
        FROM mcp_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        {}
        ORDER BY t.created_at DESC
        "#,
        where_clause
    );

    let mut query = sqlx::query(&sql);
    for param in params {
        query = query.bind(param);
    }

    let rows = query.fetch_all(&*state.db).await?;
    let tools = rows
        .into_iter()
        .filter_map(map_tool_row)
        .collect::<Vec<_>>();
    Ok(Json(tools))
}

pub async fn invoke_tool(
    State(state): State<McpState>,
    Json(payload): Json<InvokeRequest>,
) -> McpResult<Json<InvocationResponse>> {
    let lookup_row = if let Some(id) = payload.server_id {
        sqlx::query(
            r#"
            SELECT id, name, base_url, auth_type, auth_payload
            FROM mcp_servers
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&*state.db)
        .await?
    } else if let Some(name) = payload.server.as_ref() {
        sqlx::query(
            r#"
            SELECT id, name, base_url, auth_type, auth_payload
            FROM mcp_servers
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(&*state.db)
        .await?
    } else {
        None
    };

    let Some(server_row) = lookup_row else {
        return Err(McpError::BadRequest(
            "server or server_id is required".to_string(),
        ));
    };

    let server_id = parse_uuid(&server_row, "id")?;
    let base_url: String = server_row.try_get("base_url")?;
    let auth_type: Option<String> = server_row.try_get("auth_type")?;
    let auth_payload_val: Option<serde_json::Value> = server_row.try_get("auth_payload")?;
    let auth_payload_typed = auth_payload_val
        .as_ref()
        .and_then(|v| serde_json::from_value::<AuthPayload>(v.clone()).ok());

    // Persist invocation row
    let invocation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO mcp_invocations (id, server_id, tool_name, sandbox_id, request, status, started_at)
        VALUES (?, ?, ?, ?, ?, 'pending', NOW())
        "#,
    )
    .bind(invocation_id.to_string())
    .bind(server_id.to_string())
    .bind(&payload.tool)
    .bind(payload.sandbox_id.map(|v| v.to_string()))
    .bind(&payload.arguments)
    .execute(&*state.db)
    .await?;

    let result = client::invoke_tool(
        &state,
        &base_url,
        auth_type.as_deref(),
        auth_payload_typed.as_ref(),
        invocation_id,
        &payload,
    )
    .await;

    match result {
        Ok(response) => {
            sqlx::query(
                r#"
                UPDATE mcp_invocations
                SET status = 'completed', response = ?, finished_at = NOW()
                WHERE id = ?
                "#,
            )
            .bind(&response.result)
            .bind(invocation_id.to_string())
            .execute(&*state.db)
            .await?;
            Ok(Json(response))
        }
        Err(err) => {
            let message = match &err {
                McpError::Upstream(msg) | McpError::BadRequest(msg) => msg.clone(),
                McpError::Database(e) => e.to_string(),
                McpError::Internal(e) => e.to_string(),
                McpError::NotFound(msg) => msg.clone(),
                McpError::Conflict(msg) => msg.clone(),
            };
            sqlx::query(
                r#"
                UPDATE mcp_invocations
                SET status = 'failed', error_text = ?, finished_at = NOW()
                WHERE id = ?
                "#,
            )
            .bind(&message)
            .bind(invocation_id.to_string())
            .execute(&*state.db)
            .await?;
            Err(err)
        }
    }
}

pub async fn get_invocation(
    State(state): State<McpState>,
    Path(id): Path<Uuid>,
) -> McpResult<Json<InvocationResponse>> {
    let row = sqlx::query(
        r#"
        SELECT id, status, response, error_text
        FROM mcp_invocations
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(&*state.db)
    .await?;

    let Some(r) = row else {
        return Err(McpError::NotFound("invocation not found".to_string()));
    };

    let response: Option<serde_json::Value> = r.try_get("response")?;
    let error: Option<String> = r.try_get("error_text")?;
    let status: String = r.try_get("status")?;

    Ok(Json(InvocationResponse {
        id,
        status,
        result: response,
        error,
    }))
}

fn map_server_row(row: sqlx::mysql::MySqlRow) -> Option<ServerResponse> {
    let id = parse_uuid(&row, "id").ok()?;
    let created_at: DateTime<Utc> = row.try_get("created_at").ok()?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").ok()?;
    let last_seen_at: Option<DateTime<Utc>> = row.try_get("last_seen_at").ok()?;
    Some(ServerResponse {
        id,
        name: row.try_get("name").unwrap_or_default(),
        base_url: row.try_get("base_url").unwrap_or_default(),
        auth_type: row.try_get("auth_type").unwrap_or(None),
        auth_payload: row.try_get("auth_payload").unwrap_or(None),
        status: row
            .try_get("status")
            .unwrap_or_else(|_| "unknown".to_string()),
        last_seen_at: last_seen_at.map(|dt| dt.to_rfc3339()),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    })
}

fn map_tool_row(row: sqlx::mysql::MySqlRow) -> Option<ToolResponse> {
    let id = parse_uuid(&row, "id").ok()?;
    let server_id = parse_uuid(&row, "server_id").ok()?;
    let created_at: DateTime<Utc> = row.try_get("created_at").ok()?;

    Some(ToolResponse {
        id,
        server_id,
        server_name: row.try_get("server_name").unwrap_or_default(),
        name: row.try_get("name").unwrap_or_default(),
        description: row.try_get("description").unwrap_or(None),
        input_schema: row.try_get("input_schema").unwrap_or(None),
        output_schema: row.try_get("output_schema").unwrap_or(None),
        metadata: row.try_get("metadata").unwrap_or(None),
        version: row.try_get("version").unwrap_or(None),
        created_at: created_at.to_rfc3339(),
    })
}

fn parse_uuid(row: &sqlx::mysql::MySqlRow, col: &str) -> Result<Uuid, McpError> {
    let raw: String = row.try_get(col)?;
    Uuid::parse_str(&raw).map_err(|e| McpError::BadRequest(format!("invalid uuid {}: {}", col, e)))
}

async fn sync_tools_for_server_internal(
    state: &McpState,
    server_id: Uuid,
    server_name: &str,
    base_url: &str,
    auth_type: Option<&str>,
    auth_payload: Option<&AuthPayload>,
) -> McpResult<usize> {
    let tools = client::fetch_tools(state, base_url, auth_type, auth_payload).await?;

    let mut inserted = 0usize;
    for tool in tools {
        upsert_tool(state, server_id, server_name, &tool).await?;
        inserted += 1;
    }

    // Update sync status
    let _ = sqlx::query(
        r#"
        UPDATE mcp_servers SET status = 'synced', last_seen_at = NOW() WHERE id = ?
        "#,
    )
    .bind(server_id.to_string())
    .execute(&*state.db)
    .await;

    Ok(inserted)
}

async fn upsert_tool(
    state: &McpState,
    server_id: Uuid,
    server_name: &str,
    tool: &McpToolDescriptor,
) -> McpResult<()> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO mcp_tools (id, server_id, name, description, input_schema, output_schema, metadata, version, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())
        ON DUPLICATE KEY UPDATE
            description = VALUES(description),
            input_schema = VALUES(input_schema),
            output_schema = VALUES(output_schema),
            metadata = VALUES(metadata),
            version = VALUES(version)
        "#,
    )
    .bind(id.to_string())
    .bind(server_id.to_string())
    .bind(&tool.name)
    .bind(&tool.description)
    .bind(&tool.input_schema)
    .bind(&tool.output_schema)
    .bind(&tool.metadata)
    .bind(&tool.version)
    .execute(&*state.db)
    .await?;

    // Ensure we keep a stable tool id by re-reading the row
    let _ = sqlx::query(
        r#"
        SELECT id FROM mcp_tools WHERE server_id = ? AND name = ? LIMIT 1
        "#,
    )
    .bind(server_id.to_string())
    .bind(&tool.name)
    .fetch_one(&*state.db)
    .await?;

    tracing::info!(
        "Synced MCP tool '{}' from server '{}'",
        tool.name,
        server_name
    );
    Ok(())
}

async fn load_tools_for_server(state: &McpState, server_id: Uuid) -> McpResult<Vec<ToolResponse>> {
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.server_id, s.name as server_name, t.name, t.description, t.input_schema, t.output_schema, t.metadata, t.version, t.created_at
        FROM mcp_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        WHERE t.server_id = ?
        ORDER BY t.created_at DESC
        "#,
    )
    .bind(server_id.to_string())
    .fetch_all(&*state.db)
    .await?;

    Ok(rows.into_iter().filter_map(map_tool_row).collect())
}
