use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::info;

pub async fn request_logging_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start_time = Instant::now();

    let response = next.run(request).await;
    let duration = start_time.elapsed();

    info!(
        method = %method,
        path = %uri.path(),
        duration_ms = %duration.as_millis(),
        "Handled request"
    );

    Ok(response)
}

