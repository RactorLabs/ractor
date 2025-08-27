use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::shared::models::AppState;
use crate::server::rest::error::{ApiError, ApiResult};
use crate::server::rest::middleware::AuthContext;
use crate::server::rest::rbac_enforcement::{check_api_permission, permissions};

#[derive(Debug, Serialize)]
pub struct SpaceBuildResponse {
    pub space: String,
    pub status: String,
    pub image_tag: Option<String>,
    pub build_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub agents_deployed: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BuildSpaceRequest {
    pub force_rebuild: Option<bool>,
}

/// Trigger space build
pub async fn build_space(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth_context): Extension<AuthContext>,
    Json(payload): Json<BuildSpaceRequest>,
) -> ApiResult<Json<SpaceBuildResponse>> {
    check_api_permission(&auth_context, &state, &permissions::SPACE_BUILD, Some(&space))
        .await
        .map_err(|status| match status {
            StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    
    // Validate space exists
    let space_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM spaces WHERE name = ?"
    )
    .bind(&space)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to check space: {}", e)))? > 0;
    
    if !space_exists {
        return Err(ApiError::NotFound(format!("Space '{}' not found", space)));
    }
    
    // Generate unique build ID
    let build_id = Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now();
    
    // Insert build record
    sqlx::query(
        r#"
        INSERT INTO space_builds (id, space, status, build_id, started_at, force_rebuild)
        VALUES (?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&space)
    .bind("pending")
    .bind(&build_id)
    .bind(started_at)
    .bind(payload.force_rebuild.unwrap_or(false))
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create build record: {}", e)))?;
    
    // Queue build task for operator to process
    let force_rebuild = payload.force_rebuild.unwrap_or(false);
    let build_task_payload = serde_json::json!({
        "space": space,
        "build_id": build_id,
        "force_rebuild": force_rebuild
    });
    
    sqlx::query(
        r#"
        INSERT INTO build_tasks (task_type, space, build_id, payload, created_by)
        VALUES (?, ?, ?, ?, ?)
        "#
    )
    .bind("space_build")
    .bind(&space)
    .bind(&build_id)
    .bind(build_task_payload.to_string())
    .bind(auth_context.principal.name())
    .execute(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to queue build task: {}", e)))?;
    
    tracing::info!("Queued build task for space: {} (build_id: {})", space, build_id);
    
    Ok(Json(SpaceBuildResponse {
        space,
        status: "pending".to_string(),
        image_tag: None,
        build_id,
        started_at: started_at.to_rfc3339(),
        completed_at: None,
        agents_deployed: vec![],
        error: None,
    }))
}

/// Get space build status
pub async fn get_build_status(
    State(state): State<Arc<AppState>>,
    Path((space, build_id)): Path<(String, String)>,
    Extension(auth_context): Extension<AuthContext>,
) -> ApiResult<Json<SpaceBuildResponse>> {
    check_api_permission(&auth_context, &state, &permissions::SPACE_GET, Some(&space))
        .await
        .map_err(|status| match status {
            StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    
    let build = sqlx::query_as::<_, (String, String, Option<String>, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, Option<serde_json::Value>, Option<String>)>(
        r#"
        SELECT space, status, image_tag, build_id, started_at, completed_at, 
               agents_deployed, error
        FROM space_builds 
        WHERE space = ? AND build_id = ?
        ORDER BY started_at DESC 
        LIMIT 1
        "#
    )
    .bind(&space)
    .bind(&build_id)
    .fetch_optional(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch build status: {}", e)))?;
    
    let build = build.ok_or_else(|| ApiError::NotFound("Build not found".to_string()))?;
    
    let agents_deployed: Vec<String> = build.6
        .as_ref()
        .and_then(|json_value| serde_json::from_value(json_value.clone()).ok())
        .unwrap_or_default();
    
    Ok(Json(SpaceBuildResponse {
        space: build.0,
        status: build.1,
        image_tag: build.2,
        build_id: build.3,
        started_at: build.4.to_rfc3339(),
        completed_at: build.5.map(|dt| dt.to_rfc3339()),
        agents_deployed,
        error: build.7,
    }))
}

/// Get latest space build status
pub async fn get_latest_build(
    State(state): State<Arc<AppState>>,
    Path(space): Path<String>,
    Extension(auth_context): Extension<AuthContext>,
) -> ApiResult<Json<SpaceBuildResponse>> {
    check_api_permission(&auth_context, &state, &permissions::SPACE_GET, Some(&space))
        .await
        .map_err(|status| match status {
            StatusCode::FORBIDDEN => ApiError::Forbidden("Insufficient permissions".to_string()),
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            _ => ApiError::Internal(anyhow::anyhow!("Permission check failed")),
        })?;
    
    let build = sqlx::query_as::<_, (String, String, Option<String>, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, Option<serde_json::Value>, Option<String>)>(
        r#"
        SELECT space, status, image_tag, build_id, started_at, completed_at, 
               agents_deployed, error
        FROM space_builds 
        WHERE space = ?
        ORDER BY started_at DESC 
        LIMIT 1
        "#
    )
    .bind(&space)
    .fetch_optional(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to fetch latest build: {}", e)))?;
    
    let build = build.ok_or_else(|| ApiError::NotFound("No builds found for space".to_string()))?;
    
    let agents_deployed: Vec<String> = build.6
        .as_ref()
        .and_then(|json_value| serde_json::from_value(json_value.clone()).ok())
        .unwrap_or_default();
    
    Ok(Json(SpaceBuildResponse {
        space: build.0,
        status: build.1,
        image_tag: build.2,
        build_id: build.3,
        started_at: build.4.to_rfc3339(),
        completed_at: build.5.map(|dt| dt.to_rfc3339()),
        agents_deployed,
        error: build.7,
    }))
}