use crate::shared::models::AppState;
use axum::{
    extract::{Path, Query, State},
    response::{Json, Result as AxumResult},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub lines: Option<u32>,
    pub log_type: Option<String>, // stdout, stderr, or all
}

#[derive(Debug, Serialize)]
pub struct AgentLogsResponse {
    pub agent_name: String,
    pub space: String,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub timestamp: String,
}

/// Get agent logs
pub async fn get_agent_logs(
    State(_state): State<Arc<AppState>>,
    Path((space, name)): Path<(String, String)>,
    Query(params): Query<LogsQuery>,
) -> AxumResult<Json<AgentLogsResponse>> {
    info!(
        space = space,
        agent_name = name,
        lines = params.lines.unwrap_or(100),
        log_type = params.log_type.as_deref().unwrap_or("all"),
        "Fetching agent logs"
    );

    let lines_to_read = params.lines.unwrap_or(100);
    let log_type = params.log_type.as_deref().unwrap_or("all");

    // For now, we'll need to determine the session ID somehow
    // In a real implementation, this would come from the session context or be passed as a parameter
    let session_id = "current"; // Placeholder
    
    let stdout_logs = if log_type == "stderr" {
        Vec::new()
    } else {
        read_log_tail(
            &format!("/tmp/logs/agents/{}/{}-stdout.log", session_id, name),
            lines_to_read,
        ).await.unwrap_or_else(|e| {
            error!("Failed to read stdout logs: {}", e);
            Vec::new()
        })
    };

    let stderr_logs = if log_type == "stdout" {
        Vec::new()
    } else {
        read_log_tail(
            &format!("/tmp/logs/agents/{}/{}-stderr.log", session_id, name),
            lines_to_read,
        ).await.unwrap_or_else(|e| {
            error!("Failed to read stderr logs: {}", e);
            Vec::new()
        })
    };

    Ok(Json(AgentLogsResponse {
        agent_name: name,
        space,
        stdout: stdout_logs,
        stderr: stderr_logs,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

async fn read_log_tail(file_path: &str, lines: u32) -> Result<Vec<String>, std::io::Error> {
    match fs::read_to_string(file_path).await {
        Ok(content) => {
            let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            let start_index = if all_lines.len() > lines as usize {
                all_lines.len() - lines as usize
            } else {
                0
            };
            Ok(all_lines[start_index..].to_vec())
        }
        Err(e) => {
            // If file doesn't exist, return empty vector instead of error
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(Vec::new())
            } else {
                Err(e)
            }
        }
    }
}