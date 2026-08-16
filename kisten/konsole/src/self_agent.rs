use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, clear_provider_credential, load_self_agent_config_view,
    load_self_agent_permissions, load_self_agent_resume_catalog, load_self_agent_status,
    resolve_credential_target, store_provider_credential, wire_provider_reference, ConfigWiring,
    CredentialEntryError, CredentialTarget, CredentialUpdate, ProductionSelfAgentRuntime,
    SelfAgentActiveModel, SelfAgentConfigView, SelfAgentModelCatalog, SelfAgentModelCatalogError,
    SelfAgentModelChoice, SelfAgentModelSession, SelfAgentPermissionSnapshot,
    SelfAgentResumeCatalog, SelfAgentResumeCatalogError, SelfAgentRunOutcome,
    SelfAgentRuntimeBuildError, SelfAgentRuntimeError, SelfAgentStatus, SelfAgentStatusError,
};
use orchester_laufzeit::harness::StreamingRedactor;
use orchester_modell::ModelEventSink;
use secrecy::SecretString;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

mod config;
mod credentials;
mod models;
mod permissions;
mod render;
mod resume;
mod status;

pub use config::render_config;
pub use credentials::{
    render_credential_cleared, render_credential_stored, render_credential_target,
};
pub use models::{render_model_selection, render_models};
pub use permissions::render_permissions;
pub use render::{render_outcome, render_outcome_transcript};
pub use resume::render_resume;
pub use status::render_status;

