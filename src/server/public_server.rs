use anyhow::Result;
use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::path::{Path, PathBuf};
use tokio::fs;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

pub async fn start_public_server() -> Result<()> {
    let app = create_public_app().await;
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    info!("Public Canvas HTTP server listening on http://0.0.0.0:8000");
    info!("Serving published canvas content from /public directory");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn create_public_app() -> Router {
    // Create the public directory if it doesn't exist
    if let Err(e) = fs::create_dir_all("/public").await {
        warn!("Failed to create /public directory: {}", e);
    }
    
    Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .fallback_service(
            ServeDir::new("/public")
                .not_found_service(axum::routing::any(not_found_handler))
        )
}

async fn index_handler() -> impl IntoResponse {
    // Return 404 for root path as requested - no index page needed
    not_found_handler().await
}

async fn health_handler() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    
    (StatusCode::OK, headers, r#"{"status":"healthy","service":"raworc-public-server"}"#)
}

async fn not_found_handler() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>404</title>
    <style>
        html, body {
            height: 100%;
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            align-items: center;
            font-family: Arial, sans-serif;
            background: white;
            color: black;
        }
        h1 {
            font-size: 72px;
            font-weight: normal;
            margin: 0;
        }
    </style>
</head>
<body>
    <h1>404</h1>
</body>
</html>
"#;

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/html; charset=utf-8")
        .body(html.to_string())
        .unwrap()
}