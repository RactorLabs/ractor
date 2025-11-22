use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct InferenceModelInfo {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InferenceProviderInfo {
    pub name: String,
    pub display_name: String,
    pub url: String,
    pub models: Vec<InferenceModelInfo>,
    pub default_model: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct InferenceRegistry {
    providers: Vec<InferenceProviderInfo>,
    default_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedInferenceTarget<'a> {
    pub provider: &'a InferenceProviderInfo,
    pub model: &'a str,
}

impl InferenceRegistry {
    pub fn new(mut providers: Vec<InferenceProviderInfo>) -> Result<Self> {
        if providers.is_empty() {
            return Err(anyhow!(
                "At least one inference provider must be configured"
            ));
        }

        let mut default_index = providers.iter().position(|p| p.is_default).unwrap_or(0);

        if providers.iter().filter(|p| p.is_default).count() > 1 {
            let mut found = false;
            for provider in providers.iter_mut() {
                if provider.is_default && !found {
                    found = true;
                } else {
                    provider.is_default = false;
                }
            }
            default_index = providers.iter().position(|p| p.is_default).unwrap_or(0);
        }

        Ok(Self {
            providers,
            default_index,
        })
    }

    pub fn providers(&self) -> &[InferenceProviderInfo] {
        &self.providers
    }

    pub fn default_provider(&self) -> &InferenceProviderInfo {
        &self.providers[self.default_index]
    }

    pub fn resolve_provider_and_model<'a>(
        &'a self,
        provider_name: Option<&str>,
        model_name: Option<&str>,
    ) -> Result<ResolvedInferenceTarget<'a>> {
        let provider = match provider_name {
            Some(name) if !name.trim().is_empty() => self
                .providers
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| anyhow!("Unknown inference provider '{}'", name))?,
            _ => self.default_provider(),
        };

        let model = match model_name {
            Some(model) if !model.trim().is_empty() => provider
                .models
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(model))
                .ok_or_else(|| {
                    anyhow!("Invalid model '{}' for provider '{}'", model, provider.name)
                })?
                .name
                .as_str(),
            _ => provider.default_model.as_str(),
        };

        Ok(ResolvedInferenceTarget { provider, model })
    }

    pub fn resolve_model<'a>(
        &'a self,
        provider_name: Option<&str>,
        model_name: Option<&str>,
    ) -> Result<&'a str> {
        Ok(self
            .resolve_provider_and_model(provider_name, model_name)?
            .model)
    }
}
