//! Terminal-independent projection and selection of user-owned model profiles.

use std::fmt;

use super::unresolved::unresolved_metadata;
use crate::harness::config::{ConfigError, ResolvedModelProfile, UserConfig};
use thiserror::Error;

const MAX_DISPLAY_FIELD_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentModelCatalog {
    pub active: SelfAgentActiveModel,
    pub profiles: Vec<SelfAgentModelChoice>,
    /// Every entry of the user's `model_providers` block, ordered by key.
    pub providers: Vec<SelfAgentProviderChoice>,
    /// The provider this session switched to, if any. A provider switch keeps
    /// the configured model and profile, so [`Self::active`] alone cannot say
    /// which provider is current.
    pub selected_provider: Option<String>,
}

/// The model the next turn would use, as presented to a read-only caller.
///
/// Listing the catalog is how an operator finds a working profile, so an
/// active model that cannot be resolved is reported rather than raised.
/// Named profiles stay strict by contrast: offering a choice that
/// [`select_self_agent_model_profile`] would refuse is worse than saying so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfAgentActiveModel {
    /// Neither a provider nor a model is configured.
    NotConfigured,
    /// Model fields are present but do not form a usable profile. Both members
    /// are validation metadata and carry no configured value.
    Unresolved { path: String, message: String },
    /// A complete, selectable choice.
    Configured(SelfAgentModelChoice),
}

impl SelfAgentActiveModel {
    /// The choice a caller may act on, if the active model is usable.
    pub fn choice(&self) -> Option<&SelfAgentModelChoice> {
        match self {
            Self::Configured(choice) => Some(choice),
            Self::NotConfigured | Self::Unresolved { .. } => None,
        }
    }
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

/// One entry of the user's `model_providers` block, as an offered choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentProviderChoice {
    /// The configured key, which is also what selection takes.
    pub provider: String,
    pub provider_name: String,
    pub state: SelfAgentProviderState,
}

/// Whether switching to a provider would be accepted.
///
/// A provider that cannot be used is still listed, unlike a named profile: the
/// operator can see the entry in their own configuration file, so omitting it
/// would read as a bug rather than as a refusal. Selection stays strict, and
/// the reason travels with the listing so the offending field is named before
/// the choice is made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfAgentProviderState {
    /// Switching to this provider resolves to `model` over `wire_api`. The wire
    /// is one of the two names resolution accepts; the model is bounded by
    /// [`validate_display_field`] before it reaches here.
    Selectable { model: String, wire_api: String },
    /// Switching would be refused. Both members are validation metadata and
    /// carry no configured value.
    Unavailable { path: String, message: String },
}

impl SelfAgentProviderChoice {
    pub fn is_selectable(&self) -> bool {
        matches!(self.state, SelfAgentProviderState::Selectable { .. })
    }
}

#[derive(Debug, Error)]
pub enum SelfAgentModelCatalogError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("configured model is unavailable")]
    ConfiguredModelUnavailable,
}

/// What the session chose, which is what the next turn resolves against.
///
/// The three arms are mutually exclusive by construction: a named profile
/// carries its own provider, so letting a provider switch survive underneath
/// one would silently contradict the profile the operator picked.
#[derive(Clone, Default, PartialEq, Eq)]
enum SelfAgentModelSelection {
    #[default]
    Configured,
    Profile(String),
    Provider(String),
}

impl SelfAgentModelSelection {
    /// The variant name, for a `Debug` output that must not carry configuration.
    fn kind(&self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Profile(_) => "profile",
            Self::Provider(_) => "provider",
        }
    }
}

#[derive(Clone, Default)]
pub struct SelfAgentModelSession {
    selection: SelfAgentModelSelection,
}

impl SelfAgentModelSession {
    pub fn catalog(
        &self,
        config: &UserConfig,
    ) -> Result<SelfAgentModelCatalog, SelfAgentModelCatalogError> {
        let mut catalog = load_self_agent_model_catalog(config)?;
        match &self.selection {
            SelfAgentModelSelection::Configured => {}
            SelfAgentModelSelection::Profile(name) => {
                let (_, choice) = select_self_agent_model_profile(config, name)?;
                catalog.active = SelfAgentActiveModel::Configured(choice);
            }
            SelfAgentModelSelection::Provider(provider) => {
                let (_, choice) = select_self_agent_model_provider(config, provider)?;
                catalog.active = SelfAgentActiveModel::Configured(choice);
                catalog.selected_provider = Some(provider.clone());
            }
        }
        Ok(catalog)
    }

    pub fn effective_config(
        &self,
        config: &UserConfig,
    ) -> Result<UserConfig, SelfAgentModelCatalogError> {
        match &self.selection {
            SelfAgentModelSelection::Configured => Ok(config.clone()),
            SelfAgentModelSelection::Profile(name) => {
                select_self_agent_model_profile(config, name).map(|(config, _)| config)
            }
            SelfAgentModelSelection::Provider(provider) => {
                select_self_agent_model_provider(config, provider).map(|(config, _)| config)
            }
        }
    }

