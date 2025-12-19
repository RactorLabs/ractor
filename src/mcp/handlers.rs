use std::{collections::HashMap, fs, path::PathBuf};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

use crate::mcp::client;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::models::{
    AuthPayload, BatchInvokeRequest, BatchInvokeResponse, BatchInvokeResult, InvocationResponse,
    InvokeRequest, McpToolDescriptor, ServerInput, ServerResponse, ToolExampleInput,
    ToolExampleResponse, ToolResponse,
};
use crate::mcp::output_schemas::{apply_output_schema_overrides, ensure_output_schema};
use crate::mcp::state::McpState;

#[derive(Debug, Deserialize)]
pub struct ToolQuery {
    pub server: Option<String>,
    pub server_id: Option<Uuid>,
    pub q: Option<String>,
    #[serde(default)]
    pub include_examples: bool,
    pub limit: Option<u32>,
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
    let mut clauses = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(id) = filter.server_id {
        clauses.push("t.server_id = ?".to_string());
        params.push(id.to_string());
    } else if let Some(name) = filter.server {
        clauses.push("s.name = ?".to_string());
        params.push(name);
    }

    if let Some(q) = filter.q.as_ref() {
        let like = format!("%{}%", q.to_lowercase());
        clauses.push("LOWER(t.name) LIKE ? OR LOWER(t.description) LIKE ?".to_string());
        params.push(like.clone());
        params.push(like);
    }

    let limit = filter.limit.unwrap_or(50).min(200);

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };

    let sql = format!(
        r#"
        SELECT t.id, t.server_id, s.name as server_name, t.name, t.description, t.input_schema, t.output_schema, t.metadata, t.version, t.created_at
        FROM mcp_tools t
        JOIN mcp_servers s ON s.id = t.server_id
        {}
        ORDER BY t.created_at DESC
        LIMIT ?
        "#,
        where_clause
    );

    let mut query = sqlx::query(&sql);
    for param in params {
        query = query.bind(param);
    }
    query = query.bind(limit as i64);

    let rows = query.fetch_all(&*state.db).await?;
    let mut tools = rows
        .into_iter()
        .filter_map(map_tool_row)
        .collect::<Vec<_>>();

    if filter.include_examples {
        let ids = tools.iter().map(|t| t.id).collect::<Vec<_>>();
        let example_map = load_examples_for_tools(&state, &ids).await?;
        for tool in &mut tools {
            tool.examples = example_map.get(&tool.id).cloned();
        }
    }

    // Ensure every tool has a non-null output schema for downstream caches/filters.
    for tool in &mut tools {
        ensure_output_schema(&tool.server_name, &tool.name, &mut tool.output_schema);
    }

    Ok(Json(tools))
}

