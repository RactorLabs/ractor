use crate::config::{self, Config};
use anyhow::{bail, Result};
use std::io::{self, Write};

pub fn run(mut cfg: Config) -> Result<Config> {
    println!("TSBX needs provider details to connect to your inference endpoint.");
    println!("Press enter to accept the value in brackets.");
    println!();

    cfg.provider_name = prompt("Provider name", &cfg.provider_name)?;
    cfg.inference_url = prompt_required("Inference API URL", &cfg.inference_url)?;
    cfg.default_model = prompt("Default model", &cfg.default_model)?;
    cfg.api_key = prompt_required("Provider API key", &cfg.api_key)?;

    if cfg.api_key.trim().is_empty() {
        bail!("API key cannot be empty");
    }

    config::save(&cfg)?;
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