#[derive(Debug, Error)]
pub enum SelfAgentHostError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Build(#[from] SelfAgentRuntimeBuildError),
    #[error(transparent)]
    Run(#[from] SelfAgentRuntimeError),
    #[error(transparent)]
    Status(#[from] SelfAgentStatusError),
    #[error(transparent)]
    Resume(#[from] SelfAgentResumeCatalogError),
    #[error(transparent)]
    Models(#[from] SelfAgentModelCatalogError),
    #[error(transparent)]
    Credential(#[from] CredentialEntryError),
    #[error("self-agent runtime initialization failed")]
    Initialization,
}

pub struct SelfAgentHost {
    workspace: PathBuf,
    state_database: PathBuf,
    audit_log: PathBuf,
    model_session: SelfAgentModelSession,
    /// A picker choice is session-scoped: it must affect future turns without
    /// rewriting the user's protected configuration file. `Some(None)` is an
    /// explicit request for the provider default, distinct from no override.
    reasoning_effort_override: Option<Option<String>>,
    runtime: Option<ProductionSelfAgentRuntime>,
}

impl SelfAgentHost {
    pub fn new(workspace: PathBuf, state_database: PathBuf, audit_log: PathBuf) -> Self {
        Self {
            workspace,
            state_database,
            audit_log,
            model_session: SelfAgentModelSession::default(),
            reasoning_effort_override: None,
            runtime: None,
        }
    }

    pub async fn submit(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.submit_with_events(prompt, cancel, None).await
    }

    pub async fn submit_with_events(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime
            .run_with_events(prompt, cancel, events)
            .await
            .map_err(Into::into)
    }

    pub fn model_catalog(&self) -> Result<SelfAgentModelCatalog, SelfAgentHostError> {
        let config = self.load_config()?;
        let mut catalog = self.model_session.catalog(&config)?;
        if let Some(effort) = self.reasoning_effort_override.as_ref() {
            if let SelfAgentActiveModel::Configured(choice) = &mut catalog.active {
                choice.reasoning_effort = effort.clone();
            }
        }
        Ok(catalog)
    }

    /// Resolve the file-backed default without changing the session choice.
    pub fn configured_model_choice(&self) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let mut default_session = SelfAgentModelSession::default();
        default_session
            .select_configured(&config)
            .map_err(Into::into)
    }

    pub fn streaming_redactor(&self) -> Result<StreamingRedactor, SelfAgentHostError> {
        let config = self.selected_config()?;
        let provider = config.resolve_model_profile()?.provider;
        let credentials = KeyringCredentialStore::new();
        let secrets = config.resolve_configured_secrets_for_provider(&provider, &credentials)?;
        Ok(secrets.into_streaming_redactor())
    }

    pub fn model_label(&self) -> Result<String, SelfAgentHostError> {
        let catalog = self.model_catalog()?;
        Ok(match catalog.active {
            SelfAgentActiveModel::Configured(active) => match active.reasoning_effort {
                Some(reasoning) => format!("{} {reasoning}", active.model),
                None => active.model,
            },
            // The panel has room for one line, so it names the state rather
            // than the reason; /model reports which field is at fault.
            SelfAgentActiveModel::Unresolved { .. } => "model unresolved".into(),
            SelfAgentActiveModel::NotConfigured => "model not configured".into(),
        })
    }

    pub fn select_model_profile(
        &mut self,
        name: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let choice = self.model_session.select_profile(&config, name)?;
        self.reasoning_effort_override = None;
        self.runtime = None;
        Ok(choice)
    }

    pub fn select_configured_model(&mut self) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let choice = self.model_session.select_configured(&config)?;
        self.reasoning_effort_override = None;
        self.runtime = None;
        Ok(choice)
    }

    /// Select a named profile and apply a session-only reasoning effort.
    /// `None` means the provider default and is intentionally different from
    /// leaving the current override untouched.
    pub fn select_model_profile_with_effort(
        &mut self,
        name: &str,
        effort: Option<&str>,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let choice = self.select_model_profile(name)?;
        self.reasoning_effort_override = Some(normalize_reasoning_effort(effort));
        Ok(self.choice_with_effort(choice))
    }

    /// Select the configured model and apply a session-only reasoning effort.
    pub fn select_configured_model_with_effort(
        &mut self,
        effort: Option<&str>,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let choice = self.select_configured_model()?;
        self.reasoning_effort_override = Some(normalize_reasoning_effort(effort));
        Ok(self.choice_with_effort(choice))
    }

    pub fn status(&self) -> Result<SelfAgentStatus, SelfAgentHostError> {
        let config = self.selected_config()?;
        let credentials = KeyringCredentialStore::new();
        load_self_agent_status(
            &config,
            &credentials,
            &self.workspace,
            &self.state_database,
            "local-user",
        )
        .map_err(Into::into)
    }

    /// Project configuration for display.
    ///
    /// Deliberately does not go through [`Self::load_config`]: every other
    /// command `?`-propagates a load failure and leaves the human with one bare
    /// sentence, and this command exists to answer exactly that case. Only
    /// `ConfigLoader::new` can fail here, and only when the home directory
    /// cannot be located at all — the one state with no path worth reporting.
    pub fn config_view(&self) -> Result<SelfAgentConfigView, SelfAgentHostError> {
        let loader = ConfigLoader::new()?;
        let credentials = KeyringCredentialStore::new();
        Ok(load_self_agent_config_view(
            &loader,
            &credentials,
            &self.workspace,
        ))
    }

    pub fn permissions(&self) -> Result<SelfAgentPermissionSnapshot, SelfAgentHostError> {
        let config = self.selected_config()?;
        Ok(load_self_agent_permissions(&config))
    }

    pub fn resume_catalog(&self) -> Result<SelfAgentResumeCatalog, SelfAgentHostError> {
        let config = self.selected_config()?;
        let credentials = KeyringCredentialStore::new();
        load_self_agent_resume_catalog(
            &config,
            &credentials,
            &self.workspace,
            &self.state_database,
            "local-user",
        )
        .map_err(Into::into)
    }

    /// Describe the provider `/login` or `/logout` would act on, so the prompt
    /// can name it before a key is pasted.
    pub fn credential_target(
        &self,
        provider: Option<&str>,
    ) -> Result<CredentialTarget, SelfAgentHostError> {
        let config = self.load_config()?;
        let credentials = KeyringCredentialStore::new();
        resolve_credential_target(&config, &credentials, provider).map_err(Into::into)
    }

    /// Store a key and make it reachable from configuration.  No provider
    /// request is made, so the result is stored but unverified.
    ///
    /// The resolved configuration path is returned alongside the outcome:
    /// `ORCHESTER_HOME` moves that file, so only the loader knows where the
    /// reference was actually written.
    pub fn store_credential(
        &mut self,
        target: &CredentialTarget,
        secret: SecretString,
    ) -> Result<(CredentialUpdate, ConfigWiring, PathBuf), SelfAgentHostError> {
        let credentials = KeyringCredentialStore::new();
        let update = store_provider_credential(&credentials, &target.provider, secret)?;
        let loader = ConfigLoader::new()?;
        let config_path = loader.user_path().to_path_buf();
        let wiring = wire_provider_reference(&config_path, target)?;
        // The next turn must resolve against the key that was just stored.
        self.runtime = None;
        Ok((update, wiring, config_path))
    }

    /// Forget a stored key.  Reports whether one was actually present.
    pub fn clear_credential(&mut self, provider: &str) -> Result<bool, SelfAgentHostError> {
        let credentials = KeyringCredentialStore::new();
        let removed = clear_provider_credential(&credentials, provider)?;
        self.runtime = None;
        Ok(removed)
    }

    fn ensure_runtime(&mut self) -> Result<(), SelfAgentHostError> {
        if self.runtime.is_some() {
            return Ok(());
        }
        let config = self.selected_config()?;
        let credentials = KeyringCredentialStore::new();
        self.runtime = Some(build_self_agent_runtime(
            &config,
            &credentials,
            &self.workspace,
            &self.state_database,
            &self.audit_log,
            "local-user",
        )?);
        Ok(())
    }

    fn load_config(&self) -> Result<UserConfig, SelfAgentHostError> {
        ConfigLoader::new()?
            .load_effective(&self.workspace)
            .map_err(Into::into)
    }

    fn selected_config(&self) -> Result<UserConfig, SelfAgentHostError> {
        let mut config = self.load_config()?;
        config = self
            .model_session
            .effective_config(&config)
            .map_err(SelfAgentHostError::from)?;
        if let Some(effort) = self.reasoning_effort_override.as_ref() {
            config.model_reasoning_effort = effort.clone();
        }
        Ok(config)
    }

    fn choice_with_effort(&self, mut choice: SelfAgentModelChoice) -> SelfAgentModelChoice {
        if let Some(effort) = self.reasoning_effort_override.as_ref() {
            choice.reasoning_effort = effort.clone();
        }
        choice
    }
}

fn normalize_reasoning_effort(effort: Option<&str>) -> Option<String> {
    effort
        .map(str::trim)
        .filter(|effort| !effort.is_empty() && !effort.eq_ignore_ascii_case("default"))
        .map(str::to_ascii_lowercase)
}

impl fmt::Debug for SelfAgentHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentHost")
            .field("workspace", &"[REDACTED]")
            .field("state_database", &"[REDACTED]")
            .field("audit_log", &"[REDACTED]")
            .field(
                "named_model_selected",
                &self.model_session.selected_profile().is_some(),
            )
            .field("initialized", &self.runtime.is_some())
            .finish()
    }
}