pub async fn live_search_tools(
    State(state): State<McpState>,
    Query(filter): Query<ToolQuery>,
) -> McpResult<Json<Vec<ToolResponse>>> {
    let mut servers = Vec::new();
    let rows = sqlx::query(
        r#"
        SELECT id, name, base_url, auth_type, auth_payload
        FROM mcp_servers
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&*state.db)
    .await?;

    for row in rows {
        let id = parse_uuid(&row, "id")?;
        let name: String = row.try_get("name")?;
        let base_url: String = row.try_get("base_url")?;
        let auth_type: Option<String> = row.try_get("auth_type")?;
        let auth_payload_val: Option<Value> = row.try_get("auth_payload")?;
        let auth_payload = auth_payload_val
            .as_ref()
            .and_then(|v| serde_json::from_value::<AuthPayload>(v.clone()).ok());
        servers.push((id, name, base_url, auth_type, auth_payload));
    }

    let mut results = Vec::new();
    let q_lower = filter.q.as_ref().map(|q| q.to_lowercase());
    let limit = filter.limit.unwrap_or(50).min(200) as usize;

    for (server_id, server_name, base_url, auth_type, auth_payload) in servers {
        match client::fetch_tools(
            &state,
            &base_url,
            auth_type.as_deref(),
            auth_payload.as_ref(),
        )
        .await
        {
            Ok(mut tools) => {
                apply_output_schema_overrides(&server_name, &mut tools);
                for tool in tools {
                    if let Some(ref q) = q_lower {
                        let name_match = tool.name.to_lowercase().contains(q);
                        let desc_match = tool
                            .description
                            .as_ref()
                            .map(|d| d.to_lowercase().contains(q))
                            .unwrap_or(false);
                        if !name_match && !desc_match {
                            continue;
                        }
                    }
                    let now = Utc::now().to_rfc3339();
                    let mut tool_response = ToolResponse {
                        id: Uuid::new_v4(),
                        server_id,
                        server_name: server_name.clone(),
                        name: tool.name,
                        description: tool.description,
                        input_schema: tool.input_schema,
                        output_schema: tool.output_schema,
                        metadata: tool.metadata,
                        version: tool.version,
                        created_at: now.clone(),
                        examples: None,
                    };
                    ensure_output_schema(
                        &tool_response.server_name,
                        &tool_response.name,
                        &mut tool_response.output_schema,
                    );
                    results.push(tool_response);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
            Err(err) => {
                warn!("Live MCP tool search failed for {}: {:?}", server_name, err);
            }
        }
        if results.len() >= limit {
            break;
        }
    }

    Ok(Json(results))
}

pub async fn invoke_tool(
    State(state): State<McpState>,
    Json(payload): Json<InvokeRequest>,
) -> McpResult<Json<InvocationResponse>> {
    let server = resolve_server(&state, payload.server.as_ref(), payload.server_id).await?;

    // Persist invocation row
    let invocation_id = insert_invocation(
        &state,
        server.id,
        &payload.tool,
        payload.sandbox_id,
        &payload.arguments,
    )
    .await?;

    let result = client::invoke_tool(
        &state,
        &server.base_url,
        server.auth_type.as_deref(),
        server.auth_payload.as_ref(),
        invocation_id,
        &payload,
    )
    .await;

    match result {
        Ok(response) => {
            finalize_invocation_success(&state, invocation_id, &response.result).await?;
            Ok(Json(response))
        }
        Err(err) => {
            let _message = finalize_invocation_error(&state, invocation_id, &err).await?;
            Err(err)
        }
    }
}

pub async fn batch_invoke(
    State(state): State<McpState>,
    Json(payload): Json<BatchInvokeRequest>,
) -> McpResult<Json<BatchInvokeResponse>> {
    if payload.calls.is_empty() {
        return Err(McpError::BadRequest("calls are required".to_string()));
    }

    let server = resolve_server(&state, payload.server.as_ref(), payload.server_id).await?;
    let batch_id = Uuid::new_v4();
    let mut results = Vec::new();

    for call in &payload.calls {
        let invocation_id = insert_invocation(
            &state,
            server.id,
            &call.tool,
            payload.sandbox_id,
            &call.arguments,
        )
        .await?;

        let request = InvokeRequest {
            server: None,
            server_id: Some(server.id),
            tool: call.tool.clone(),
            arguments: call.arguments.clone(),
            sandbox_id: payload.sandbox_id,
        };

        let result = client::invoke_tool(
            &state,
            &server.base_url,
            server.auth_type.as_deref(),
            server.auth_payload.as_ref(),
            invocation_id,
            &request,
        )
        .await;

        match result {
            Ok(resp) => {
                finalize_invocation_success(&state, invocation_id, &resp.result).await?;
                results.push(BatchInvokeResult {
                    invocation_id,
                    tool: call.tool.clone(),
                    status: resp.status,
                    result: resp.result,
                    error: None,
                });
            }
            Err(err) => {
                let message = finalize_invocation_error(&state, invocation_id, &err).await?;
                results.push(BatchInvokeResult {
                    invocation_id,
                    tool: call.tool.clone(),
                    status: "failed".to_string(),
                    result: None,
                    error: Some(message),
                });
            }
        }
    }

    if payload.write_trace {
        let _ = write_batch_trace(batch_id, server.id, &results);
    }

    Ok(Json(BatchInvokeResponse {
        batch_id,
        server_id: server.id,
        results,
    }))
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

pub async fn list_tool_examples(
    State(state): State<McpState>,
    Path(tool_id): Path<Uuid>,
) -> McpResult<Json<Vec<ToolExampleResponse>>> {
    ensure_tool_exists(&state, tool_id).await?;
    let examples_map = load_examples_for_tools(&state, &[tool_id]).await?;
    Ok(Json(
        examples_map.get(&tool_id).cloned().unwrap_or_default(),
    ))
}

pub async fn create_tool_example(
    State(state): State<McpState>,
    Path(tool_id): Path<Uuid>,
    Json(payload): Json<ToolExampleInput>,
) -> McpResult<(StatusCode, Json<ToolExampleResponse>)> {
    ensure_tool_exists(&state, tool_id).await?;

    if payload.body.is_null() {
        return Err(McpError::BadRequest("body cannot be null".to_string()));
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO mcp_tool_examples (id, tool_id, title, body, created_at)
        VALUES (?, ?, ?, ?, NOW())
        "#,
    )
    .bind(id.to_string())
    .bind(tool_id.to_string())
    .bind(&payload.title)
    .bind(&payload.body)
    .execute(&*state.db)
    .await?;

    let created_at: DateTime<Utc> = Utc::now();
    let response = ToolExampleResponse {
        id,
        tool_id,
        title: payload.title,
        body: payload.body,
        created_at: created_at.to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(response)))
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
        examples: None,
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
    let mut tools = client::fetch_tools(state, base_url, auth_type, auth_payload).await?;
    apply_output_schema_overrides(server_name, &mut tools);

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

    let mut tools = rows
        .into_iter()
        .filter_map(map_tool_row)
        .collect::<Vec<_>>();
    for tool in &mut tools {
        ensure_output_schema(&tool.server_name, &tool.name, &mut tool.output_schema);
    }
    Ok(tools)
}

async fn resolve_server(
    state: &McpState,
    server_name: Option<&String>,
    server_id: Option<Uuid>,
) -> McpResult<ResolvedServer> {
    let lookup_row = if let Some(id) = server_id {
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
    } else if let Some(name) = server_name {
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

    let Some(row) = lookup_row else {
        return Err(McpError::BadRequest(
            "server or server_id is required".to_string(),
        ));
    };

    let id = parse_uuid(&row, "id")?;
    let base_url: String = row.try_get("base_url")?;
    let auth_type: Option<String> = row.try_get("auth_type")?;
    let auth_payload_val: Option<Value> = row.try_get("auth_payload")?;
    let auth_payload = auth_payload_val
        .as_ref()
        .and_then(|v| serde_json::from_value::<AuthPayload>(v.clone()).ok());

    Ok(ResolvedServer {
        id,
        base_url,
        auth_type,
        auth_payload,
    })
}

async fn insert_invocation(
    state: &McpState,
    server_id: Uuid,
    tool: &str,
    sandbox_id: Option<Uuid>,
    arguments: &Option<Value>,
) -> McpResult<Uuid> {
    let invocation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO mcp_invocations (id, server_id, tool_name, sandbox_id, request, status, started_at)
        VALUES (?, ?, ?, ?, ?, 'pending', NOW())
        "#,
    )
    .bind(invocation_id.to_string())
    .bind(server_id.to_string())
    .bind(tool)
    .bind(sandbox_id.map(|v| v.to_string()))
    .bind(arguments)
    .execute(&*state.db)
    .await?;
    Ok(invocation_id)
}

async fn finalize_invocation_success(
    state: &McpState,
    invocation_id: Uuid,
    result: &Option<Value>,
) -> McpResult<()> {
    sqlx::query(
        r#"
        UPDATE mcp_invocations
        SET status = 'completed', response = ?, finished_at = NOW()
        WHERE id = ?
        "#,
    )
    .bind(result)
    .bind(invocation_id.to_string())
    .execute(&*state.db)
    .await?;
    Ok(())
}

async fn finalize_invocation_error(
    state: &McpState,
    invocation_id: Uuid,
    err: &McpError,
) -> McpResult<String> {
    let message = error_message(err);
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
    Ok(message)
}

fn error_message(err: &McpError) -> String {
    match err {
        McpError::Upstream(msg) | McpError::BadRequest(msg) => msg.clone(),
        McpError::Database(e) => e.to_string(),
        McpError::Internal(e) => e.to_string(),
        McpError::NotFound(msg) => msg.clone(),
        McpError::Conflict(msg) => msg.clone(),
    }
}

async fn load_examples_for_tools(
    state: &McpState,
    tool_ids: &[Uuid],
) -> McpResult<HashMap<Uuid, Vec<ToolExampleResponse>>> {
    if tool_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut ids: Vec<Uuid> = tool_ids.to_vec();
    ids.sort();
    ids.dedup();
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        r#"
        SELECT id, tool_id, title, body, created_at
        FROM mcp_tool_examples
        WHERE tool_id IN ({})
        ORDER BY created_at DESC
        "#,
        placeholders
    );
    let mut query = sqlx::query(&sql);
    for id in ids {
        query = query.bind(id.to_string());
    }
    let rows = query.fetch_all(&*state.db).await?;
    let mut map: HashMap<Uuid, Vec<ToolExampleResponse>> = HashMap::new();
    for row in rows {
        let tool_id = parse_uuid(&row, "tool_id")?;
        let id = parse_uuid(&row, "id")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;
        let entry = map.entry(tool_id).or_default();
        entry.push(ToolExampleResponse {
            id,
            tool_id,
            title: row.try_get("title").unwrap_or(None),
            body: row.try_get("body").unwrap_or(json!({})),
            created_at: created_at.to_rfc3339(),
        });
    }
    Ok(map)
}

async fn ensure_tool_exists(state: &McpState, tool_id: Uuid) -> McpResult<()> {
    let exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1 FROM mcp_tools WHERE id = ? LIMIT 1
        "#,
    )
    .bind(tool_id.to_string())
    .fetch_optional(&*state.db)
    .await?;
    if exists.is_none() {
        return Err(McpError::NotFound("tool not found".to_string()));
    }
    Ok(())
}

fn write_batch_trace(
    batch_id: Uuid,
    server_id: Uuid,
    results: &[BatchInvokeResult],
) -> std::io::Result<()> {
    let dir = PathBuf::from("target/mcp_traces");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}_{}.json", batch_id, server_id));
    let body = serde_json::to_string_pretty(&json!({
        "batch_id": batch_id,
        "server_id": server_id,
        "results": results,
    }))?;
    fs::write(path, body)
}

struct ResolvedServer {
    id: Uuid,
    base_url: String,
    auth_type: Option<String>,
    auth_payload: Option<AuthPayload>,
}
