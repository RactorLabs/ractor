use sqlx::{MySql, Pool};
use thiserror::Error;

pub mod constants;
pub mod session;
// message module removed in favor of response
pub mod response;
pub mod state_helpers;

pub use session::{
    CreateSessionRequest, PublishSessionRequest, RemixSessionRequest, RestoreSessionRequest,
    Session, UpdateSessionRequest, UpdateSessionStateRequest,
};
// legacy message exports removed
pub use response::{CreateResponseRequest, ResponseView, SessionResponse, UpdateResponseRequest};

// Database errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection error: {0}")]
    Connection(sqlx::Error),
    #[error("UUID parse error: {0}")]
    UuidParse(#[from] uuid::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("General error: {0}")]
    General(#[from] anyhow::Error),
    #[error("Unique constraint violation: {0}")]
    Unique(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        // Check for MySQL unique constraint violation (error code 1062)
        if let sqlx::Error::Database(db_err) = &err {
            if let Some(code) = db_err.code() {
                if code == "23000" || code == "1062" {
                    return DatabaseError::Unique(db_err.message().to_string());
                }
            }
        }
        DatabaseError::Connection(err)
    }
}

// Application state
#[derive(Clone)]
pub struct AppState {
    pub db: std::sync::Arc<Pool<MySql>>,
    pub jwt_secret: String,
}
