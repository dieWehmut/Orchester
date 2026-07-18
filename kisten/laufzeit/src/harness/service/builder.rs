use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use super::identity::{IdentityError, WorkspaceIdentity};
use super::{SelfAgentRuntime, SelfAgentService};
use crate::harness::agent_loop::{AgentLoopConfig, AgentLoopError, SelfAgentLoop};
use crate::harness::audit::{AuditError, JsonlAuditSink};
use crate::harness::config::{ConfigError, UserConfig};
use crate::harness::context::{ContextAssembler, ContextLimits};
use crate::harness::coordinator::SystemCoordinatorClock;
use crate::harness::credentials::CredentialStore;
use crate::harness::execution::{GovernedExecution, GovernedExecutionError};
use crate::harness::executor::{ToolExecutor, ToolExecutorError};
use crate::harness::files::FileToolLimits;
use crate::harness::provider::responses::{
    build_responses_model, build_responses_model_with_transport, ConfiguredResponsesModel,
    ResponsesModelBuildError,
};
use crate::harness::provider::{HttpTransport, ReqwestHttpTransport};
use crate::harness::run_store::{SqliteRunStore, StoreError};

pub type ProductionSelfAgentService = SelfAgentService<
    ConfiguredResponsesModel<ReqwestHttpTransport>,
    Arc<SqliteRunStore>,
    SystemCoordinatorClock,
>;

pub type ConfiguredSelfAgentRuntime<T> =
    SelfAgentRuntime<ConfiguredResponsesModel<T>, JsonlAuditSink>;

pub type ProductionSelfAgentRuntime = ConfiguredSelfAgentRuntime<ReqwestHttpTransport>;

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

#[derive(Debug, Error)]
pub enum SelfAgentRuntimeBuildError {
    #[error(transparent)]
    Service(#[from] SelfAgentBuildError),
    #[error(transparent)]
    Audit(#[from] AuditError),
    #[error(transparent)]
    Executor(#[from] ToolExecutorError),
    #[error(transparent)]
    Execution(#[from] GovernedExecutionError),
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
    SelfAgentService<ConfiguredResponsesModel<T>, Arc<SqliteRunStore>, SystemCoordinatorClock>,
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

pub fn build_self_agent_runtime<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    audit_log: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<ProductionSelfAgentRuntime, SelfAgentRuntimeBuildError> {
    let workspace_root = workspace_root.as_ref();
    let owner_actor_id = owner_actor_id.into();
    let service = build_self_agent_service(
        config,
        credentials,
        workspace_root,
        state_database,
        owner_actor_id.clone(),
    )?;
    finish_runtime(config, service, workspace_root, audit_log, owner_actor_id)
}

pub fn build_self_agent_runtime_with_transport<S, T>(
    config: &UserConfig,
    credentials: &S,
    transport: T,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    audit_log: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<ConfiguredSelfAgentRuntime<T>, SelfAgentRuntimeBuildError>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport + 'static,
{
    let workspace_root = workspace_root.as_ref();
    let owner_actor_id = owner_actor_id.into();
    let service = build_self_agent_service_with_transport(
        config,
        credentials,
        transport,
        workspace_root,
        state_database,
        owner_actor_id.clone(),
    )?;
    finish_runtime(config, service, workspace_root, audit_log, owner_actor_id)
}

fn finish_build<S, T>(
    config: &UserConfig,
    credentials: &S,
    model: ConfiguredResponsesModel<T>,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<
    SelfAgentService<ConfiguredResponsesModel<T>, Arc<SqliteRunStore>, SystemCoordinatorClock>,
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
    let store = Arc::new(SqliteRunStore::open_with_terminal_secrets(
        state_database,
        secrets.values,
    )?);
    Ok(SelfAgentService::from_identity(
        loop_engine,
        store,
        identity,
        SystemCoordinatorClock,
    ))
}

fn finish_runtime<T>(
    config: &UserConfig,
    service: SelfAgentService<
        ConfiguredResponsesModel<T>,
        Arc<SqliteRunStore>,
        SystemCoordinatorClock,
    >,
    workspace_root: &Path,
    audit_log: impl AsRef<Path>,
    owner_actor_id: String,
) -> Result<ConfiguredSelfAgentRuntime<T>, SelfAgentRuntimeBuildError>
where
    T: HttpTransport + 'static,
{
    let store = service.store().clone();
    let audit = Arc::new(JsonlAuditSink::open(audit_log)?);
    let mut file_limits = FileToolLimits::default();
    let observation_limit = u64::try_from(config.limits.max_observation_bytes).unwrap_or(u64::MAX);
    file_limits.max_read_bytes = file_limits.max_read_bytes.min(observation_limit);
    let executor = ToolExecutor::new(workspace_root, file_limits)?;
    let execution = GovernedExecution::new(store, audit, executor, owner_actor_id)?;
    Ok(SelfAgentRuntime::from_parts(service, execution))
}
