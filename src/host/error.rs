use thiserror::Error;

#[derive(Error, Debug)]
pub enum HostError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Claude API error: {0}")]
    Claude(String),

    #[error("Guardrail violation: {0}")]
    Guardrail(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, HostError>;
