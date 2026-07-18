use std::path::Path;

use thiserror::Error;

use super::identity::{IdentityError, WorkspaceIdentity};
use super::SelfAgentService;
use crate::harness::agent_loop::{AgentLoopConfig, AgentLoopError, SelfAgentLoop};
use crate::harness::config::{ConfigError, UserConfig};
use crate::harness::context::{ContextAssembler, ContextLimits};
use crate::harness::coordinator::SystemCoordinatorClock;
use crate::harness::credentials::CredentialStore;
use crate::harness::provider::responses::{
    build_responses_model, build_responses_model_with_transport, ConfiguredResponsesModel,
    ResponsesModelBuildError,
};
use crate::harness::provider::{HttpTransport, ReqwestHttpTransport};
use crate::harness::run_store::{SqliteRunStore, StoreError};

pub type ProductionSelfAgentService = SelfAgentService<
    ConfiguredResponsesModel<ReqwestHttpTransport>,
    SqliteRunStore,
    SystemCoordinatorClock,
>;

#[derive(Debug, Error)]
pub enum SelfAgentBuildError {
    #[error(transparent)]
    Model(#[from] ResponsesModelBuildError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Loop(#[from] AgentLoopError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub fn build_self_agent_service<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<ProductionSelfAgentService, SelfAgentBuildError> {
    let model = build_responses_model(config, credentials)?;
    finish_build(
        config,
        credentials,
        model,
        workspace_root,
        state_database,
        owner_actor_id,
    )
}

pub fn build_self_agent_service_with_transport<S, T>(
    config: &UserConfig,
    credentials: &S,
    transport: T,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<
    SelfAgentService<ConfiguredResponsesModel<T>, SqliteRunStore, SystemCoordinatorClock>,
    SelfAgentBuildError,
>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport + 'static,
{
    let model = build_responses_model_with_transport(config, credentials, transport)?;
    finish_build(
        config,
        credentials,
        model,
        workspace_root,
        state_database,
        owner_actor_id,
    )
}

fn finish_build<S, T>(
    config: &UserConfig,
    credentials: &S,
    model: ConfiguredResponsesModel<T>,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<
    SelfAgentService<ConfiguredResponsesModel<T>, SqliteRunStore, SystemCoordinatorClock>,
    SelfAgentBuildError,
>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport + 'static,
{
    let secrets = config.resolve_configured_secrets(credentials)?;
    let identity = WorkspaceIdentity::for_workspace(workspace_root, owner_actor_id)?;
    let profile = model.profile().clone();
    let loop_engine = SelfAgentLoop::new(
        model,
        ContextAssembler::new(ContextLimits::default(), secrets.values.clone()),
        AgentLoopConfig {
            model: profile.model,
            max_steps: config.limits.max_steps,
            max_text_bytes: config.limits.max_observation_bytes,
            store: profile.store,
        },
    )?;
    let store = SqliteRunStore::open_with_terminal_secrets(state_database, secrets.values)?;
    Ok(SelfAgentService::from_identity(
        loop_engine,
        store,
        identity,
        SystemCoordinatorClock,
    ))
}
