//! TUI-independent entry point for the self-owned agent.

mod builder;
mod config_view;
mod credentials;
mod events;
mod identity;
mod model_catalog;
mod permissions;
mod provider_editor;
mod resume_catalog;
mod runtime;
mod status;
mod turn;
mod unresolved;

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use orchester_protokoll::RunId;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::agent_loop::SelfAgentLoop;
use super::coordinator::{
    CoordinatorClock, CoordinatorError, CoordinatorStore, DurableCoordinator,
    SystemCoordinatorClock,
};
use super::governance::PolicyEngine;
pub use builder::{
    build_self_agent_runtime, build_self_agent_runtime_with_transport, build_self_agent_service,
    build_self_agent_service_with_transport, ConfiguredSelfAgentRuntime,
    ProductionSelfAgentRuntime, ProductionSelfAgentService, SelfAgentBuildError,
    SelfAgentRuntimeBuildError,
};
pub use config_view::{load_self_agent_config_view, ConfigResolution, SelfAgentConfigView};
pub use credentials::{
    clear_provider_credential, resolve_credential_target, store_provider_credential,
    wire_provider_reference, ConfigWiring, CredentialEntryError, CredentialTarget,
    CredentialUpdate,
};
pub use events::RunEventSink;
use events::RunNarrator;
use identity::WorkspaceIdentity;
pub use identity::{IdentityError, WorkspaceIdentitySnapshot};
pub use model_catalog::{
    load_self_agent_model_catalog, select_self_agent_model_profile,
    select_self_agent_model_provider, SelfAgentActiveModel, SelfAgentModelCatalog,
    SelfAgentModelCatalogError, SelfAgentModelChoice, SelfAgentModelSession,
    SelfAgentProviderChoice, SelfAgentProviderState,
};
pub use permissions::{
    load_self_agent_permissions, SelfAgentApprovalStatus, SelfAgentAuditStatus,
    SelfAgentPermissionGovernance, SelfAgentPermissionRule, SelfAgentPermissionSnapshot,
};
pub use provider_editor::{
    provider_draft, write_self_agent_provider, ProviderDraft, ProviderEdit, ProviderEditError,
    PROVIDER_WIRE_APIS,
};
pub use resume_catalog::{
    load_self_agent_resume_catalog, resolve_self_agent_resume_handle, SelfAgentResumeAvailability,
    SelfAgentResumeCatalog, SelfAgentResumeCatalogError, SelfAgentResumeEntry,
    SelfAgentResumeStage, SelfAgentResumeStep, SelfAgentResumeTargetError,
};
pub use runtime::{
    SelfAgentOutcome, SelfAgentRunOutcome, SelfAgentRuntime, SelfAgentRuntimeError,
    SelfAgentToolStep,
};
pub use status::{
    load_self_agent_status, SelfAgentDurableStatus, SelfAgentGovernanceStatus,
    SelfAgentLimitStatus, SelfAgentModelReport, SelfAgentModelStatus, SelfAgentStatus,
    SelfAgentStatusError,
};
pub use turn::SelfAgentTurn;

