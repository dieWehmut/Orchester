use std::fmt;
use std::path::PathBuf;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, load_self_agent_permissions, load_self_agent_resume_catalog,
    load_self_agent_status, ProductionSelfAgentRuntime, SelfAgentModelCatalog,
    SelfAgentModelCatalogError, SelfAgentModelChoice, SelfAgentModelSession,
    SelfAgentPermissionSnapshot, SelfAgentResumeCatalog, SelfAgentResumeCatalogError,
    SelfAgentRunOutcome, SelfAgentRuntimeBuildError, SelfAgentRuntimeError, SelfAgentStatus,
    SelfAgentStatusError,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

mod models;
mod permissions;
mod render;
mod resume;
mod status;

pub use models::{render_model_selection, render_models};
pub use permissions::render_permissions;
pub use render::render_outcome;
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
    #[error("self-agent runtime initialization failed")]
    Initialization,
}

pub struct SelfAgentHost {
    workspace: PathBuf,
    state_database: PathBuf,
    audit_log: PathBuf,
    model_session: SelfAgentModelSession,
    runtime: Option<ProductionSelfAgentRuntime>,
}

impl SelfAgentHost {
    pub fn new(workspace: PathBuf, state_database: PathBuf, audit_log: PathBuf) -> Self {
        Self {
            workspace,
            state_database,
            audit_log,
            model_session: SelfAgentModelSession::default(),
            runtime: None,
        }
    }

    pub async fn submit(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<SelfAgentRunOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime.run(prompt, cancel).await.map_err(Into::into)
    }

    pub fn model_catalog(&self) -> Result<SelfAgentModelCatalog, SelfAgentHostError> {
        let config = self.load_config()?;
        self.model_session.catalog(&config).map_err(Into::into)
    }

    pub fn model_label(&self) -> Result<String, SelfAgentHostError> {
        let catalog = self.model_catalog()?;
        Ok(match catalog.configured {
            Some(active) => match active.reasoning_effort {
                Some(reasoning) => format!("{} {reasoning}", active.model),
                None => active.model,
            },
            None => "model not configured".into(),
        })
    }

    pub fn select_model_profile(
        &mut self,
        name: &str,
    ) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let choice = self.model_session.select_profile(&config, name)?;
        self.runtime = None;
        Ok(choice)
    }

    pub fn select_configured_model(&mut self) -> Result<SelfAgentModelChoice, SelfAgentHostError> {
        let config = self.load_config()?;
        let choice = self.model_session.select_configured(&config)?;
        self.runtime = None;
        Ok(choice)
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
        let config = self.load_config()?;
        self.model_session
            .effective_config(&config)
            .map_err(Into::into)
    }
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
