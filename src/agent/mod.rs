// Agent (Computer Use Agent) modules
mod api;
mod builtin_tools;
mod config;
mod error;
mod guardrails;
mod ollama;
mod response_handler;
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
    let agent_dirs = ["/agent", "/agent/code", "/agent/content", "/agent/template"];

    for dir in agent_dirs.iter() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Failed to create directory {}: {}", dir, e);
        }
    }

    // Ensure /agent/bin exists and install command wrappers
    if let Err(e) = std::fs::create_dir_all("/agent/bin") {
        warn!("Failed to create /agent/bin: {}", e);
    } else {
        if let Err(e) = install_wrappers() {
            warn!("Failed to install wrappers: {}", e);
        }
    }

    // No separate content preview server; content is published via raworc-content.

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

    // Initialize response handler
    let message_handler = response_handler::ResponseHandler::new(
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

    // Log published content URL hint
    let base_url = std::env::var("RAWORC_HOST_URL")
        .expect("RAWORC_HOST_URL must be set by the start script")
        .trim_end_matches('/')
        .to_string();
    let host_name = std::env::var("RAWORC_HOST_NAME").unwrap_or_else(|_| "Raworc".to_string());
    let operator_url = format!("{}", base_url);
    let api_url = format!("{}/api", base_url);
    let published_url = format!("{}/content/{}/", base_url, agent_name);
    info!("{} environment detected", host_name);
    info!("Operator: {}", operator_url);
    info!("API: {}", api_url);
    info!("Content folder: /agent/content/ - Publish when ready to share");
    info!("Templates folder: /agent/template/ - Default HTML templates");
    info!("Published content (when published): {}", published_url);

    // Set initial state thoughtfully: don't clobber a pre-set busy state
    match api_client.get_agent().await {
        Ok(agent_info) => {
            let state = agent_info.state.to_lowercase();
            if state == "busy" {
                info!("Skipping initial idle update because agent is marked busy");
            } else if state == "slept" {
                info!("Skipping initial idle update because agent is slept");
            } else {
                info!("Setting agent to idle to start timeout...");
                if let Err(e) = api_client.update_agent_to_idle().await {
                    warn!("Failed to set agent to idle after initialization: {}", e);
                } else {
                    info!("Agent set to idle - timeout started");
                }
            }
        }
        Err(e) => {
            warn!(
                "Could not fetch agent state on startup (will proceed): {}",
                e
            );
        }
    }

    info!("Starting response polling loop...");

    // Main polling loop with comprehensive error handling
    loop {
        match message_handler.poll_and_process().await {
            Ok(count) => {
                if count > 0 {
                    info!("Processed {} responses", count);
                }
            }
            Err(e) => {
                error!("Error processing responses: {}", e);
                // Continue polling - agent should never die silently
            }
        }

        // Wait before next poll
        tokio::time::sleep(config.polling_interval).await;
    }
}

// Content preview server removed.

fn install_wrappers() -> anyhow::Result<()> {
    write_exec("/agent/bin/ls", LS_WRAPPER)?;
    write_exec("/agent/bin/rg", RG_WRAPPER)?;
    write_exec("/agent/bin/fd", FD_WRAPPER)?;
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
