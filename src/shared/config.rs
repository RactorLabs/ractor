use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::shared::inference::{InferenceModelInfo, InferenceProviderInfo, InferenceRegistry};

#[derive(Debug, Clone, Deserialize)]
pub struct TsbxConfig {
    #[serde(default)]
    pub host: HostConfig,
    #[serde(default)]
    pub inference_providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HostConfig {
    #[serde(default = "default_host_name")]
    pub name: String,
    #[serde(default = "default_host_url")]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub url: String,
    #[serde(default)]
    pub models: Vec<ProviderModel>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderModel {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

impl TsbxConfig {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let data = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config at {}: {}", path.display(), e))?;
        let mut config: TsbxConfig = serde_json::from_str(&data)
            .map_err(|e| anyhow!("Failed to parse config JSON at {}: {}", path.display(), e))?;

        config.host.name = config.host.name.trim().to_string();
        if config.host.name.is_empty() {
            config.host.name = default_host_name();
        }

        config.host.url = config.host.url.trim().trim_end_matches('/').to_string();
        if config.host.url.is_empty() {
            config.host.url = default_host_url();
        }

        Ok(config)
    }

    pub fn load_default() -> Result<(Self, PathBuf)> {
        let path = resolve_config_path();
        let config = Self::load_from_path(&path)?;
        Ok((config, path))
    }

    pub fn build_inference_registry(&self) -> Result<InferenceRegistry> {
        let providers = self
            .inference_providers
            .iter()
            .map(|raw| raw.to_provider_info())
            .collect::<Result<Vec<_>>>()?;
        InferenceRegistry::new(providers)
    }
}

impl ProviderConfig {
    fn to_provider_info(&self) -> Result<InferenceProviderInfo> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(anyhow!("Inference provider name must not be empty"));
        }

        let url = self.url.trim();
        if url.is_empty() {
            return Err(anyhow!(
                "Inference provider '{}' is missing a URL",
                self.name
            ));
        }

        if self.models.is_empty() {
            return Err(anyhow!(
                "Inference provider '{}' must define at least one model",
                self.name
            ));
        }

        let models: Vec<InferenceModelInfo> = self
            .models
            .iter()
            .map(|model| {
                let model_name = model.name.trim();
                if model_name.is_empty() {
                    return Err(anyhow!(
                        "Inference provider '{}' has a model without a name",
                        self.name
                    ));
                }
                Ok(InferenceModelInfo {
                    name: model_name.to_string(),
                    display_name: model
                        .display_name
                        .as_ref()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| model_name.to_string()),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if models.is_empty() {
            return Err(anyhow!(
                "Inference provider '{}' must define at least one valid model",
                self.name
            ));
        }

        let default_model = self
            .default_model
            .as_deref()
            .and_then(|value| {
                models
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(value))
                    .map(|m| m.name.clone())
            })
            .unwrap_or_else(|| models[0].name.clone());

        Ok(InferenceProviderInfo {
            name: name.to_string(),
            display_name: self
                .display_name
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| name.to_string()),
            url: url.to_string(),
            models,
            default_model,
            is_default: self.default,
        })
    }
}

fn default_host_name() -> String {
    "TSBX".to_string()
}

fn default_host_url() -> String {
    "http://localhost".to_string()
}

pub fn resolve_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("TSBX_CONFIG_PATH") {
        return expand_path(path);
    }

    default_config_path()
}

fn expand_path(input: String) -> PathBuf {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    } else if let Some(stripped) = input.strip_prefix("~\\") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(input)
}

fn default_config_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tsbx")
        .join("tsbx.json")
}

fn home_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}
