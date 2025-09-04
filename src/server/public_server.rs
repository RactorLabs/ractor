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
    info!("Public Content HTTP server listening on http://0.0.0.0:8000");
    info!("Serving published content files from /public directory");

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
            ServeDir::new("/public").not_found_service(axum::routing::any(not_found_handler)),
        )
}

async fn index_handler() -> impl IntoResponse {
    // Return simple 200 OK response for home page
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Raworc Public Server</title>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        html, body {
            height: 100%;
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            align-items: center;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #f8f9fa;
            color: #333;
        }
        .container {
            text-align: center;
            padding: 40px;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
        h1 {
            font-size: 2.5em;
            font-weight: 300;
            margin: 0 0 20px 0;
            color: #2c3e50;
        }
        p {
            font-size: 1.1em;
            color: #7f8c8d;
            margin: 0;
        }
        .status {
            display: inline-block;
            background: #27ae60;
            color: white;
            padding: 4px 12px;
            border-radius: 12px;
            font-size: 0.9em;
            margin-top: 20px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Raworc Public Server</h1>
        <p>Public content server running on port 8000</p>
        <div class="status">Online</div>
    </div>
</body>
</html>
"#;

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html; charset=utf-8")
        .body(html.to_string())
        .unwrap()
}

async fn health_handler() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static("application/json"));

    (
        StatusCode::OK,
        headers,
        r#"{"status":"healthy","service":"raworc-public-server"}"#,
    )
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
