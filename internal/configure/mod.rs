use crate::config::{self, Config};
use anyhow::{bail, Context, Result};
use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn run(mut cfg: Config) -> Result<Config> {
    println!("TSBX needs provider details to connect to your inference endpoint.");
    println!("Press enter to accept the value in brackets.");
    println!();

    cfg.provider_name = prompt("Provider name", &cfg.provider_name)?;
    cfg.inference_url = prompt_required("Inference API URL", &cfg.inference_url)?;
    cfg.default_model = prompt("Default model", &cfg.default_model)?;
    cfg.api_key = prompt_required("Provider API key", &cfg.api_key)?;
    let sandbox_prompt = if cfg.sandbox_dir.trim().is_empty() {
        default_workspace_hint()
    } else {
        cfg.sandbox_dir.clone()
    };
    let sandbox_input = prompt("Sandbox workspace directory", &sandbox_prompt)?;
    cfg.sandbox_dir = normalize_workspace_dir(&sandbox_input)?;

    if cfg.api_key.trim().is_empty() {
        bail!("API key cannot be empty");
    }

    config::save(&cfg)?;
    if !cfg.sandbox_dir.trim().is_empty() && !Path::new(&cfg.sandbox_dir).exists() {
        println!(
            "Warning: sandbox workspace '{}' does not exist yet.",
            cfg.sandbox_dir
        );
    }
    println!();
    println!("Configuration complete.");
    Ok(cfg)
}

fn prompt(label: &str, current: &str) -> Result<String> {
    prompt_internal(label, current, false)
}

fn prompt_required(label: &str, current: &str) -> Result<String> {
    prompt_internal(label, current, true)
}

fn prompt_internal(label: &str, current: &str, required: bool) -> Result<String> {
    let rendered = if current.is_empty() {
        label.to_string()
    } else {
        format!("{label} [{current}]")
    };

    loop {
        print!("{rendered}: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            if required && current.trim().is_empty() {
                println!("Value is required.");
                continue;
            }
            return Ok(current.to_string());
        }
        return Ok(trimmed);
    }
}

fn normalize_workspace_dir(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let expanded = expand_home(trimmed);
    let path = PathBuf::from(&expanded);
    let absolute = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .with_context(|| "determine current directory")?
            .join(path)
    };
    Ok(absolute.to_string_lossy().into_owned())
}

fn expand_home(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{stripped}");
        }
    } else if value == "~" {
        if let Ok(home) = env::var("HOME") {
            return home;
        }
    }
    value.to_string()
}

fn default_workspace_hint() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let candidate = Path::new(&home).join("repos/tsbx");
    if candidate.exists() {
        candidate.to_string_lossy().into_owned()
    } else {
        String::new()
    }
}
