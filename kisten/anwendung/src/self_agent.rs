use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig, USER_CONFIG};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, clear_provider_credential, load_self_agent_config_view,
    load_self_agent_permissions, load_self_agent_resume_catalog, load_self_agent_status,
    provider_draft, resolve_credential_target, store_provider_credential, wire_provider_reference,
    write_self_agent_provider, ConfigWiring, CredentialEntryError, CredentialTarget,
    CredentialUpdate, ProductionSelfAgentRuntime, ProviderDraft, ProviderEdit, ProviderEditError,
    RunEventSink, SelfAgentActiveModel, SelfAgentConfigView, SelfAgentModelCatalog,
    SelfAgentModelCatalogError, SelfAgentModelChoice, SelfAgentModelSession,
    SelfAgentPermissionSnapshot, SelfAgentResumeCatalog, SelfAgentResumeCatalogError,
    SelfAgentRunOutcome, SelfAgentRuntimeBuildError, SelfAgentRuntimeError, SelfAgentStatus,
    SelfAgentStatusError,
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
    config_loader: Option<ConfigLoader>,
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
        let mut host = Self::new(
            paths.workspace().to_path_buf(),
            paths.run_database(),
            paths.audit_log(),
        );
        host.config_loader = Some(ConfigLoader::for_user_path(paths.home().join(USER_CONFIG)));
        host
    }

    pub fn new(workspace: PathBuf, state_database: PathBuf, audit_log: PathBuf) -> Self {
        Self {
            workspace,
            state_database,
            audit_log,
            config_loader: None,
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

    pub fn pending_approval(
        &self,
        outcome: &SelfAgentRunOutcome,
    ) -> Result<
        Option<orchester_laufzeit::harness::service::SelfAgentPendingApproval>,
        SelfAgentHostError,
    > {
        self.runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?
            .pending_approval(outcome)
            .map_err(Into::into)
    }
    pub async fn resolve_approval(
        &mut self,
        pending: &orchester_laufzeit::harness::service::SelfAgentPendingApproval,
        approve: bool,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<Option<SelfAgentRunOutcome>, SelfAgentHostError> {
        self.ensure_runtime()?;
        self.runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?
            .resolve_approval(pending, approve, cancel, events)
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

    /// Run a turn while narrating the vendor-neutral event stream as well.
    ///
    /// `ModelEventSink` carries the model's text deltas; `RunEventSink` carries
    /// the unified [`orchester_protokoll::Event`] vocabulary (tool calls, usage,
    /// stop reasons). A frontend that does not own the run needs both, and this
    /// is the only entry point that hands it the narrated one.
    pub async fn submit_narrated(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
        run_events: Option<Arc<dyn RunEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime
            .run_streaming(prompt, cancel, events, run_events)
            .await
            .map_err(Into::into)
    }

    /// Resume a durable run while narrating the vendor-neutral event stream.
    pub async fn resume_narrated(
        &mut self,
        handle: &str,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
        run_events: Option<Arc<dyn RunEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime
            .resume_streaming(handle, cancel, events, run_events)
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
    /// Resolving a legacy host's loader can fail only when the home directory
    /// cannot be located; hosts built from [`OrchesterPaths`] are already bound
    /// to the explicit home selected by the frontend.
    pub fn config_view(&self) -> Result<SelfAgentConfigView, SelfAgentHostError> {
        let loader = self.config_loader()?;
        let credentials = KeyringCredentialStore::new();
        Ok(load_self_agent_config_view(
            &loader,
            &credentials,
            &self.workspace,
        ))
    }

    pub fn permissions(&self) -> Result<SelfAgentPermissionSnapshot, SelfAgentHostError> {
        let config = self.load_config()?;
        Ok(load_self_agent_permissions(&config))
    }

    /// Persist the user's explicit mode and rebuild execution before the next
    /// turn. A permission change never alters provider or reasoning choices.
    pub fn select_permission_mode(
        &mut self,
        mode: orchester_laufzeit::harness::config::PermissionMode,
    ) -> Result<SelfAgentPermissionSnapshot, SelfAgentHostError> {
        use orchester_laufzeit::harness::config::ConfigValue;
        let loader = self.config_loader()?;
        loader.load_effective(&self.workspace)?;
        loader.edit_user_config(&[(
            vec!["governance".into(), "permission_mode".into()],
            ConfigValue::String(mode.name().into()),
        )])?;
        self.runtime = None;
        self.permissions()
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
        let loader = self.config_loader()?;
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
        let loader = self.config_loader()?;
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

    pub async fn discover_provider_models(
        &self,
        provider: &str,
        cancel: CancellationToken,
    ) -> Result<
        Vec<orchester_laufzeit::harness::service::DiscoveredProviderModel>,
        SelfAgentHostError,
    > {
        let config = self.load_config()?;
        let credentials = KeyringCredentialStore::new();
        let transport = orchester_laufzeit::harness::provider::ReqwestHttpTransport::new()
            .map_err(|_| ConfigError::Validation {
                path: "provider".into(),
                message: "Model discovery transport unavailable".into(),
            })?;
        orchester_laufzeit::harness::service::discover_provider_models(
            &config,
            &credentials,
            provider,
            &transport,
            cancel,
        )
        .await
        .map_err(|_| {
            ConfigError::Validation {
                path: "provider".into(),
                message: "Could not retrieve provider model catalog".into(),
            }
            .into()
        })
    }

    pub fn provider_profiles(
        &self,
        provider: &str,
    ) -> Result<
        Vec<(
            String,
            orchester_laufzeit::harness::config::ModelProfileConfig,
        )>,
        SelfAgentHostError,
    > {
        Ok(self
            .load_config()?
            .model_profiles()
            .iter()
            .filter(|(_, p)| p.model_provider == provider)
            .map(|(n, p)| (n.clone(), p.clone()))
            .collect())
    }

    pub fn save_provider_catalog(
        &mut self,
        draft: &ProviderDraft,
        mut profiles: Vec<(
            String,
            orchester_laufzeit::harness::config::ModelProfileConfig,
        )>,
        secret: Option<SecretString>,
    ) -> Result<(), SelfAgentHostError> {
        use orchester_laufzeit::harness::config::{ConfigValue, ProviderConfig};
        let loader = self.config_loader()?;
        let config = loader.load_effective(&self.workspace)?;
        if profiles.is_empty() || profiles.len() > 128 {
            return Err(ConfigError::Validation {
                path: "models".into(),
                message: "Configure between 1 and 128 models".into(),
            }
            .into());
        }
        let mut entry = config
            .model_providers()
            .get(&draft.provider)
            .cloned()
            .unwrap_or(ProviderConfig {
                name: None,
                base_url: None,
                api_key: None,
                wire_api: None,
                requires_openai_auth: None,
            });
        entry.name = Some(draft.name.clone());
        entry.base_url = Some(draft.base_url.clone());
        entry.wire_api = Some(draft.wire_api.clone());
        config.resolve_provider_entry(
            &draft.provider,
            entry.clone(),
            Some(&profiles[0].1.model),
        )?;
        let mut candidate =
            config.with_provider_for_edit(&draft.provider, entry, &profiles[0].1.model)?;
        for (name, profile) in &mut profiles {
            *name = name.trim().to_owned();
            profile.model = profile.model.trim().to_owned();
            profile.display_name = profile
                .display_name
                .take()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());

            if let Some(old) = config
                .model_profiles()
                .get(name)
                .filter(|old| old.model_provider == draft.provider)
            {
                if profile.model_reasoning_effort.is_none() {
                    profile.model_reasoning_effort = old.model_reasoning_effort.clone();
                }
                if profile.plan_mode_reasoning_effort.is_none() {
                    profile.plan_mode_reasoning_effort = old.plan_mode_reasoning_effort.clone();
                }
                if profile.service_tier.is_none() {
                    profile.service_tier = old.service_tier.clone();
                }
            }
            if let Some(levels) = &profile.reasoning_levels {
                if !levels
                    .iter()
                    .any(|l| Some(l.level.as_str()) == profile.model_reasoning_effort.as_deref())
                {
                    profile.model_reasoning_effort = levels.first().map(|l| l.level.clone());
                }
                if !levels.iter().any(|l| {
                    Some(l.level.as_str()) == profile.plan_mode_reasoning_effort.as_deref()
                }) {
                    profile.plan_mode_reasoning_effort = profile.model_reasoning_effort.clone();
                }
            }
        }
        let mut names = std::collections::BTreeSet::new();
        for (name, profile) in &profiles {
            if profile.model_provider != draft.provider
                || !names.insert(name.clone())
                || config
                    .model_profiles()
                    .get(name)
                    .is_some_and(|p| p.model_provider != draft.provider)
            {
                return Err(ConfigError::Validation {
                    path: "models".into(),
                    message: "Model profile ownership or identifier is invalid".into(),
                }
                .into());
            }
            candidate = candidate.with_model_profile_entry(name, Some(profile.clone()))?;
        }
        let mut edits = Vec::new();
        if !config.model_providers().contains_key(&draft.provider) && secret.is_none() {
            return Err(ConfigError::Validation {
                path: "api_key".into(),
                message: "New providers require an API key".into(),
            }
            .into());
        }
        if config.model_provider.is_none()
            || (config.model_provider.as_deref() == Some(&draft.provider)
                && !profiles
                    .iter()
                    .any(|(_, p)| Some(p.model.as_str()) == config.model.as_deref()))
        {
            edits.push((
                vec!["model_provider".into()],
                ConfigValue::String(draft.provider.clone()),
            ));
            edits.push((
                vec!["model".into()],
                ConfigValue::String(profiles[0].1.model.clone()),
            ));
        }

        for (field, value) in [
            ("name", &draft.name),
            ("base_url", &draft.base_url),
            ("wire_api", &draft.wire_api),
        ] {
            edits.push((
                vec![
                    "model_providers".into(),
                    draft.provider.clone(),
                    field.into(),
                ],
                ConfigValue::String(value.clone()),
            ));
        }
        for (name, profile) in config.model_profiles() {
            if profile.model_provider == draft.provider && !names.contains(name) {
                edits.push((
                    vec!["model_profiles".into(), name.clone()],
                    ConfigValue::Remove,
                ));
            }
        }
        for (name, profile) in profiles {
            edits.push((
                vec!["model_profiles".into(), name],
                ConfigValue::from_serializable(&profile).expect("model profile serializes"),
            ));
        }
        if let Some(secret) = secret {
            with_staged_credential(&KeyringCredentialStore::new(), secret, |reference| {
                edits.push((
                    vec![
                        "model_providers".into(),
                        draft.provider.clone(),
                        "api_key".into(),
                    ],
                    ConfigValue::String(reference.into()),
                ));
                loader.edit_user_config(&edits)
            })?;
        } else {
            loader.edit_user_config(&edits)?;
        }
        self.runtime = None;
        self.model_session = Default::default();
        Ok(())
    }

    pub fn delete_provider(&mut self, provider: &str) -> Result<(), SelfAgentHostError> {
        use orchester_laufzeit::harness::config::ConfigValue;
        let loader = self.config_loader()?;
        let config = loader.load_effective(&self.workspace)?;
        let mut edits = vec![(
            vec!["model_providers".into(), provider.into()],
            ConfigValue::Remove,
        )];
        for (name, profile) in config.model_profiles() {
            if profile.model_provider == provider {
                edits.push((
                    vec!["model_profiles".into(), name.clone()],
                    ConfigValue::Remove,
                ));
            }
        }
        if config.model_provider.as_deref() == Some(provider) {
            edits.push((vec!["model_provider".into()], ConfigValue::Remove));
            edits.push((vec!["model".into()], ConfigValue::Remove));
        }
        loader.edit_user_config(&edits)?;
        self.runtime = None;
        self.model_session = Default::default();
        Ok(())
    }

    pub fn edit_model_profile(
        &mut self,
        provider: &str,
        name: &str,
        model: Option<&str>,
    ) -> Result<(), SelfAgentHostError> {
        use orchester_laufzeit::harness::config::{ConfigError, ConfigValue, ModelProfileConfig};
        let loader = self.config_loader()?;
        let config = loader.load_effective(&self.workspace)?;
        if config
            .model_profiles()
            .get(name)
            .is_some_and(|p| p.model_provider != provider)
        {
            return Err(ConfigError::Validation {
                path: "model_profiles".into(),
                message: "This profile belongs to another provider".into(),
            }
            .into());
        }
        let profile = model.map(|model| {
            let mut profile =
                config
                    .model_profiles()
                    .get(name)
                    .cloned()
                    .unwrap_or(ModelProfileConfig {
                        model_provider: provider.to_owned(),
                        model: model.to_owned(),
                        display_name: None,
                        context_window: None,
                        max_output_tokens: None,
                        input_types: None,
                        reasoning_levels: None,
                        model_reasoning_effort: None,
                        plan_mode_reasoning_effort: None,
                        service_tier: None,
                    });
            profile.model = model.to_owned();
            profile
        });
        let candidate = config.with_model_profile_entry(name, profile)?;
        let members = candidate
            .model_profiles()
            .iter()
            .map(|(name, p)| {
                (
                    name.clone(),
                    ConfigValue::from_serializable(p).expect("model profile serializes"),
                )
            })
            .collect();
        loader
            .edit_user_config(&[(vec!["model_profiles".into()], ConfigValue::Object(members))])?;
        self.runtime = None;
        if model.is_none() && self.model_session.selected_profile() == Some(name) {
            self.model_session = Default::default();
        }
        Ok(())
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
        self.config_loader()?
            .load_effective(&self.workspace)
            .map_err(Into::into)
    }

    fn config_loader(&self) -> Result<ConfigLoader, SelfAgentHostError> {
        self.config_loader
            .clone()
            .map(Ok)
            .unwrap_or_else(ConfigLoader::new)
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
        if let Some(metadata) = &choice.metadata {
            if let Some(levels) = &metadata.reasoning_levels {
                if !choice
                    .reasoning_effort
                    .as_ref()
                    .is_some_and(|e| levels.iter().any(|l| &l.level == e))
                {
                    choice.reasoning_effort = metadata
                        .model_reasoning_effort
                        .clone()
                        .filter(|e| levels.iter().any(|l| &l.level == e));
                }
            }
        }
        choice
    }
}

fn with_staged_credential<
    S: orchester_laufzeit::harness::credentials::CredentialStore + ?Sized,
    T,
>(
    store: &S,
    secret: SecretString,
    apply: impl FnOnce(&str) -> Result<T, ConfigError>,
) -> Result<T, SelfAgentHostError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| SelfAgentHostError::Initialization)?
        .as_nanos();
    let slot = format!(
        "provider-edit-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    if store.present(&slot).map_err(CredentialEntryError::from)? {
        return Err(SelfAgentHostError::Initialization);
    }
    let update = store_provider_credential(store, &slot, secret)?;
    match apply(&update.reference) {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ = store.clear(&slot);
            Err(error.into())
        }
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
    use orchester_laufzeit::harness::service::SelfAgentActiveModel;
    use std::fs;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvironmentRestore {
        previous: Option<std::ffi::OsString>,
    }

    impl Drop for EnvironmentRestore {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var("ORCHESTER_HOME", previous);
            } else {
                std::env::remove_var("ORCHESTER_HOME");
            }
        }
    }

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
    fn staged_credential_failure_keeps_existing_key_and_removes_unused_stage() {
        use orchester_laufzeit::harness::credentials::{CredentialStore, InMemoryCredentialStore};
        use secrecy::ExposeSecret;
        let store = InMemoryCredentialStore::with("relay", "working-key");
        let mut slot = String::new();
        let result: Result<(), SelfAgentHostError> =
            with_staged_credential(&store, SecretString::from("replacement-key"), |reference| {
                slot = reference
                    .strip_prefix("${secret:")
                    .unwrap()
                    .strip_suffix('}')
                    .unwrap()
                    .to_owned();
                assert_eq!(
                    store.get(&slot).unwrap().unwrap().expose_secret(),
                    "replacement-key"
                );
                Err(ConfigError::Validation {
                    path: "test".into(),
                    message: "simulated write failure".into(),
                })
            });
        assert!(result.is_err());
        assert!(!store.present(&slot).unwrap());
        assert_eq!(
            store.get("relay").unwrap().unwrap().expose_secret(),
            "working-key"
        );
    }

    #[test]
    fn provider_catalog_edit_preserves_credentials_and_other_provider_then_deletes_owned_models() {
        use orchester_laufzeit::harness::config::{ConfigValue, ModelProfileConfig};
        let root = std::env::temp_dir().join(format!(
            "orchester-rich-provider-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let path = home.join(USER_CONFIG);
        let loader = ConfigLoader::for_user_path(&path);
        for provider in ["alpha", "beta"] {
            loader
                .edit_user_config(&[(
                    vec!["model_providers".into(), provider.into()],
                    ConfigValue::Object(vec![
                        (
                            "base_url".into(),
                            ConfigValue::String("https://example.com/v1".into()),
                        ),
                        (
                            "api_key".into(),
                            ConfigValue::String("existing-private-test-key".into()),
                        ),
                        ("wire_api".into(), ConfigValue::String("responses".into())),
                    ]),
                )])
                .unwrap();
        }
        loader
            .edit_user_config(&[
                (
                    vec!["model_provider".into()],
                    ConfigValue::String("alpha".into()),
                ),
                (vec!["model".into()], ConfigValue::String("old".into())),
            ])
            .unwrap();
        let paths = OrchesterPaths::new(&home, &workspace);
        let mut host = SelfAgentHost::for_paths(&paths);
        let profile = ModelProfileConfig {
            model_provider: "alpha".into(),
            model: "new-model".into(),
            display_name: Some("Friendly".into()),
            context_window: Some(1000000),
            max_output_tokens: Some(256000),
            input_types: Some(vec!["text".into(), "image".into()]),
            reasoning_levels: Some(vec![]),
            model_reasoning_effort: None,
            plan_mode_reasoning_effort: None,
            service_tier: None,
        };
        let draft = ProviderDraft {
            provider: "alpha".into(),
            name: "Alpha edited".into(),
            base_url: "https://example.com/v1".into(),
            wire_api: "responses".into(),
            model: "new-model".into(),
            activate: false,
        };
        host.save_provider_catalog(&draft, vec![("alpha.new".into(), profile.clone())], None)
            .unwrap();
        let reopened = SelfAgentHost::for_paths(&paths);
        assert_eq!(reopened.provider_profiles("alpha").unwrap()[0].1, profile);
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("existing-private-test-key").count(), 2);
        assert!(!text.contains("__orchester"));
        host.delete_provider("alpha").unwrap();
        let loaded = loader.load_effective(&workspace).unwrap();
        assert!(loaded.model_providers().contains_key("beta"));
        assert!(!loaded.model_providers().contains_key("alpha"));
        assert!(loaded.model_profiles().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn host_exposes_eventful_resume_entrypoint() {
        let _: ResumeEntrypoint = invoke_resume;
    }

    #[test]
    fn permission_selection_persists_without_touching_models_or_other_settings() {
        use orchester_laufzeit::harness::config::{ConfigValue, PermissionMode};
        use orchester_protokoll::PolicyDecision;

        let root = std::env::temp_dir().join(format!(
            "orchester-host-permissions-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let path = home.join(USER_CONFIG);
        ConfigLoader::for_user_path(&path)
            .edit_user_config(&[
                (vec!["model".into()], ConfigValue::String("my-model".into())),
                (
                    vec!["governance".into(), "approval_reviewer".into()],
                    ConfigValue::String("local-reviewer".into()),
                ),
            ])
            .unwrap();
        let paths = OrchesterPaths::new(&home, &workspace);
        let mut host = SelfAgentHost::for_paths(&paths);
        assert_eq!(host.permissions().unwrap().mode, None);
        let changed = host
            .select_permission_mode(PermissionMode::FullAccess)
            .unwrap();
        assert_eq!(changed.mode, Some(PermissionMode::FullAccess));

        let mut reopened = SelfAgentHost::for_paths(&paths);
        assert_eq!(
            reopened.permissions().unwrap().mode,
            Some(PermissionMode::FullAccess)
        );
        let readonly = reopened
            .select_permission_mode(PermissionMode::ReadOnly)
            .unwrap();
        assert_eq!(
            readonly
                .rules
                .iter()
                .find(|rule| rule.id == "workspace.write")
                .unwrap()
                .effective,
            PolicyDecision::Ask
        );
        assert_eq!(
            readonly
                .rules
                .iter()
                .find(|rule| rule.id == "workspace.read")
                .unwrap()
                .effective,
            PolicyDecision::Allow
        );
        let loaded = ConfigLoader::for_user_path(&path)
            .load_user_file(&path)
            .unwrap();
        assert_eq!(loaded.model.as_deref(), Some("my-model"));
        assert_eq!(loaded.governance.approval_reviewer, "local-reviewer");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn for_paths_binds_model_configuration_to_the_explicit_home() {
        use orchester_laufzeit::harness::config::ConfigValue;

        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchester-host-paths-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let explicit_home = root.join("explicit-home");
        let unrelated_home = root.join("unrelated-home");
        let workspace = root.join("workspace");
        fs::create_dir_all(&explicit_home).expect("explicit home");
        fs::create_dir_all(&unrelated_home).expect("unrelated home");
        fs::create_dir_all(&workspace).expect("workspace");
        let config = explicit_home.join("orchester.jsonc");
        // Seed the fixture through the application writer so Windows ACLs and
        // Unix file permissions satisfy the same loader as production.
        ConfigLoader::for_user_path(&config)
            .edit_user_config(&[(
                vec!["model".to_owned()],
                ConfigValue::String("gpt-explicit".to_owned()),
            )])
            .expect("private model configuration");
        fs::write(
            &config,
            r#"{
                "model_provider": "OpenAI",
                "model": "gpt-explicit",
                "model_providers": {
                    "OpenAI": { "base_url": "https://example.test/v1" }
                }
            }"#,
        )
        .expect("model config");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The loader requires the config's directory to be user-only as
            // well as the file, and a fresh temp directory is 0755 under the
            // runner's umask: without this the test fails on Linux while
            // passing on Windows, where the gate reads ACLs instead.
            fs::set_permissions(&explicit_home, fs::Permissions::from_mode(0o700))
                .expect("private home permissions");
            fs::set_permissions(&config, fs::Permissions::from_mode(0o600))
                .expect("private config permissions");
        }
        let previous = std::env::var_os("ORCHESTER_HOME");
        std::env::set_var("ORCHESTER_HOME", &unrelated_home);
        let _restore = EnvironmentRestore { previous };

        let paths = OrchesterPaths::new(&explicit_home, &workspace);
        let mut host = SelfAgentHost::for_paths(&paths);
        let catalog = host.model_catalog().expect("model catalog");

        assert!(matches!(
            catalog.active,
            SelfAgentActiveModel::Configured(ref choice) if choice.model == "gpt-explicit"
        ));
        host.edit_model_profile("OpenAI", "first", Some("first-model"))
            .unwrap();
        host.edit_model_profile("OpenAI", "second", Some("second-model"))
            .unwrap();
        let reloaded = SelfAgentHost::for_paths(&paths).model_catalog().unwrap();
        assert_eq!(reloaded.profiles.len(), 2);
        assert!(host
            .edit_model_profile("Other", "first", Some("stolen-model"))
            .is_err());
        host.edit_model_profile("OpenAI", "first", Some("edited-model"))
            .unwrap();
        assert_eq!(
            host.model_catalog().unwrap().profiles[0].model,
            "edited-model"
        );
        host.edit_model_profile("OpenAI", "first", None).unwrap();
        let remaining = SelfAgentHost::for_paths(&paths).model_catalog().unwrap();
        assert_eq!(remaining.profiles.len(), 1);
        assert_eq!(remaining.profiles[0].model, "second-model");
        let _ = fs::remove_dir_all(root);
    }
}
