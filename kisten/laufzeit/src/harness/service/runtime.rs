use std::fmt;
use std::path::Path;
use std::sync::Arc;

use orchester_modell::{LanguageModel, ModelUsage};
use orchester_protokoll::{ActionId, AgentAction, CallId, PolicyDecision, RunId};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::{SelfAgentService, SelfAgentServiceError, SelfAgentTurn};
use crate::harness::agent_loop::SelfAgentLoop;
use crate::harness::audit::AuditSink;
use crate::harness::coordinator::SystemCoordinatorClock;
use crate::harness::execution::{GovernedExecution, GovernedExecutionError, GovernedToolOutcome};
use crate::harness::executor::ToolExecutor;
use crate::harness::run_store::SqliteRunStore;

pub enum SelfAgentOutcome {
    Model(SelfAgentTurn),
    Tool {
        run_id: RunId,
        action_id: ActionId,
        call_id: CallId,
        outcome: GovernedToolOutcome,
        model_calls: u32,
        usage: ModelUsage,
    },
}

impl SelfAgentOutcome {
    pub fn run_id(&self) -> &RunId {
        match self {
            Self::Model(turn) => turn.run_id(),
            Self::Tool { run_id, .. } => run_id,
        }
    }

    pub fn model_calls(&self) -> u32 {
        match self {
            Self::Model(turn) => turn.model_calls(),
            Self::Tool { model_calls, .. } => *model_calls,
        }
    }

    pub fn usage(&self) -> ModelUsage {
        match self {
            Self::Model(turn) => turn.usage(),
            Self::Tool { usage, .. } => *usage,
        }
    }
}

impl fmt::Debug for SelfAgentOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(turn) => formatter.debug_tuple("Model").field(turn).finish(),
            Self::Tool {
                outcome,
                model_calls,
                usage,
                ..
            } => formatter
                .debug_struct("Tool")
                .field("outcome", outcome)
                .field("model_calls", model_calls)
                .field("usage", usage)
                .finish(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SelfAgentRuntimeError {
    #[error(transparent)]
    Service(#[from] SelfAgentServiceError),
    #[error(transparent)]
    Execution(#[from] GovernedExecutionError),
}

pub struct SelfAgentRuntime<M, A> {
    service: SelfAgentService<M, Arc<SqliteRunStore>, SystemCoordinatorClock>,
    execution: GovernedExecution<A, SystemCoordinatorClock>,
}

impl<M, A> SelfAgentRuntime<M, A>
where
    M: LanguageModel,
    A: AuditSink,
{
    pub(super) fn from_parts(
        service: SelfAgentService<M, Arc<SqliteRunStore>, SystemCoordinatorClock>,
        execution: GovernedExecution<A, SystemCoordinatorClock>,
    ) -> Self {
        Self { service, execution }
    }

    pub fn new(
        loop_engine: SelfAgentLoop<M>,
        store: Arc<SqliteRunStore>,
        audit: Arc<A>,
        executor: ToolExecutor,
        workspace_root: impl AsRef<Path>,
        owner_actor_id: impl Into<String>,
    ) -> Result<Self, SelfAgentRuntimeError> {
        let owner_actor_id = owner_actor_id.into();
        let service = SelfAgentService::new(
            loop_engine,
            store.clone(),
            workspace_root,
            owner_actor_id.clone(),
        )?;
        let execution = GovernedExecution::new(store, audit, executor, owner_actor_id)?;
        Ok(Self::from_parts(service, execution))
    }

    pub fn model(&self) -> &M {
        self.service.model()
    }

    pub fn store(&self) -> &Arc<SqliteRunStore> {
        self.service.store()
    }

    pub async fn start(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
    ) -> Result<SelfAgentOutcome, SelfAgentRuntimeError> {
        let turn = self.service.start(prompt, cancel).await?;
        let should_execute = matches!(
            &turn,
            SelfAgentTurn::Action {
                action:
                    AgentAction::ListFiles { .. }
                    | AgentAction::SearchText { .. }
                    | AgentAction::ReadFile { .. },
                policy,
                ..
            } if policy.decision == PolicyDecision::Allow
        );
        if !should_execute {
            return Ok(SelfAgentOutcome::Model(turn));
        }

        let SelfAgentTurn::Action {
            run_id,
            action_id,
            call_id,
            model_calls,
            usage,
            ..
        } = turn
        else {
            unreachable!("execution predicate accepts only action turns");
        };
        let outcome = self.execution.execute(&run_id, &action_id, &call_id)?;
        Ok(SelfAgentOutcome::Tool {
            run_id,
            action_id,
            call_id,
            outcome,
            model_calls,
            usage,
        })
    }
}

impl<M, A> fmt::Debug for SelfAgentRuntime<M, A>
where
    M: LanguageModel,
    A: AuditSink,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentRuntime")
            .field("configured", &true)
            .finish_non_exhaustive()
    }
}
