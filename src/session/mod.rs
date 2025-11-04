// Session (Computer Use Session) modules
mod api;
mod builtin_tools;
mod config;
mod error;
mod guardrails;
mod ollama;
mod task_handler;
mod tool_registry;
mod tools;

use anyhow::Result;
use std::sync::Arc;
use tracing::{error, info, warn};

pub async fn run(api_url: &str, session_name: &str) -> Result<()> {
    tracing::info!("Starting TaskSandbox Session...");
    tracing::info!("Connecting to API: {}", api_url);
    tracing::info!("Session Name: {}", session_name);

    // Log which principal this Session is running as
    if let Ok(principal) = std::env::var("TSBX_PRINCIPAL") {
        let principal_type =
            std::env::var("TSBX_PRINCIPAL_TYPE").unwrap_or_else(|_| "Unknown".to_string());
        tracing::info!("Running as principal: {} ({})", principal, principal_type);
    }

    // Use TSBX_TOKEN from environment (user's token set as secret)
    let api_token = std::env::var("TSBX_TOKEN")
        .map_err(|_| anyhow::anyhow!("TSBX_TOKEN environment variable is required"))?;

    // Debug: Log the TSBX token being used (partially masked for security)
    let masked_token = if api_token.len() > 20 {
        format!(
            "{}...{}",
            &api_token[..20],
            &api_token[api_token.len() - 8..]
        )
    } else {
        "<too-short>".to_string()
    };
    tracing::info!("Using TSBX_TOKEN: {}", masked_token);

    // Resolve Ollama host from environment; required (no default)
    let mut ollama_host = std::env::var("OLLAMA_HOST")
        .map_err(|_| anyhow::anyhow!("OLLAMA_HOST environment variable is required"))?;
    // Be tolerant of missing scheme in OLLAMA_HOST (e.g., "127.0.0.1:11434")
    if !(ollama_host.starts_with("http://") || ollama_host.starts_with("https://")) {
        ollama_host = format!("http://{}", ollama_host);
    }
    tracing::info!("Using OLLAMA_HOST: {}", ollama_host);

    // Initialize configuration
    let config = Arc::new(config::Config {
        session_name: session_name.to_string(),
        api_url: api_url.to_string(),
        api_token,
        polling_interval: std::time::Duration::from_secs(2),
    });

    // Initialize API client
    let api_client = Arc::new(api::TaskSandboxClient::new(config.clone()));

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

    // Initialize session directories
    let session_dirs = [
        "/session",
        "/session/code",
        "/session/content",
    ];

    for dir in session_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }

    // Ensure /session/bin exists and install command wrappers
    if let Err(e) = std::fs::create_dir_all("/session/bin") {
        warn!("Failed to create /session/bin: {}", e);
    } else {
        if let Err(e) = install_wrappers() {
            warn!("Failed to install wrappers: {}", e);
        }
    }

    // Wait for and execute setup script if it becomes available
    let setup_script = std::path::Path::new("/session/code/setup.sh");

    // Check if a setup script is expected based on environment variable
    let has_setup_script = std::env::var("TSBX_HAS_SETUP").is_ok();

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
        info!("Executing setup script: /session/code/setup.sh");
        match std::process::Command::new("bash")
            .arg("/session/code/setup.sh")
            .current_dir("/session")
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
        info!("No setup script found at /session/code/setup.sh");
    }

    // Set working directory to session directory
    if let Err(e) = std::env::set_current_dir("/session") {
        warn!("Failed to set working directory to /session: {}", e);
    } else {
        info!("Set working directory to /session");
    }

    // Initialize task handler
    let task_handler = task_handler::TaskHandler::new(
        api_client.clone(),
        ollama_client.clone(),
        guardrails.clone(),
    );

    // Initialize processed task tracking to prevent reprocessing on restore
    if let Err(e) = task_handler.initialize_processed_tracking().await {
        warn!(
            "Failed to initialize processed tracking: {}, proceeding anyway",
            e
        );
    }

    // Log published content URL hint
    let base_url = std::env::var("TSBX_HOST_URL")
        .expect("TSBX_HOST_URL must be set by the start script")
        .trim_end_matches('/')
        .to_string();
    let host_name = std::env::var("TSBX_HOST_NAME").unwrap_or_else(|_| "TaskSandbox".to_string());
    let operator_url = format!("{}", base_url);
    let api_url = format!("{}/api", base_url);
    info!("{} environment detected", host_name);
    info!("Operator: {}", operator_url);
    info!("API: {}", api_url);

    // Set initial state thoughtfully: don't clobber a pre-set busy state
    match api_client.get_session().await {
        Ok(session_info) => {
            let state = session_info.state.to_lowercase();
            if state == "busy" {
                info!("Skipping initial idle update because session is marked busy");
            } else if state == "stopped" {
                info!("Skipping initial idle update because session is stopped");
            } else {
                info!("Setting session to idle to start timeout...");
                if let Err(e) = api_client.update_session_to_idle().await {
                    warn!("Failed to set session to idle after initialization: {}", e);
                } else {
                    info!("Session set to idle - timeout started");
                }
            }
        }
        Err(e) => {
            warn!(
                "Could not fetch session state on startup (will proceed): {}",
                e
            );
        }
    }

    info!("Starting task polling loop...");

    // Main polling loop with comprehensive error handling
    loop {
        match task_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} tasks", count);
                }
            }
            Err(e) => {
                error!("Error processing tasks: {}", e);
                // Continue polling - session should never die silently
            }
        }

        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

