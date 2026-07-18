use std::fmt;
use std::path::PathBuf;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_runtime, ProductionSelfAgentRuntime, SelfAgentOutcome,
    SelfAgentRuntimeBuildError, SelfAgentRuntimeError,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

mod render;

pub use render::render_outcome;

#[derive(Debug, Error)]
pub enum SelfAgentHostError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Build(#[from] SelfAgentRuntimeBuildError),
    #[error(transparent)]
    Run(#[from] SelfAgentRuntimeError),
    #[error("self-agent runtime initialization failed")]
    Initialization,
}

pub struct SelfAgentHost {
    workspace: PathBuf,
    state_database: PathBuf,
    audit_log: PathBuf,
    runtime: Option<ProductionSelfAgentRuntime>,
}

impl SelfAgentHost {
    pub fn new(workspace: PathBuf, state_database: PathBuf, audit_log: PathBuf) -> Self {
        Self {
            workspace,
            state_database,
            audit_log,
            runtime: None,
        }
    }

    pub async fn submit(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<SelfAgentOutcome, SelfAgentHostError> {
        self.ensure_runtime()?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        runtime.start(prompt, cancel).await.map_err(Into::into)
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
