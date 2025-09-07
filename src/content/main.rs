use anyhow::Result;
use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::fs;
use tower_http::services::ServeDir;
use tracing::{info, warn};
#[path = "../shared/mod.rs"]
mod shared;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging under content service name
    let _ = shared::logging::init_service_logging("/app/logs", "raworc_content");

    let app = create_app().await;
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    info!("Content server listening on http://0.0.0.0:8000");
    info!("Serving published content from /content directory");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_app() -> Router {
    // Ensure /content exists
    if let Err(e) = fs::create_dir_all("/content").await {
        warn!("Failed to create /content directory: {}", e);
    }

    // Build a router that is mounted under /content
    let content_router = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .fallback_service(
            ServeDir::new("/content").not_found_service(axum::routing::any(not_found_handler)),
        );

    Router::new().nest("/content", content_router)
}

async fn index_handler() -> impl IntoResponse {
    StatusCode::OK
}

async fn health_handler() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    (
        StatusCode::OK,
        headers,
        r#"{"status":"healthy","service":"raworc-content"}"#,
    )
}

async fn not_found_handler() -> impl IntoResponse {
    let html = r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset=\"utf-8\" />
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
    <title>No Content</title>
    <style>
      html, body { height: 100%; margin: 0; }
      body { display: flex; align-items: center; justify-content: center; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, 'Noto Sans', 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol'; background: #fff; color: #111; }
      .wrap { text-align: center; padding: 24px; }
      h1 { font-size: 28px; margin: 0 0 8px; font-weight: 600; }
      p { margin: 0; color: rgba(0,0,0,0.6); }
    </style>
  </head>
  <body>
    <div class=\"wrap\">
      <h1>No Content</h1>
      <p>This agent has no published content.</p>
    </div>
  </body>
</html>"#;
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("content-type", "text/html; charset=utf-8")
        .body(html.to_string())
        .unwrap()
}
