// Agent (Computer Use Agent) modules
mod api;
mod builtin_tools;
mod config;
mod error;
mod guardrails;
mod message_handler;
mod ollama;
mod tool_registry;
mod tools;

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

pub async fn run(api_url: &str, agent_name: &str) -> Result<()> {
    tracing::info!("Starting Raworc Agent...");
    tracing::info!("Connecting to API: {}", api_url);
    tracing::info!("Agent Name: {}", agent_name);

    // Log which principal this Agent is running as
    if let Ok(principal) = std::env::var("RAWORC_PRINCIPAL") {
        let principal_type =
            std::env::var("RAWORC_PRINCIPAL_TYPE").unwrap_or_else(|_| "Unknown".to_string());
        tracing::info!("Running as principal: {} ({})", principal, principal_type);
    }

    // Use RAWORC_TOKEN from environment (user's token set as secret)
    let api_token = std::env::var("RAWORC_TOKEN")
        .map_err(|_| anyhow::anyhow!("RAWORC_TOKEN environment variable is required"))?;

    // Debug: Log the RAWORC token being used (partially masked for security)
    let masked_token = if api_token.len() > 20 {
        format!(
            "{}...{}",
            &api_token[..20],
            &api_token[api_token.len() - 8..]
        )
    } else {
        "<too-short>".to_string()
    };
    tracing::info!("Using RAWORC_TOKEN: {}", masked_token);

    // Resolve Ollama host from environment or default to host.docker.internal
    let ollama_host = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://host.docker.internal:11434".to_string());
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize configuration
    let config = Arc::new(config::Config {
        agent_name: agent_name.to_string(),
        api_url: api_url.to_string(),
        api_token,
        polling_interval: std::time::Duration::from_secs(2),
    });

    // Initialize API client
    let api_client = Arc::new(api::RaworcClient::new(config.clone()));

    // Initialize Ollama client
    let ollama_client = match ollama::OllamaClient::new(&ollama_host) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to initialize Ollama client: {}", e);
            return Err(anyhow::anyhow!("Failed to initialize Ollama client: {}", e));
        }
    };
    let ollama_client = Arc::new(ollama_client);

    // Initialize guardrails
    let guardrails = Arc::new(guardrails::Guardrails::new());

    // Initialize agent directories
    let agent_dirs = ["/agent", "/agent/code", "/agent/secrets", "/agent/content"];

    for dir in agent_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }

    // Start Content HTTP server on port 8000
    info!("Starting Content HTTP server on port 8000 (mounted under /content)...");
    tokio::spawn(async {
        if let Err(e) = start_content_server().await {
            error!("Content HTTP server failed: {}", e);
        }
    });

    // Wait for and execute setup script if it becomes available
    let setup_script = std::path::Path::new("/agent/code/setup.sh");

    // Check if a setup script is expected based on environment variable
    let has_setup_script = std::env::var("RAWORC_HAS_SETUP").is_ok();

    if has_setup_script {
        // Setup script is expected, wait up to 2 seconds for it to be written by controller
        info!("Setup script expected, waiting for it to be created...");
        let mut attempts = 0;
        let max_attempts = 4; // 2 seconds with 500ms intervals

        while !setup_script.exists() && attempts < max_attempts {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            attempts += 1;
        }
    } else {
        // No setup script expected, check once and proceed
        info!("No setup script expected, checking once...");
    }

    if setup_script.exists() {
        info!("Executing setup script: /agent/code/setup.sh");
        match std::process::Command::new("bash")
            .arg("/agent/code/setup.sh")
            .current_dir("/agent")
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Setup script executed successfully");
                    if !output.stdout.is_empty() {
                        info!("Setup stdout: {}", String::from_utf8_lossy(&output.stdout));
                    }
                } else {
                    error!(
                        "Setup script failed with exit code: {:?}",
                        output.status.code()
                    );
                    if !output.stderr.is_empty() {
                        error!("Setup stderr: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
            }
            Err(e) => {
                error!("Failed to execute setup script: {}", e);
            }
        }
    } else {
        info!("No setup script found at /agent/code/setup.sh");
    }

    // Set working directory to agent directory
    if let Err(e) = std::env::set_current_dir("/agent") {
        warn!("Failed to set working directory to /agent: {}", e);
    } else {
        info!("Set working directory to /agent");
    }

    // Initialize message handler
    let message_handler = message_handler::MessageHandler::new(
        api_client.clone(),
        ollama_client.clone(),
        guardrails.clone(),
    );

    // Initialize processed message tracking to prevent reprocessing on restore
    if let Err(e) = message_handler.initialize_processed_tracking().await {
        warn!(
            "Failed to initialize processed tracking: {}, proceeding anyway",
            e
        );
    }

    info!("Agent initialized, getting Content port information...");

    // Get agent info to display Content URL
    match api_client.get_agent().await {
        Ok(agent) => {
            if let Some(content_port) = agent.content_port {
                // Prefer RAWORC_HOST_URL (injected by start script) for user-facing base URL
                let base_url = std::env::var("RAWORC_HOST_URL")
                    .expect("RAWORC_HOST_URL must be set by the start script")
                    .trim_end_matches('/')
                    .to_string();

                let host_name = std::env::var("RAWORC_HOST_NAME")
                    .unwrap_or_else(|_| "Raworc".to_string());

                let operator_url = format!("{}", base_url);
                let api_url = format!("{}/api", base_url);
                // Live server now mounts content under /content for path consistency with the gateway
                let live_url = format!("{}:{}/content/", base_url, content_port);

                info!("{} environment detected", host_name);
                info!("Operator: {}", operator_url);
                info!("API: {}", api_url);
                info!("Live content: {}", live_url);
                // Note: published URL is intentionally not logged by default to avoid
                // leaking it unless explicitly requested by the user.
                info!("Content folder: /agent/content/ - Create HTML files here for visual displays");
            } else {
                warn!("Content port not available for this agent");
            }
        }
        Err(e) => {
            warn!("Failed to get agent info for Content port: {}", e);
        }
    }

    info!("Setting agent to idle to start timeout...");

    // Set agent to idle after initialization to start timeout
    if let Err(e) = api_client.update_agent_to_idle().await {
        warn!("Failed to set agent to idle after initialization: {}", e);
    } else {
        info!("Agent set to idle - timeout started");
    }

    info!("Starting message polling loop...");

    // Main polling loop with comprehensive error handling
    loop {
        match message_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} messages", count);
                }
            }
            Err(e) => {
                error!("Error processing messages: {}", e);

                // Send error as message to user instead of crashing
                let error_message = format!("Agent encountered an error: {}", e);
                if let Err(send_err) = api_client.send_message(error_message, None).await {
                    error!("Failed to send error message to user: {}", send_err);
                } else {
                    info!("Sent error message to user, continuing operation");
                }

                // Continue polling - agent should never die silently
            }
        }

        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

