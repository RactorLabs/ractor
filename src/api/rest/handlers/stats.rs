use std::sync::Arc;

use anyhow::anyhow;
use axum::{
    extract::{Extension, State},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::AppState;

#[derive(Debug, Serialize)]
pub struct GlobalStatsResponse {
    pub sandboxes_total: i64,
    pub sandboxes_active: i64,
    pub sandboxes_terminated: i64,
    pub sandboxes_by_state: JsonValue,
    pub inference_name: Option<String>,
    pub inference_url: Option<String>,
    pub inference_models: Vec<String>,
    pub default_inference_model: Option<String>,
    pub captured_at: String,
}

pub async fn get_global_stats(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<GlobalStatsResponse>> {
    check_api_permission(&auth, &state, &permissions::SANDBOX_LIST)
        .await
        .map_err(|_| {
            ApiError::Forbidden("Insufficient permissions to view global stats".to_string())
        })?;

    let rows: Vec<(String, i64)> = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT LOWER(state) as state, COUNT(*) as count
        FROM sandboxes
        GROUP BY LOWER(state)
        "#,
    )
    .fetch_all(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow!("Failed to aggregate sandbox counts: {}", e)))?;

    let mut total: i64 = 0;
    let mut terminated: i64 = 0;
    let mut inactive: i64 = 0;
    let mut state_map = JsonMap::new();

    for (state, count) in rows {
        total += count;
        if state == "terminated" {
            terminated = count;
        }
        if state == "deleted" || state == "terminated" {
            inactive += count;
        }
        state_map.insert(state, JsonValue::from(count));
    }

    let active = total.saturating_sub(inactive);

    let provider = state.inference_registry.default_provider();
    let inference_url = Some(provider.url.clone());

    Ok(Json(GlobalStatsResponse {
        sandboxes_total: total,
        sandboxes_active: active,
        sandboxes_terminated: terminated,
        sandboxes_by_state: JsonValue::Object(state_map),
        inference_name: Some(provider.display_name.clone()),
        inference_url,
        inference_models: provider.models.iter().map(|m| m.name.clone()).collect(),
        default_inference_model: Some(provider.default_model.clone()),
        captured_at: Utc::now().to_rfc3339(),
    }))
}
