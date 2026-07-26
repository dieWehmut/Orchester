use std::fmt;
use std::path::PathBuf;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, load_self_agent_status, ProductionSelfAgentRuntime,
    SelfAgentModelCatalog, SelfAgentModelCatalogError, SelfAgentModelSession, SelfAgentRunOutcome,
    SelfAgentRuntimeBuildError, SelfAgentRuntimeError, SelfAgentStatus, SelfAgentStatusError,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

mod models;
mod render;
mod status;

pub use models::render_models;
pub use render::render_outcome;
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
        let config = ConfigLoader::new()?.load_effective(&self.workspace)?;
        self.model_session.catalog(&config).map_err(Into::into)
    }

    pub fn status(&self) -> Result<SelfAgentStatus, SelfAgentHostError> {
        let config = ConfigLoader::new()?.load_effective(&self.workspace)?;
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

    fn ensure_runtime(&mut self) -> Result<(), SelfAgentHostError> {
        if self.runtime.is_some() {
            return Ok(());
        }
        let config = ConfigLoader::new()?.load_effective(&self.workspace)?;
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
}

impl fmt::Debug for SelfAgentHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentHost")
            .field("workspace", &"[REDACTED]")
            .field("state_database", &"[REDACTED]")
            .field("audit_log", &"[REDACTED]")
            .field("initialized", &self.runtime.is_some())
            .finish()
    }
}