async fn start_content_server() -> Result<()> {
    use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE};
    use hyper::service::{make_service_fn, service_fn};
    use hyper::{Body, Request, Response, Server, StatusCode};
    use std::convert::Infallible;
    use std::net::SocketAddr;
    use std::path::Path;
    use tokio::fs;

    async fn serve_content(req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let path = req.uri().path().to_string();

        // Security: prevent path traversal
        if path.contains("..") {
            return Ok(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::from("Forbidden"))
                .unwrap());
        }

        // Redirect root to /content/ for consistency
        if path == "/" {
            return Ok(Response::builder()
                .status(StatusCode::FOUND)
                .header("Location", "/content/")
                .body(Body::empty())
                .unwrap());
        }

        // Only serve under /content/*
        if !path.starts_with("/content/") {
            return Ok(Response::builder()
                .status(StatusCode::FOUND)
                .header("Location", "/content/")
                .body(Body::empty())
                .unwrap());
        }

        // Map /content/... -> /agent/content/...
        let stripped = path.trim_start_matches("/content/");
        let file_path_owned = if stripped.is_empty() {
            "/agent/content/index.html".to_string()
        } else {
            format!("/agent/content/{}", stripped)
        };
        let file_path = file_path_owned.as_str();

        // Check if file exists and serve it
        if Path::new(file_path).exists() {
            match fs::read(file_path).await {
                Ok(contents) => {
                    // Determine content type based on file extension
                    let content_type = match Path::new(file_path)
                        .extension()
                        .and_then(|ext| ext.to_str())
                    {
                        Some("html") => "text/html; charset=utf-8",
                        Some("css") => "text/css",
                        Some("js") => "application/javascript",
                        Some("json") => "application/json",
                        Some("png") => "image/png",
                        Some("jpg") | Some("jpeg") => "image/jpeg",
                        Some("gif") => "image/gif",
                        Some("svg") => "image/svg+xml",
                        Some("ico") => "image/x-icon",
                        _ => "application/octet-stream",
                    };

                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, content_type)
                        .header(CONTENT_LENGTH, contents.len())
                        .body(Body::from(contents))
                        .unwrap())
                }
                Err(_) => Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal Server Error"))
                    .unwrap()),
            }
        } else {
            // Return a full-page 404-style response with a helpful message
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
    </div>
  </body>
</html>"#;

            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(html))
                .unwrap())
        }
    }

    let make_svc =
        make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(serve_content)) });

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let server = Server::bind(&addr).serve(make_svc);

    info!("Content HTTP server listening on http://0.0.0.0:8000 (URL prefix: /content)");
    info!("Content directory: /agent/content/ mapped to /content");

    if let Err(e) = server.await {
        error!("Content HTTP server error: {}", e);
        return Err(anyhow::anyhow!("Content HTTP server failed: {}", e));
    }

    Ok(())
}
