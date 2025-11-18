use crate::config::{self, Config};
use anyhow::{bail, Context, Result};
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;

pub fn start(cfg: &Config) -> Result<()> {
    if cfg.api_key.trim().is_empty() || cfg.inference_url.trim().is_empty() {
        bail!("Missing provider configuration; run `tsbx configure` first.");
    }
    if cfg.tsbx_api_url.trim().is_empty()
        || cfg.sandbox_id.trim().is_empty()
        || cfg.tsbx_token.trim().is_empty()
    {
        bail!("Missing TSBX API configuration; run `tsbx configure` and supply API URL, sandbox ID, and token.");
    }
    if cfg.sandbox_dir.trim().is_empty() {
        bail!("Sandbox workspace directory is not configured; rerun `tsbx configure`.");
    }
    if !Path::new(cfg.sandbox_dir.trim()).exists() {
        bail!(
            "Sandbox workspace directory '{}' does not exist.",
            cfg.sandbox_dir
        );
    }

    config::ensure_dirs()?;
    let log_name = format!(
        "sandbox-{}.log",
        chrono::DateTime::<chrono::Utc>::from(SystemTime::now()).format("%Y%m%d-%H%M%S")
    );
    let log_file = config::open_log_file(&log_name)?;
    println!("Starting a new TSBX sandbox…");
    println!(
        "Writing boot logs to {}",
        config::logs_dir().join(&log_name).display()
    );

    let command = env::var("TSBX_SANDBOX_COMMAND")
        .ok()
        .unwrap_or_else(|| build_default_command(cfg));

    let mut child = Command::new("bash")
        .arg("-c")
        .arg(&command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TSBX_PROVIDER_NAME", cfg.provider_name.as_str())
        .env("TSBX_INFERENCE_URL", cfg.inference_url.as_str())
        .env("TSBX_DEFAULT_MODEL", cfg.default_model.as_str())
        .env("TSBX_API_KEY", cfg.api_key.as_str())
        .env("TSBX_TOKEN", cfg.tsbx_token.as_str())
        .env("TSBX_API_URL", cfg.tsbx_api_url.as_str())
        .env("SANDBOX_ID", cfg.sandbox_id.as_str())
        .spawn()
        .with_context(|| format!("unable to run command: {command}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log = Arc::new(Mutex::new(log_file));

    let mut handles = Vec::new();
    if let Some(out) = stdout {
        handles.push(spawn_pipe(out, io::stdout(), Arc::clone(&log)));
    }
    if let Some(err) = stderr {
        handles.push(spawn_pipe(err, io::stderr(), Arc::clone(&log)));
    }

    let status = child.wait()?;
    for handle in handles {
        let _ = handle.join();
    }

    if !status.success() {
        bail!("Sandbox exited with status {}", status);
    }
    Ok(())
}

fn spawn_pipe<R>(
    mut reader: R,
    mut console: impl Write + Send + 'static,
    log: Arc<Mutex<File>>,
) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buffer[..n];
                    let _ = console.write_all(chunk);
                    let _ = console.flush();
                    if let Ok(mut file) = log.lock() {
                        let _ = file.write_all(chunk);
                        let _ = file.flush();
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn shell_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() + 2);
    escaped.push('\'');
    for ch in input.chars() {
        if ch == '\'' {
            escaped.push_str("'\"'\"'");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}

fn build_default_command(cfg: &Config) -> String {
    format!(
        "cd {workspace} && cargo run --release --bin tsbx-sandbox -- --api-url {api_url} --sandbox-id {sandbox_id}",
        workspace = shell_escape(cfg.sandbox_dir.trim()),
        api_url = shell_escape(cfg.tsbx_api_url.trim()),
        sandbox_id = shell_escape(cfg.sandbox_id.trim()),
    )
}
