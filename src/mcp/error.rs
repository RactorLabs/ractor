use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug)]
pub enum McpError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Upstream(String),
    Database(sqlx::Error),
    Internal(anyhow::Error),
}

pub type McpResult<T> = Result<T, McpError>;

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

impl IntoResponse for McpError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            McpError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            McpError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            McpError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            McpError::Upstream(msg) => (StatusCode::BAD_GATEWAY, msg),
            McpError::Database(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database error: {}", err),
            ),
            McpError::Internal(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("internal error: {}", err),
            ),
        };
        (status, Json(ErrorResponse { message })).into_response()
    }
}

impl From<sqlx::Error> for McpError {
    fn from(err: sqlx::Error) -> Self {
        McpError::Database(err)
    }
}

impl From<anyhow::Error> for McpError {
    fn from(err: anyhow::Error) -> Self {
        McpError::Internal(err)
    }
}
