use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

use crate::shared::models::DatabaseError;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    Forbidden(String),
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Database(DatabaseError),
    Jwt(jsonwebtoken::errors::Error),
    Internal(anyhow::Error),
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Database(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", e),
            ),
            ApiError::Jwt(e) => (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)),
            ApiError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal error: {}", e),
            ),
        };
        (status, Json(ErrorResponse { message })).into_response()
    }
}

impl From<DatabaseError> for ApiError {
    fn from(e: DatabaseError) -> Self {
        ApiError::Database(e)
    }
}

impl From<bcrypt::BcryptError> for ApiError {
    fn from(e: bcrypt::BcryptError) -> Self {
        ApiError::Internal(anyhow::anyhow!(e.to_string()))
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e)
    }
}
