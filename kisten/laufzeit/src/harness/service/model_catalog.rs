//! Terminal-independent projection and selection of user-owned model profiles.

use std::fmt;

use crate::harness::config::{ConfigError, ResolvedModelProfile, UserConfig};
use thiserror::Error;

const MAX_DISPLAY_FIELD_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentModelCatalog {
    pub configured: Option<SelfAgentModelChoice>,
    pub profiles: Vec<SelfAgentModelChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentModelChoice {
    pub profile: Option<String>,
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub plan_reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
}

#[derive(Debug, Error)]
pub enum SelfAgentModelCatalogError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("configured model is unavailable")]
    ConfiguredModelUnavailable,
}

#[derive(Clone, Default)]
pub struct SelfAgentModelSession {
    selected_profile: Option<String>,
}

impl SelfAgentModelSession {
    pub fn catalog(
        &self,
        config: &UserConfig,
    ) -> Result<SelfAgentModelCatalog, SelfAgentModelCatalogError> {
        let mut catalog = load_self_agent_model_catalog(config)?;
        if let Some(name) = self.selected_profile.as_deref() {
            let (_, choice) = select_self_agent_model_profile(config, name)?;
            catalog.configured = Some(choice);
        }
        Ok(catalog)
    }

    pub fn effective_config(
        &self,
        config: &UserConfig,
    ) -> Result<UserConfig, SelfAgentModelCatalogError> {
        match self.selected_profile.as_deref() {
            Some(name) => select_self_agent_model_profile(config, name).map(|(config, _)| config),
            None => Ok(config.clone()),
        }
    }

    pub fn select_profile(
        &mut self,
        config: &UserConfig,
        name: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentModelCatalogError> {
        let (_, choice) = select_self_agent_model_profile(config, name)?;
        self.selected_profile = choice.profile.clone();
        Ok(choice)
    }

    pub fn select_configured(
        &mut self,
        config: &UserConfig,
    ) -> Result<SelfAgentModelChoice, SelfAgentModelCatalogError> {
        let choice = load_self_agent_model_catalog(config)?
            .configured
            .ok_or(SelfAgentModelCatalogError::ConfiguredModelUnavailable)?;
        self.selected_profile = None;
        Ok(choice)
    }

    pub fn selected_profile(&self) -> Option<&str> {
        self.selected_profile.as_deref()
    }
}

impl fmt::Debug for SelfAgentModelSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentModelSession")
            .field("named_profile_selected", &self.selected_profile.is_some())
            .finish()
    }
}

pub fn load_self_agent_model_catalog(
    config: &UserConfig,
) -> Result<SelfAgentModelCatalog, SelfAgentModelCatalogError> {
    let configured = if config.model_provider.is_none() && config.model.is_none() {
        None
    } else {
        Some(choice_from_resolved(None, config.resolve_model_profile()?)?)
    };
    let mut profiles = Vec::with_capacity(config.model_profiles().len());
    for name in config.model_profiles().keys() {
        let (_, choice) = select_self_agent_model_profile(config, name)?;
        profiles.push(choice);
    }
    Ok(SelfAgentModelCatalog {
        configured,
        profiles,
    })
}

pub fn select_self_agent_model_profile(
    config: &UserConfig,
    name: &str,
) -> Result<(UserConfig, SelfAgentModelChoice), SelfAgentModelCatalogError> {
    let selected = config.with_model_profile(name)?;
    let resolved = selected.resolve_model_profile()?;
    let choice = choice_from_resolved(Some(name.to_owned()), resolved)?;
    Ok((selected, choice))
}

fn choice_from_resolved(
    profile: Option<String>,
    resolved: ResolvedModelProfile,
) -> Result<SelfAgentModelChoice, ConfigError> {
    let ResolvedModelProfile {
        provider,
        provider_name,
        model,
        base_url: _,
        wire_api: _,
        reasoning_effort,
        plan_mode_reasoning_effort,
        store: _,
        service_tier,
        requires_auth: _,
    } = resolved;
    validate_display_field(&provider, "model_provider")?;
    validate_display_field(&provider_name, "model_provider_name")?;
    validate_display_field(&model, "model")?;
    for (path, value) in [
        ("model_reasoning_effort", reasoning_effort.as_deref()),
        (
            "plan_mode_reasoning_effort",
            plan_mode_reasoning_effort.as_deref(),
        ),
        ("service_tier", service_tier.as_deref()),
    ] {
        if let Some(value) = value {
            validate_display_field(value, path)?;
        }
    }
    Ok(SelfAgentModelChoice {
        profile,
        provider,
        provider_name,
        model,
        reasoning_effort,
        plan_reasoning_effort: plan_mode_reasoning_effort,
        service_tier,
    })
}

fn validate_display_field(value: &str, path: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty()
        || value.len() > MAX_DISPLAY_FIELD_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ConfigError::Validation {
            path: path.into(),
            message: "model catalog metadata is invalid".into(),
        });
    }
    Ok(())
}
