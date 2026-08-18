use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, clear_provider_credential, load_self_agent_config_view,
    load_self_agent_permissions, load_self_agent_resume_catalog, load_self_agent_status,
    provider_draft, resolve_credential_target, store_provider_credential, wire_provider_reference,
    write_self_agent_provider, ConfigWiring, CredentialEntryError, CredentialTarget,
    CredentialUpdate, ProductionSelfAgentRuntime, ProviderDraft, ProviderEdit, ProviderEditError,
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

use crate::OrchesterPaths;

/// The actor a locally started run is attributed to in the audit log.
///
/// Every frontend on this machine speaks for the same person, so they must all
/// name the same actor: a trail that said `web-user` in one window and
/// `local-user` in another would read as two operators.
const OWNER_ACTOR_ID: &str = "local-user";

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
    #[error(transparent)]
    ProviderEdit(#[from] ProviderEditError),
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
    /// Host the self-agent for one project, using the standard file layout.
    ///
    /// Preferred over [`Self::new`]: it is the layout every frontend shares, so
    /// a WebUI and the terminal open the same run database rather than each
    /// joining its own file names onto the home directory.
    pub fn for_paths(paths: &OrchesterPaths) -> Self {
        Self::new(
            paths.workspace().to_path_buf(),
            paths.run_database(),
            paths.audit_log(),
        )
    }

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

    pub async fn resume_with_events(
        &mut self,
        handle: &str,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime
            .resume_with_events(handle, cancel, events)
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

    /// Resolve what switching to `provider` would produce, without changing the
    /// session choice.
    ///
    /// A throwaway session keeps this a projection: the effort picker needs the
    /// resolved model before the user has committed to anything.
    pub fn provider_model_choice(
        &self,
        provider: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let mut probe = SelfAgentModelSession::default();
        probe.select_provider(&config, provider).map_err(Into::into)
    }

    pub fn streaming_redactor(&mut self) -> Result<StreamingRedactor, SelfAgentHostError> {
        self.ensure_runtime()?;
        self.runtime
            .as_ref()
            .map(ProductionSelfAgentRuntime::streaming_redactor)
            .ok_or(SelfAgentHostError::Initialization)
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

    /// Route future turns through another configured provider, keeping the
    /// active model. Replaces any named profile the session had selected.
    pub fn select_model_provider(
        &mut self,
        provider: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let choice = self.model_session.select_provider(&config, provider)?;
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

    /// Switch provider and apply a session-only reasoning effort.
    pub fn select_model_provider_with_effort(
        &mut self,
        provider: &str,
        effort: Option<&str>,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let choice = self.select_model_provider(provider)?;
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
            OWNER_ACTOR_ID,
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
            OWNER_ACTOR_ID,
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

    /// Project the provider entry a form should open on.  `None` means the
    /// provider is not configured yet, so the form is adding rather than
    /// editing.
    pub fn provider_draft(
        &self,
        provider: &str,
    ) -> Result<Option<ProviderDraft>, SelfAgentHostError> {
        let config = self.load_config()?;
        Ok(provider_draft(&config, provider))
    }

    /// Write one `model_providers` entry into the user's configuration file,
    /// storing `secret` under the reference the entry names.
    ///
    /// The configuration on disk is the source every later command reads, so the
    /// cached runtime is dropped unconditionally.  An activated entry also
    /// replaces this session's provider choice: a session override would
    /// otherwise mask the default that was just written and make the edit look
    /// as though it had been ignored.
    pub fn write_provider(
        &mut self,
        draft: &ProviderDraft,
        secret: Option<SecretString>,
    ) -> Result<ProviderEdit, SelfAgentHostError> {
        let loader = ConfigLoader::new()?;
        let config = loader.load_effective(&self.workspace)?;
        let credentials = KeyringCredentialStore::new();
        let edit = write_self_agent_provider(&loader, &config, &credentials, draft, secret)?;
        self.runtime = None;
        if edit.activated {
            let written = loader.load_effective(&self.workspace)?;
            self.model_session
                .select_provider(&written, &edit.provider)?;
            self.reasoning_effort_override = None;
        }
        Ok(edit)
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
            OWNER_ACTOR_ID,
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
            .field(
                "provider_selected",
                &self.model_session.selected_provider().is_some(),
            )
            .field("initialized", &self.runtime.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;

    type ResumeFuture<'a> =
        Pin<Box<dyn Future<Output = Result<SelfAgentRunOutcome, SelfAgentHostError>> + 'a>>;
    type ResumeEntrypoint = for<'a> fn(
        &'a mut SelfAgentHost,
        &'a str,
        CancellationToken,
        Option<Arc<dyn ModelEventSink>>,
    ) -> ResumeFuture<'a>;

    fn invoke_resume<'a>(
        host: &'a mut SelfAgentHost,
        handle: &'a str,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> ResumeFuture<'a> {
        Box::pin(host.resume_with_events(handle, cancel, events))
    }

    #[test]
    fn host_exposes_eventful_resume_entrypoint() {
        let _: ResumeEntrypoint = invoke_resume;
    }
}
