//! TUI-independent entry point for the self-owned agent.

mod builder;
mod identity;
mod turn;

use std::fmt;
use std::path::Path;

use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::agent_loop::SelfAgentLoop;
use super::coordinator::{
    CoordinatorClock, CoordinatorError, CoordinatorOutcome, CoordinatorStore, DurableCoordinator,
    SystemCoordinatorClock,
};
use super::governance::PolicyEngine;
pub use builder::{
    build_self_agent_service, build_self_agent_service_with_transport, ProductionSelfAgentService,
    SelfAgentBuildError,
};
use identity::WorkspaceIdentity;
pub use identity::{IdentityError, WorkspaceIdentitySnapshot};
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
        let config_snapshot_hash = loop_engine.config_snapshot_hash();
        let max_steps = u64::from(loop_engine.max_steps());
        Self {
            coordinator: DurableCoordinator::with_clock(loop_engine, store, clock),
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
        let (input, run_id) = self.identity.coordinator_input(
            prompt.into(),
            self.config_snapshot_hash.clone(),
            self.max_steps,
            PolicyEngine::snapshot_hash(),
        )?;
        let outcome = self.coordinator.start_new_run(input, cancel).await?;
        Ok(match outcome {
            CoordinatorOutcome::Text {
                text,
                model_calls,
                usage,
            } => SelfAgentTurn::Text {
                run_id,
                text,
                model_calls,
                usage,
            },
            CoordinatorOutcome::Action {
                action_id,
                call_id,
                action,
                model_calls,
                usage,
            } => SelfAgentTurn::Action {
                run_id,
                action_id,
                call_id,
                action,
                model_calls,
                usage,
            },
        })
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
