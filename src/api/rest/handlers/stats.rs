use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use axum::{
    extract::{Extension, State},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use sysinfo::{LoadAvg, System};
use tokio::task;

use crate::api::rest::error::{ApiError, ApiResult};
use crate::api::rest::middleware::AuthContext;
use crate::api::rest::rbac_enforcement::{check_api_permission, permissions};
use crate::shared::models::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct HostMetrics {
    pub hostname: Option<String>,
    pub uptime_seconds: u64,
    pub cpu_cores: usize,
    pub cpu_percent: f64,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub memory_used_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct GlobalStatsResponse {
    pub sandboxes_total: i64,
    pub sandboxes_active: i64,
    pub sandboxes_terminated: i64,
    pub sandboxes_by_state: JsonValue,
    pub sandbox_tasks_total: i64,
    pub sandbox_tasks_active: i64,
    pub inference_name: Option<String>,
    pub inference_url: Option<String>,
    pub inference_models: Vec<String>,
    pub default_inference_model: Option<String>,
    pub captured_at: String,
    pub host: Option<HostMetrics>,
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

    let tasks_total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sandbox_tasks")
        .fetch_one(&*state.db)
        .await
        .map_err(|e| ApiError::Internal(anyhow!("Failed to count sandbox tasks: {}", e)))?;

    let tasks_active = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sandbox_tasks WHERE status IN ('queued','processing')",
    )
    .fetch_one(&*state.db)
    .await
    .map_err(|e| ApiError::Internal(anyhow!("Failed to count in-flight tasks: {}", e)))?;

    let provider = state.inference_registry.default_provider();
    let inference_url = Some(provider.url.clone());

    let host_metrics = task::spawn_blocking(capture_host_metrics).await.ok();

    Ok(Json(GlobalStatsResponse {
        sandboxes_total: total,
        sandboxes_active: active,
        sandboxes_terminated: terminated,
        sandboxes_by_state: JsonValue::Object(state_map),
        sandbox_tasks_total: tasks_total,
        sandbox_tasks_active: tasks_active,
        inference_name: Some(provider.display_name.clone()),
        inference_url,
        inference_models: provider.models.iter().map(|m| m.name.clone()).collect(),
        default_inference_model: Some(provider.default_model.clone()),
        captured_at: Utc::now().to_rfc3339(),
        host: host_metrics,
    }))
}

fn capture_host_metrics() -> HostMetrics {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    thread::sleep(Duration::from_millis(200));
    sys.refresh_cpu_usage();

    let hostname = System::host_name();
    let uptime_seconds = System::uptime();
    let cpu_cores = sys.cpus().len();
    let cpu_percent = sys.global_cpu_info().cpu_usage() as f64;
    let LoadAvg { one, five, fifteen } = System::load_average();
    let memory_total_bytes = sys.total_memory();
    let memory_used_bytes = sys.used_memory();
    let memory_used_percent = if memory_total_bytes > 0 {
        (memory_used_bytes as f64 / memory_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    HostMetrics {
        hostname,
        uptime_seconds,
        cpu_cores,
        cpu_percent,
        load_avg_1m: one,
        load_avg_5m: five,
        load_avg_15m: fifteen,
        memory_total_bytes,
        memory_used_bytes,
        memory_used_percent,
    }
}
