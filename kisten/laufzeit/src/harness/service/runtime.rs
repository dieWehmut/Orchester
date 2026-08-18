mod outcome;

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use orchester_modell::{LanguageModel, ModelEventSink, ModelUsage};
use orchester_protokoll::{ActionId, AgentAction, CallId, PolicyDecision, RunId};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::{
    resolve_self_agent_resume_handle, RunEventSink, RunNarrator, SelfAgentResumeTargetError,
    SelfAgentService, SelfAgentServiceError, SelfAgentTurn,
};
use crate::harness::agent_loop::SelfAgentLoop;
use crate::harness::audit::AuditSink;
use crate::harness::coordinator::SystemCoordinatorClock;
use crate::harness::execution::{GovernedExecution, GovernedExecutionError, GovernedToolOutcome};
use crate::harness::executor::ToolExecutor;
use crate::harness::feedback::StreamingRedactor;
use crate::harness::run_store::SqliteRunStore;

pub use outcome::{SelfAgentRunOutcome, SelfAgentToolStep};

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
    #[error(transparent)]
    Resume(#[from] SelfAgentResumeTargetError),
    #[error("self-agent run was cancelled")]
    Cancelled,
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

    pub fn streaming_redactor(&self) -> StreamingRedactor {
        self.store().streaming_redactor()
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

    /// Advance one durable run across every automatically executable file
    /// action, stopping at the next model response that needs a caller.
    pub async fn run(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        self.run_with_events(prompt, cancel, None).await
    }

    pub async fn run_with_events(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        self.run_streaming(prompt, cancel, events, None).await
    }

    /// Run while narrating the unified [`orchester_protokoll::Event`] stream to
    /// `run_events`, for a frontend that does not own the run.
    pub async fn run_streaming(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
        run_events: Option<Arc<dyn RunEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        let narrator = self.narrator(run_events);
        let turn = self
            .service
            .start_with_events(prompt, cancel.clone(), events.clone())
            .await
            .inspect_err(|error| narrator.failed(&error.to_string()))?;
        self.drive_turn(turn, cancel, events, &narrator).await
    }

    pub async fn resume(
        &self,
        handle: &str,
        cancel: CancellationToken,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        self.resume_with_events(handle, cancel, None).await
    }

    pub async fn resume_with_events(
        &self,
        handle: &str,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        self.resume_streaming(handle, cancel, events, None).await
    }

    /// Resume while narrating the unified event stream to `run_events`.
    pub async fn resume_streaming(
        &self,
        handle: &str,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
        run_events: Option<Arc<dyn RunEventSink>>,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        let narrator = self.narrator(run_events);
        let identity = self.service.identity();
        let run_id = match resolve_self_agent_resume_handle(self.store(), &identity, handle) {
            Ok(run_id) => run_id,
            Err(error) => {
                narrator.failed(&error.to_string());
                return Err(error.into());
            }
        };
        let turn = self
            .service
            .continue_run_with_events(run_id, cancel.clone(), events.clone())
            .await
            .inspect_err(|error| narrator.failed(&error.to_string()))?;
        self.drive_turn(turn, cancel, events, &narrator).await
    }

    fn narrator(&self, run_events: Option<Arc<dyn RunEventSink>>) -> RunNarrator {
        RunNarrator::new(run_events, self.streaming_redactor())
    }

    async fn drive_turn(
        &self,
        mut turn: SelfAgentTurn,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
        narrator: &RunNarrator,
    ) -> Result<SelfAgentRunOutcome, SelfAgentRuntimeError> {
        let mut tool_steps = Vec::new();
        narrator.started(turn.run_id());

        loop {
            let Some((run_id, action_id, call_id)) = executable_file_action(&turn) else {
                narrator.finished(&turn);
                return Ok(SelfAgentRunOutcome::new(turn, tool_steps));
            };
            if cancel.is_cancelled() {
                narrator.cancelled();
                return Err(SelfAgentRuntimeError::Cancelled);
            }
            let action = turn
                .action()
                .expect("an executable action turn carries its action");

            narrator.tool_started(action);
            let outcome = self
                .execution
                .execute(&run_id, &action_id, &call_id)
                .inspect_err(|error| narrator.failed(&error.to_string()))?;
            narrator.tool_finished(action, &outcome);

            tool_steps.push(SelfAgentToolStep::new(action_id, call_id, outcome));
            turn = self
                .service
                .continue_run_with_events(run_id, cancel.clone(), events.clone())
                .await
                .inspect_err(|error| narrator.failed(&error.to_string()))?;
        }
    }
}

fn executable_file_action(turn: &SelfAgentTurn) -> Option<(RunId, ActionId, CallId)> {
    match turn {
        SelfAgentTurn::Action {
            run_id,
            action_id,
            call_id,
            action:
                AgentAction::ListFiles { .. }
                | AgentAction::SearchText { .. }
                | AgentAction::ReadFile { .. },
            policy,
            ..
        } if policy.decision == PolicyDecision::Allow => {
            Some((run_id.clone(), action_id.clone(), call_id.clone()))
        }
        SelfAgentTurn::Text { .. } | SelfAgentTurn::Action { .. } => None,
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
