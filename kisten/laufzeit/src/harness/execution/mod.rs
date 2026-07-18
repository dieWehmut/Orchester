//! Durable orchestration around permit-bound tool implementations.

mod observation;

use std::fmt;
use std::sync::Arc;

use orchester_protokoll::{ActionId, CallId, FeedbackReport, HarnessEventKind, Observation, RunId};
use thiserror::Error;

use super::audit::AuditSink;
use super::barrier::{BarrierError, ExecutionAuthorization, PreExecutionBarrier};
use super::coordinator::{CoordinatorClock, SystemCoordinatorClock};
use super::executor::ToolExecutor;
use super::run_store::{EventAppend, ResumeNext, RunStore, SqliteRunStore, StoreError};

const MAX_OWNER_BYTES: usize = 512;

pub enum GovernedToolOutcome {
    Completed(Observation),
    Failed(FeedbackReport),
}

impl fmt::Debug for GovernedToolOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed(observation) => formatter
                .debug_struct("Completed")
                .field("kind", &observation.kind)
                .field("summary_bytes", &observation.summary.len())
                .finish(),
            Self::Failed(feedback) => formatter
                .debug_struct("Failed")
                .field("classification", &feedback.classification)
                .field("retryable", &feedback.retryable)
                .finish(),
        }
    }
}

#[derive(Debug, Error)]
pub enum GovernedExecutionError {
    #[error("governed execution owner is invalid")]
    InvalidOwner,
    #[error("recorded action is not ready for execution")]
    NotReady,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Barrier(#[from] BarrierError),
}

pub struct GovernedExecution<S, C = SystemCoordinatorClock> {
    store: Arc<SqliteRunStore>,
    barrier: PreExecutionBarrier<S>,
    executor: ToolExecutor,
    owner_actor_id: String,
    clock: C,
}

impl<S> GovernedExecution<S, SystemCoordinatorClock>
where
    S: AuditSink,
{
    pub fn new(
        store: Arc<SqliteRunStore>,
        audit: Arc<S>,
        executor: ToolExecutor,
        owner_actor_id: impl Into<String>,
    ) -> Result<Self, GovernedExecutionError> {
        Self::with_clock(
            store,
            audit,
            executor,
            owner_actor_id,
            SystemCoordinatorClock,
        )
    }
}

impl<S, C> GovernedExecution<S, C>
where
    S: AuditSink,
    C: CoordinatorClock,
{
    pub fn with_clock(
        store: Arc<SqliteRunStore>,
        audit: Arc<S>,
        executor: ToolExecutor,
        owner_actor_id: impl Into<String>,
        clock: C,
    ) -> Result<Self, GovernedExecutionError> {
        let owner_actor_id = owner_actor_id.into();
        if owner_actor_id.trim().is_empty()
            || owner_actor_id.len() > MAX_OWNER_BYTES
            || owner_actor_id.chars().any(char::is_control)
        {
            return Err(GovernedExecutionError::InvalidOwner);
        }
        Ok(Self {
            barrier: PreExecutionBarrier::new(store.clone(), audit),
            store,
            executor,
            owner_actor_id,
            clock,
        })
    }

    pub fn execute(
        &self,
        run_id: &RunId,
        action_id: &ActionId,
        call_id: &CallId,
    ) -> Result<GovernedToolOutcome, GovernedExecutionError> {
        let run = self.store.load_run_owned(run_id, &self.owner_actor_id)?;
        let resume = self
            .store
            .resume_point_owned(run_id, &self.owner_actor_id, &run.project_id)?
            .ok_or(GovernedExecutionError::NotReady)?;
        let ResumeNext::PrepareExecution {
            action_id: ready_action,
            call_id: ready_call,
        } = resume.next
        else {
            return Err(GovernedExecutionError::NotReady);
        };
        if ready_action != *action_id || ready_call != *call_id {
            return Err(GovernedExecutionError::NotReady);
        }
        let turn_id = resume.turn_id.ok_or(GovernedExecutionError::NotReady)?;
        let step_id = resume.step_id.ok_or(GovernedExecutionError::NotReady)?;

        let permit = self.barrier.prepare(
            &self.owner_actor_id,
            run_id,
            action_id,
            ExecutionAuthorization::Allow,
            self.clock.now(),
        )?;
        let started = self.barrier.start_tool(
            &self.owner_actor_id,
            run_id,
            permit,
            EventAppend {
                turn_id: Some(turn_id.clone()),
                step_id: Some(step_id.clone()),
                call_id: Some(call_id.clone()),
                occurred_at: self.clock.now(),
                kind: HarnessEventKind::ToolStarted {
                    action_id: action_id.clone(),
                },
            },
        )?;

        let terminal = match self.executor.execute(started) {
            Ok(execution) => {
                match observation::from_execution(run_id, action_id, call_id, execution) {
                    Ok(observation) => HarnessEventKind::ToolCompleted { observation },
                    Err(()) => HarnessEventKind::ToolFailed {
                        feedback: observation::output_failure(),
                    },
                }
            }
            Err(error) => HarnessEventKind::ToolFailed {
                feedback: observation::tool_failure(error),
            },
        };
        let event = self.store.append_event(
            &self.owner_actor_id,
            run_id,
            EventAppend {
                turn_id: Some(turn_id),
                step_id: Some(step_id),
                call_id: Some(call_id.clone()),
                occurred_at: self.clock.now(),
                kind: terminal,
            },
        )?;
        match event.kind {
            HarnessEventKind::ToolCompleted { observation } => {
                Ok(GovernedToolOutcome::Completed(observation))
            }
            HarnessEventKind::ToolFailed { feedback } => Ok(GovernedToolOutcome::Failed(feedback)),
            _ => Err(StoreError::Invariant(
                "tool terminal persistence returned an unexpected event".into(),
            )
            .into()),
        }
    }
}

impl<S, C> fmt::Debug for GovernedExecution<S, C>
where
    S: AuditSink,
    C: CoordinatorClock,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedExecution")
            .field("owner_actor_id", &"[REDACTED]")
            .field("executor", &self.executor)
            .field("clock_configured", &true)
            .finish_non_exhaustive()
    }
}