// Content preview server removed.

fn install_wrappers() -> anyhow::Result<()> {
    write_exec("/session/bin/ls", LS_WRAPPER)?;
    write_exec("/session/bin/rg", RG_WRAPPER)?;
    write_exec("/session/bin/fd", FD_WRAPPER)?;
    Ok(())
}

fn write_exec(path: &str, content: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, content)?;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

const LS_WRAPPER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
_bin="/bin/ls"
if [[ ! -x "$_bin" ]]; then _bin="/usr/bin/ls"; fi
EXCLUDES=(
  node_modules .venv venv target dist build .cache __pycache__
  .svelte-kit .next logs .pytest_cache .mypy_cache .ruff_cache
  pip-wheel-metadata .tox .git
)
args=()
for ex in "${EXCLUDES[@]}"; do args+=( -I "$ex" ); done
exec "$_bin" "${args[@]}" "$@"
"#;

const RG_WRAPPER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
_bin="/usr/bin/rg"
GLOBS=(
  "!**/node_modules/**" "!**/.venv/**" "!**/venv/**" "!**/target/**" "!**/dist/**" "!**/build/**"
  "!**/.cache/**" "!**/__pycache__/**" "!**/.svelte-kit/**" "!**/.next/**" "!**/logs/**" "!**/.pytest_cache/**"
  "!**/.mypy_cache/**" "!**/.ruff_cache/**" "!**/pip-wheel-metadata/**" "!**/.tox/**" "!**/.git/**"
  "!**/*.pyc" "!**/*.pyo" "!**/*.o" "!**/*.so" "!**/*.a" "!**/*.class"
)
args=()
for g in "${GLOBS[@]}"; do args+=( -g "$g" ); done
exec "$_bin" "${args[@]}" "$@"
"#;

const FD_WRAPPER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
_bin="/usr/bin/fd"
EXCLUDES=(
  node_modules .venv venv target dist build .cache __pycache__
  .svelte-kit .next logs .pytest_cache .mypy_cache .ruff_cache
  pip-wheel-metadata .tox .git
  "*.pyc" "*.pyo" "*.o" "*.so" "*.a" "*.class"
)
args=()
for ex in "${EXCLUDES[@]}"; do args+=( --exclude "$ex" ); done
exec "$_bin" "${args[@]}" "$@"
"#;