    pub fn select_profile(
        &mut self,
        config: &UserConfig,
        name: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentModelCatalogError> {
        let (_, choice) = select_self_agent_model_profile(config, name)?;
        self.selection = match choice.profile.clone() {
            Some(name) => SelfAgentModelSelection::Profile(name),
            None => SelfAgentModelSelection::Configured,
        };
        Ok(choice)
    }

    /// Switch to another configured provider, keeping the active model.
    ///
    /// Strict like [`Self::select_profile`]: a provider the catalog only
    /// *reports* as unavailable must not become the session's choice.
    pub fn select_provider(
        &mut self,
        config: &UserConfig,
        provider: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentModelCatalogError> {
        let (_, choice) = select_self_agent_model_provider(config, provider)?;
        self.selection = SelfAgentModelSelection::Provider(choice.provider.clone());
        Ok(choice)
    }

    /// Return to the configured default.
    ///
    /// Selection is an action, not a projection, so it stays strict: an
    /// active model the catalog only *reports* as broken must not silently
    /// become the session's choice. The reason travels with the refusal so
    /// the caller learns which field to repair.
    pub fn select_configured(
        &mut self,
        config: &UserConfig,
    ) -> Result<SelfAgentModelChoice, SelfAgentModelCatalogError> {
        let choice = match load_self_agent_model_catalog(config)?.active {
            SelfAgentActiveModel::Configured(choice) => choice,
            SelfAgentActiveModel::Unresolved { path, message } => {
                return Err(ConfigError::Validation { path, message }.into());
            }
            SelfAgentActiveModel::NotConfigured => {
                return Err(SelfAgentModelCatalogError::ConfiguredModelUnavailable);
            }
        };
        self.selection = SelfAgentModelSelection::Configured;
        Ok(choice)
    }

    pub fn selected_profile(&self) -> Option<&str> {
        match &self.selection {
            SelfAgentModelSelection::Profile(name) => Some(name),
            SelfAgentModelSelection::Configured | SelfAgentModelSelection::Provider(_) => None,
        }
    }

    pub fn selected_provider(&self) -> Option<&str> {
        match &self.selection {
            SelfAgentModelSelection::Provider(provider) => Some(provider),
            SelfAgentModelSelection::Configured | SelfAgentModelSelection::Profile(_) => None,
        }
    }
}

impl fmt::Debug for SelfAgentModelSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentModelSession")
            .field("selection", &self.selection.kind())
            .finish()
    }
}

pub fn load_self_agent_model_catalog(
    config: &UserConfig,
) -> Result<SelfAgentModelCatalog, SelfAgentModelCatalogError> {
    let active = active_model(config)?;
    let mut profiles = Vec::with_capacity(config.model_profiles().len());
    for name in config.model_profiles().keys() {
        let (_, choice) = select_self_agent_model_profile(config, name)?;
        profiles.push(choice);
    }
    Ok(SelfAgentModelCatalog {
        active,
        profiles,
        providers: provider_choices(config)?,
        selected_provider: None,
    })
}

fn provider_choices(config: &UserConfig) -> Result<Vec<SelfAgentProviderChoice>, ConfigError> {
    let mut providers = Vec::with_capacity(config.model_providers().len());
    for provider in config.model_providers().keys() {
        // The key is what selection takes, so an unusable one is a hard failure
        // rather than a listed entry nobody could act on.
        validate_display_field(provider, "model_providers")?;
        let (provider_name, state) = match provider_state(config, provider) {
            Ok(selectable) => selectable,
            Err(error) => {
                let (path, message) = unresolved_metadata(error);
                (
                    provider.clone(),
                    SelfAgentProviderState::Unavailable { path, message },
                )
            }
        };
        providers.push(SelfAgentProviderChoice {
            provider: provider.clone(),
            provider_name,
            state,
        });
    }
    Ok(providers)
}

/// Resolve one provider entry into the name and state to offer for it.
///
/// Display validation happens here rather than at the call site so that
/// unprojectable metadata degrades into `Unavailable` like any other resolution
/// failure. That also keeps listing and selection in agreement: every field
/// checked here is one [`choice_from_resolved`] would refuse.
fn provider_state(
    config: &UserConfig,
    provider: &str,
) -> Result<(String, SelfAgentProviderState), ConfigError> {
    let resolved = config
        .with_model_provider(provider)
        .and_then(|selected| selected.resolve_model_profile())?;
    validate_display_field(&resolved.provider_name, "model_provider_name")?;
    validate_display_field(&resolved.model, "model")?;
    Ok((
        resolved.provider_name,
        SelfAgentProviderState::Selectable {
            model: resolved.model,
            wire_api: resolved.wire_api,
        },
    ))
}

fn active_model(config: &UserConfig) -> Result<SelfAgentActiveModel, SelfAgentModelCatalogError> {
    if config.model_provider.is_none() && config.model.is_none() {
        return Ok(SelfAgentActiveModel::NotConfigured);
    }
    let resolved = match config.resolve_model_profile() {
        Ok(resolved) => resolved,
        Err(error) => return Ok(unresolved_active_model(error)),
    };
    match choice_from_resolved(None, resolved) {
        Ok(choice) => Ok(SelfAgentActiveModel::Configured(choice)),
        Err(error) => Ok(unresolved_active_model(error)),
    }
}

fn unresolved_active_model(error: ConfigError) -> SelfAgentActiveModel {
    let (path, message) = unresolved_metadata(error);
    SelfAgentActiveModel::Unresolved { path, message }
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

/// Switch provider without changing the model, and report what that resolves
/// to. The returned choice has no `profile`: a provider switch leaves the
/// configured default in place and only redirects the transport.
pub fn select_self_agent_model_provider(
    config: &UserConfig,
    provider: &str,
) -> Result<(UserConfig, SelfAgentModelChoice), SelfAgentModelCatalogError> {
    let selected = config.with_model_provider(provider)?;
    let resolved = selected.resolve_model_profile()?;
    let choice = choice_from_resolved(None, resolved)?;
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
