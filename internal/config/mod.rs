use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::SystemTime;

const CONFIG_RELATIVE_DIR: &str = ".config/tsbx";
const LOG_SUBDIR: &str = "logs";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider_name: String,
    pub inference_url: String,
    pub default_model: String,
    pub api_key: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub sandbox_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        let now = timestamp();
        Self {
            provider_name: String::new(),
            inference_url: String::new(),
            default_model: String::new(),
            api_key: String::new(),
            created_at: now.clone(),
            updated_at: now,
            sandbox_dir: default_sandbox_dir(),
        }
    }
}

pub fn load_or_default() -> Result<Config> {
    ensure_dirs()?;
    let path = config_file();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut cfg: Config =
        serde_json::from_str(&contents).with_context(|| format!("parse {}", path.display()))?;
    if cfg.created_at.trim().is_empty() {
        cfg.created_at = timestamp();
    }
    if cfg.updated_at.trim().is_empty() {
        cfg.updated_at = timestamp();
    }
    if cfg.sandbox_dir.trim().is_empty() {
        cfg.sandbox_dir = default_sandbox_dir();
    }
    Ok(cfg)
}

pub fn save(cfg: &Config) -> Result<()> {
    ensure_dirs()?;
    let mut cfg = cfg.clone();
    if cfg.created_at.trim().is_empty() {
        cfg.created_at = timestamp();
    }
    cfg.updated_at = timestamp();
    let json = serde_json::to_string_pretty(&cfg)?;
    let path = config_file();
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, json.as_bytes()).with_context(|| format!("write {}", tmp.display()))?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).ok();
    fs::rename(&tmp, &path).with_context(|| format!("persist {}", path.display()))?;
    Ok(())
}

pub fn ensure_dirs() -> Result<()> {
    let cfg_dir = config_dir();
    fs::create_dir_all(&cfg_dir).with_context(|| format!("create {}", cfg_dir.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&cfg_dir, fs::Permissions::from_mode(0o700)).ok();
    let logs = logs_dir();
    fs::create_dir_all(&logs).with_context(|| format!("create {}", logs.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o700)).ok();
    Ok(())
}

pub fn config_dir() -> PathBuf {
    home_dir().join(CONFIG_RELATIVE_DIR)
}

pub fn logs_dir() -> PathBuf {
    config_dir().join(LOG_SUBDIR)
}

pub fn config_file() -> PathBuf {
    config_dir().join(CONFIG_FILE)
}

pub fn open_log_file(name: &str) -> Result<fs::File> {
    ensure_dirs()?;
    let path = logs_dir().join(name);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).ok();
    Ok(file)
}

fn timestamp() -> String {
    chrono::DateTime::<chrono::Utc>::from(SystemTime::now()).to_rfc3339()
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn default_sandbox_dir() -> String {
    let mut candidate = home_dir();
    candidate.push("repos/tsbx");
    if candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    String::new()
}