/// Errors raised before or during one self-agent turn.
#[derive(Debug, Error)]
pub enum SelfAgentServiceError {
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Coordinator(#[from] CoordinatorError),
}

/// Owns the durable coordinator and the workspace identity needed to admit a
/// prompt as a new run.
pub struct SelfAgentService<M, S, C = SystemCoordinatorClock> {
    coordinator: DurableCoordinator<M, S, C>,
    identity: WorkspaceIdentity,
    config_snapshot_hash: String,
    max_steps: u64,
}

impl<M, S> SelfAgentService<M, S, SystemCoordinatorClock>
where
    M: orchester_modell::LanguageModel,
    S: CoordinatorStore,
{
    pub fn new(
        loop_engine: SelfAgentLoop<M>,
        store: S,
        workspace_root: impl AsRef<Path>,
        owner_actor_id: impl Into<String>,
    ) -> Result<Self, SelfAgentServiceError> {
        Self::with_clock(
            loop_engine,
            store,
            workspace_root,
            owner_actor_id,
            SystemCoordinatorClock,
        )
    }
}

impl<M, S, C> SelfAgentService<M, S, C>
where
    M: orchester_modell::LanguageModel,
    S: CoordinatorStore,
    C: CoordinatorClock,
{
    pub fn with_clock(
        loop_engine: SelfAgentLoop<M>,
        store: S,
        workspace_root: impl AsRef<Path>,
        owner_actor_id: impl Into<String>,
        clock: C,
    ) -> Result<Self, SelfAgentServiceError> {
        let identity = WorkspaceIdentity::for_workspace(workspace_root, owner_actor_id)?;
        Ok(Self::from_identity(loop_engine, store, identity, clock))
    }

    fn from_identity(
        loop_engine: SelfAgentLoop<M>,
        store: S,
        identity: WorkspaceIdentity,
        clock: C,
    ) -> Self {
        Self::from_identity_with_policy(loop_engine, store, identity, PolicyEngine::new(), clock)
    }

    fn from_identity_with_policy(
        loop_engine: SelfAgentLoop<M>,
        store: S,
        identity: WorkspaceIdentity,
        policy: PolicyEngine,
        clock: C,
    ) -> Self {
        let config_snapshot_hash = loop_engine.config_snapshot_hash();
        let max_steps = u64::from(loop_engine.max_steps());
        Self {
            coordinator: DurableCoordinator::with_policy_and_clock(
                loop_engine,
                store,
                policy,
                clock,
            ),
            identity,
            config_snapshot_hash,
            max_steps,
        }
    }

    pub fn identity(&self) -> WorkspaceIdentitySnapshot {
        self.identity.snapshot()
    }

    pub fn store(&self) -> &S {
        self.coordinator.store()
    }

    pub fn model(&self) -> &M {
        self.coordinator.model()
    }

    pub async fn start(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
    ) -> Result<SelfAgentTurn, SelfAgentServiceError> {
        self.start_with_events(prompt, cancel, None).await
    }

    pub async fn start_with_events(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
        events: Option<Arc<dyn orchester_modell::ModelEventSink>>,
    ) -> Result<SelfAgentTurn, SelfAgentServiceError> {
        let (input, run_id) = self.identity.coordinator_input(
            prompt.into(),
            self.config_snapshot_hash.clone(),
            self.max_steps,
            self.coordinator.policy_snapshot_hash(),
        )?;
        let outcome = self
            .coordinator
            .start_new_run_with_events(input, cancel, events)
            .await?;
        Ok(SelfAgentTurn::from_coordinator(run_id, outcome))
    }

    pub async fn continue_run(
        &self,
        run_id: RunId,
        cancel: CancellationToken,
    ) -> Result<SelfAgentTurn, SelfAgentServiceError> {
        self.continue_run_with_events(run_id, cancel, None).await
    }

    pub async fn continue_run_with_events(
        &self,
        run_id: RunId,
        cancel: CancellationToken,
        events: Option<Arc<dyn orchester_modell::ModelEventSink>>,
    ) -> Result<SelfAgentTurn, SelfAgentServiceError> {
        let input = self.identity.continuation_input(run_id.clone())?;
        let outcome = self
            .coordinator
            .continue_run_with_events(input, cancel, events)
            .await?;
        Ok(SelfAgentTurn::from_coordinator(run_id, outcome))
    }
}

impl<M, S, C> fmt::Debug for SelfAgentService<M, S, C>
where
    M: orchester_modell::LanguageModel,
    S: CoordinatorStore,
    C: CoordinatorClock + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentService")
            .field("identity", &self.identity)
            .field("config_snapshot_hash", &self.config_snapshot_hash)
            .field("max_steps", &self.max_steps)
            .finish_non_exhaustive()
    }
}
