//! Durable boundary for one self-agent model step.
//!
//! The coordinator records a model response and decoded action, then asks the
//! durable store for the authoritative policy decision. Permit-bound execution
//! and validator-gated completion remain subsequent phases. Keeping this
//! boundary narrow makes the crash point explicit: a provider is never called
//! until the durable store has accepted `model.started`.

use std::sync::Arc;

use orchester_modell::{LanguageModel, ModelError, ModelUsage};
use orchester_protokoll::{ActionId, AgentAction, CallId, HarnessEventKind, RunId, StepId, TurnId};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::agent_loop::{AgentLoopError, PreparedModelStep, PreparedOutcome, SelfAgentLoop};
use super::feedback::SecretSetId;
use super::governance::{PolicyEngine, PolicyResult};
use super::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, RunStore, SqliteRunStore, StoreError,
    Transition,
};
use super::transcript::TranscriptLimits;
use super::transcript::TranscriptRecord;

mod continuation;

use continuation::load_sqlite_continuation;
pub use continuation::{CoordinatorContinuationInput, CoordinatorContinuationState};

const MAX_COORDINATOR_ID_BYTES: usize = 256;
const MAX_COORDINATOR_FIELD_BYTES: usize = 512;
const MAX_COORDINATOR_PATH_BYTES: usize = 32 * 1024;

/// Clock sampled at each durable boundary. Production uses the system clock;
/// deterministic tests can inject a fixed value.
pub trait CoordinatorClock: Send + Sync {
    fn now(&self) -> String;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCoordinatorClock;

impl CoordinatorClock for SystemCoordinatorClock {
    fn now(&self) -> String {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("unix:{seconds}")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FixedCoordinatorClock {
    timestamp: String,
}

impl std::fmt::Debug for FixedCoordinatorClock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FixedCoordinatorClock")
            .field("timestamp_bytes", &self.timestamp.len())
            .finish()
    }
}

impl FixedCoordinatorClock {
    pub fn new(timestamp: impl Into<String>) -> Self {
        Self {
            timestamp: timestamp.into(),
        }
    }
}

impl CoordinatorClock for FixedCoordinatorClock {
    fn now(&self) -> String {
        self.timestamp.clone()
    }
}

/// Inputs that must be fixed before a model request is admitted to the store.
///
/// IDs are explicit for this first offline slice; timestamps come from the
/// coordinator clock at the actual boundary. The CLI will not construct this
/// directly until its self-agent service is wired.
#[derive(Clone, PartialEq, Eq)]
pub struct CoordinatorInput {
    pub run: NewRun,
    pub prompt: String,
    pub turn_id: TurnId,
    pub step_id: StepId,
    pub model_call_id: CallId,
    pub action_id: ActionId,
}

struct CoordinatorStepInput {
    run_id: RunId,
    owner_actor_id: String,
    turn_id: TurnId,
    step_id: StepId,
    model_call_id: CallId,
    action_id: ActionId,
}

impl std::fmt::Debug for CoordinatorInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoordinatorInput")
            .field("run_id", &"<redacted>")
            .field("project_id_bytes", &self.run.project_id.len())
            .field("owner_actor_id", &"<redacted>")
            .field("prompt_bytes", &self.prompt.len())
            .field("turn_id", &"<redacted>")
            .field("step_id", &"<redacted>")
            .field("model_call_id", &"<redacted>")
            .field("action_id", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("coordinator input is invalid")]
    InvalidInput,
    #[error("coordinator dependencies do not match the run snapshot")]
    DependencyMismatch,
    #[error("model text exceeds the durable transcript limit")]
    DurableTextTooLarge,
    #[error("model action cannot be represented in the durable transcript")]
    DurableActionInvalid,
    #[error(transparent)]
    Loop(#[from] AgentLoopError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("action policy classification failed")]
    Policy,
}

/// The durable result of one model boundary.  A text result is intentionally
/// not terminal: validation and mutation evidence must gate `run.completed`.
pub enum CoordinatorOutcome {
    Text {
        text: String,
        model_calls: u32,
        usage: ModelUsage,
    },
    Action {
        action_id: ActionId,
        call_id: CallId,
        action: AgentAction,
        policy: PolicyResult,
        model_calls: u32,
        usage: ModelUsage,
    },
}

impl std::fmt::Debug for CoordinatorOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text {
                text,
                model_calls,
                usage,
            } => formatter
                .debug_struct("Text")
                .field("text_bytes", &text.len())
                .field("model_calls", model_calls)
                .field("usage", usage)
                .finish(),
            Self::Action {
                action_id: _,
                call_id: _,
                action,
                policy,
                model_calls,
                usage,
            } => formatter
                .debug_struct("Action")
                .field("action_summary", &action.action_summary())
                .field("policy_decision", &policy.decision)
                .field("policy_rule_id", &policy.rule_id)
                .field("model_calls", model_calls)
                .field("usage", usage)
                .finish(),
        }
    }
}

/// The narrow persistence surface needed by the model-step coordinator.
///
/// It is intentionally separate from the general `RunStore` trait because
/// model request transcript binding and model+action completion are atomic
/// operations that are not safe to reconstruct from generic event calls.
pub trait CoordinatorStore: Send + Sync {
    fn secret_set_id(&self) -> SecretSetId;

    fn create_run(&self, input: NewRun) -> Result<(), StoreError>;

    fn load_continuation(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<CoordinatorContinuationState, StoreError>;

    fn append_transition(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        transition: Transition,
    ) -> Result<(), StoreError>;

    fn append_model_started_with_transcript(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        records: Vec<TranscriptRecord>,
    ) -> Result<(), StoreError>;

    fn append_model_completed(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        usage: ModelUsage,
    ) -> Result<String, StoreError>;

    fn append_model_completed_with_action(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        action: ActionRecord,
        usage: ModelUsage,
    ) -> Result<(), StoreError>;

    fn decide_policy(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        occurred_at: String,
    ) -> Result<PolicyResult, StoreError>;
}

impl<T> CoordinatorStore for Arc<T>
where
    T: CoordinatorStore + ?Sized,
{
    fn secret_set_id(&self) -> SecretSetId {
        self.as_ref().secret_set_id()
    }

    fn create_run(&self, input: NewRun) -> Result<(), StoreError> {
        self.as_ref().create_run(input)
    }

    fn load_continuation(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<CoordinatorContinuationState, StoreError> {
        self.as_ref().load_continuation(run_id, owner_actor_id)
    }

    fn append_transition(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        transition: Transition,
    ) -> Result<(), StoreError> {
        self.as_ref()
            .append_transition(run_id, owner_actor_id, transition)
    }

    fn append_model_started_with_transcript(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        records: Vec<TranscriptRecord>,
    ) -> Result<(), StoreError> {
        self.as_ref()
            .append_model_started_with_transcript(owner_actor_id, run_id, input, records)
    }

    fn append_model_completed(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        usage: ModelUsage,
    ) -> Result<String, StoreError> {
        self.as_ref()
            .append_model_completed(owner_actor_id, run_id, input, usage)
    }

    fn append_model_completed_with_action(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        action: ActionRecord,
        usage: ModelUsage,
    ) -> Result<(), StoreError> {
        self.as_ref().append_model_completed_with_action(
            owner_actor_id,
            run_id,
            input,
            action,
            usage,
        )
    }

    fn decide_policy(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        occurred_at: String,
    ) -> Result<PolicyResult, StoreError> {
        self.as_ref()
            .decide_policy(owner_actor_id, run_id, action_id, occurred_at)
    }
}

impl CoordinatorStore for SqliteRunStore {
    fn secret_set_id(&self) -> SecretSetId {
        SqliteRunStore::secret_set_id(self)
    }

    fn create_run(&self, input: NewRun) -> Result<(), StoreError> {
        <Self as RunStore>::create_run(self, input).map(|_| ())
    }

    fn load_continuation(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<CoordinatorContinuationState, StoreError> {
        load_sqlite_continuation(self, run_id, owner_actor_id)
    }

    fn append_transition(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        transition: Transition,
    ) -> Result<(), StoreError> {
        <Self as RunStore>::append_transition(self, run_id, owner_actor_id, transition).map(|_| ())
    }

    fn append_model_started_with_transcript(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        records: Vec<TranscriptRecord>,
    ) -> Result<(), StoreError> {
        SqliteRunStore::append_model_started_with_transcript(
            self,
            owner_actor_id,
            run_id,
            input,
            records,
        )
        .map(|_| ())
    }

    fn append_model_completed(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        usage: ModelUsage,
    ) -> Result<String, StoreError> {
        let event = self.append_model_completed_with_usage(owner_actor_id, run_id, input, usage)?;
        let HarnessEventKind::ModelCompleted { assistant_text } = event.kind else {
            return Err(StoreError::Invariant(
                "coordinator completion did not persist a model event".into(),
            ));
        };
        Ok(assistant_text)
    }

    fn append_model_completed_with_action(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        action: ActionRecord,
        usage: ModelUsage,
    ) -> Result<(), StoreError> {
        SqliteRunStore::append_model_completed_with_action_and_usage(
            self,
            owner_actor_id,
            run_id,
            input,
            action,
            usage,
        )
        .map(|_| ())
    }

    fn decide_policy(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        occurred_at: String,
    ) -> Result<PolicyResult, StoreError> {
        SqliteRunStore::decide_policy(self, owner_actor_id, run_id, action_id, occurred_at)
            .map(|(_, result)| result)
    }
}

/// Drives durable model calls through separate new-run and continuation paths
/// so a resumed step can never recreate its run.
pub struct DurableCoordinator<M, S, C = SystemCoordinatorClock> {
    loop_engine: SelfAgentLoop<M>,
    store: S,
    clock: C,
}

impl<M, S, C> std::fmt::Debug for DurableCoordinator<M, S, C>
where
    M: LanguageModel,
    S: CoordinatorStore,
    C: CoordinatorClock + std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DurableCoordinator")
            .field("loop", &self.loop_engine)
            .finish_non_exhaustive()
    }
}

impl<M, S> DurableCoordinator<M, S, SystemCoordinatorClock>
where
    M: LanguageModel,
    S: CoordinatorStore,
{
    pub fn new(loop_engine: SelfAgentLoop<M>, store: S) -> Self {
        Self::with_clock(loop_engine, store, SystemCoordinatorClock)
    }
}

impl<M, S, C> DurableCoordinator<M, S, C>
where
    M: LanguageModel,
    S: CoordinatorStore,
    C: CoordinatorClock,
{
    pub fn with_clock(loop_engine: SelfAgentLoop<M>, store: S, clock: C) -> Self {
        Self {
            loop_engine,
            store,
            clock,
        }
    }

    pub fn model(&self) -> &M {
        self.loop_engine.model()
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn start_new_run(
        &self,
        input: CoordinatorInput,
        cancel: CancellationToken,
    ) -> Result<CoordinatorOutcome, CoordinatorError> {
        self.validate_input(&input)?;
        self.validate_dependencies(&input)?;

        // Assemble and validate context before mutating durable state.  A
        // rejected prompt therefore cannot leave an empty run behind.
        let prepared = self
            .loop_engine
            .prepare_start(input.prompt.clone(), &cancel)?;
        let mut run = input.run.clone();
        run.occurred_at = self.durable_timestamp()?;
        let run_id = run.run_id.clone();
        let owner = run.owner_actor_id.clone();
        self.store.create_run(run)?;
        let step_started_at = self.durable_timestamp()?;
        self.store.append_transition(
            &run_id,
            &owner,
            Transition::StartStep {
                turn_id: input.turn_id.clone(),
                step_id: input.step_id.clone(),
                occurred_at: step_started_at,
            },
        )?;
        let model_started_at = self.durable_timestamp()?;
        self.store.append_model_started_with_transcript(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(input.turn_id.clone()),
                step_id: Some(input.step_id.clone()),
                call_id: Some(input.model_call_id.clone()),
                occurred_at: model_started_at,
                kind: HarnessEventKind::ModelStarted,
            },
            vec![TranscriptRecord::user(input.prompt)],
        )?;
        self.complete_model_step(
            prepared,
            CoordinatorStepInput {
                run_id,
                owner_actor_id: owner,
                turn_id: input.turn_id,
                step_id: input.step_id,
                model_call_id: input.model_call_id,
                action_id: input.action_id,
            },
            cancel,
        )
        .await
    }

    pub async fn continue_run(
        &self,
        input: CoordinatorContinuationInput,
        cancel: CancellationToken,
    ) -> Result<CoordinatorOutcome, CoordinatorError> {
        self.validate_continuation_input(&input)?;
        let state = self
            .store
            .load_continuation(&input.run_id, &input.owner_actor_id)?;
        self.validate_continuation_state(&input, &state)?;
        let model_calls = u32::try_from(state.run.steps_used)
            .map_err(|_| CoordinatorError::DependencyMismatch)?;
        let usage = ModelUsage {
            input_tokens: state.run.input_tokens_used,
            output_tokens: state.run.output_tokens_used,
        };
        let prepared = self.loop_engine.prepare_durable_resume(
            state.transcript,
            model_calls,
            usage,
            &cancel,
        )?;
        self.store.append_transition(
            &input.run_id,
            &input.owner_actor_id,
            Transition::StartStep {
                turn_id: state.turn_id.clone(),
                step_id: input.step_id.clone(),
                occurred_at: self.durable_timestamp()?,
            },
        )?;
        self.store.append_model_started_with_transcript(
            &input.owner_actor_id,
            &input.run_id,
            EventAppend {
                turn_id: Some(state.turn_id.clone()),
                step_id: Some(input.step_id.clone()),
                call_id: Some(input.model_call_id.clone()),
                occurred_at: self.durable_timestamp()?,
                kind: HarnessEventKind::ModelStarted,
            },
            Vec::new(),
        )?;
        self.complete_model_step(
            prepared,
            CoordinatorStepInput {
                run_id: input.run_id,
                owner_actor_id: input.owner_actor_id,
                turn_id: state.turn_id,
                step_id: input.step_id,
                model_call_id: input.model_call_id,
                action_id: input.action_id,
            },
            cancel,
        )
        .await
    }

    async fn complete_model_step(
        &self,
        prepared: PreparedModelStep,
        input: CoordinatorStepInput,
        cancel: CancellationToken,
    ) -> Result<CoordinatorOutcome, CoordinatorError> {
        // This durable boundary is the last fallible operation before the
        // provider call. Failures after it resume as ReconcileModelCall.
        let response = self
            .loop_engine
            .model()
            .complete(prepared.request().clone(), cancel)
            .await?;
        let response_usage = response.usage;
        if response.assistant_text.len() > TranscriptLimits::DEFAULT_MAX_TEXT_BYTES {
            return Err(CoordinatorError::DurableTextTooLarge);
        }
        if let Some(call) = response.tool_call.as_ref() {
            self.loop_engine
                .validate_durable_record(&TranscriptRecord::tool_call(
                    call.call_id.clone(),
                    call.name.clone(),
                    call.arguments_json.clone(),
                ))
                .map_err(|_| CoordinatorError::DurableActionInvalid)?;
        }
        let assistant_text = response.assistant_text.clone();
        let prepared_outcome = self.loop_engine.complete_prepared(prepared, response)?;
        let model_completed_at = self.durable_timestamp()?;
        let completion_input = |assistant_text| EventAppend {
            turn_id: Some(input.turn_id.clone()),
            step_id: Some(input.step_id.clone()),
            call_id: Some(input.model_call_id.clone()),
            occurred_at: model_completed_at.clone(),
            kind: HarnessEventKind::ModelCompleted { assistant_text },
        };

        match prepared_outcome {
            PreparedOutcome::Final(result) => {
                let durable_text = self.store.append_model_completed(
                    &input.owner_actor_id,
                    &input.run_id,
                    completion_input(assistant_text),
                    response_usage,
                )?;
                Ok(CoordinatorOutcome::Text {
                    text: durable_text,
                    model_calls: result.model_calls(),
                    usage: result.usage(),
                })
            }
            PreparedOutcome::Pending(pending) => {
                let effect_class = PolicyEngine::new()
                    .evaluate(pending.action())
                    .map_err(|_| CoordinatorError::Policy)?
                    .effect_class();
                let action = pending.action().clone();
                let record = ActionRecord {
                    action_id: input.action_id.clone(),
                    run_id: input.run_id.clone(),
                    step_id: input.step_id.clone(),
                    call_id: pending.call_id().clone(),
                    origin_model_call_id: input.model_call_id.clone(),
                    action_hash: action_hash(&action)?,
                    effect_class,
                    action,
                    occurred_at: model_completed_at.clone(),
                };
                self.store.append_model_completed_with_action(
                    &input.owner_actor_id,
                    &input.run_id,
                    completion_input(assistant_text),
                    record,
                    response_usage,
                )?;
                let policy = self.store.decide_policy(
                    &input.owner_actor_id,
                    &input.run_id,
                    &input.action_id,
                    self.durable_timestamp()?,
                )?;
                Ok(CoordinatorOutcome::Action {
                    action_id: input.action_id,
                    call_id: pending.call_id().clone(),
                    action: pending.action().clone(),
                    policy,
                    model_calls: pending.model_calls(),
                    usage: pending.usage(),
                })
            }
        }
    }

    fn validate_continuation_state(
        &self,
        input: &CoordinatorContinuationInput,
        state: &CoordinatorContinuationState,
    ) -> Result<(), CoordinatorError> {
        if state.run.run_id != input.run_id
            || state.run.owner_actor_id != input.owner_actor_id
            || state.run.steps_used == 0
            || state.run.steps_used > state.run.max_steps
            || state.run.max_steps != u64::from(self.loop_engine.max_steps())
            || state.run.config_snapshot_hash != self.loop_engine.config_snapshot_hash()
            || state.run.policy_snapshot_hash != PolicyEngine::snapshot_hash()
            || self.store.secret_set_id() != self.loop_engine.secret_set_id()
            || state.transcript.is_empty()
        {
            return Err(CoordinatorError::DependencyMismatch);
        }
        if !self
            .loop_engine
            .is_durable_field(&state.turn_id.0, MAX_COORDINATOR_ID_BYTES)
            || state
                .transcript
                .iter()
                .any(|record| self.loop_engine.validate_durable_record(record).is_err())
        {
            return Err(CoordinatorError::InvalidInput);
        }
        Ok(())
    }

    fn validate_continuation_input(
        &self,
        input: &CoordinatorContinuationInput,
    ) -> Result<(), CoordinatorError> {
        for value in [
            &input.run_id.0,
            &input.owner_actor_id,
            &input.step_id.0,
            &input.model_call_id.0,
            &input.action_id.0,
        ] {
            if !self
                .loop_engine
                .is_durable_field(value, MAX_COORDINATOR_ID_BYTES)
            {
                return Err(CoordinatorError::InvalidInput);
            }
        }
        Ok(())
    }

    fn validate_dependencies(&self, input: &CoordinatorInput) -> Result<(), CoordinatorError> {
        if input.run.max_steps != u64::from(self.loop_engine.max_steps())
            || input.run.config_snapshot_hash != self.loop_engine.config_snapshot_hash()
            || input.run.policy_snapshot_hash != PolicyEngine::snapshot_hash()
            || self.store.secret_set_id() != self.loop_engine.secret_set_id()
        {
            return Err(CoordinatorError::DependencyMismatch);
        }
        Ok(())
    }

    fn validate_input(&self, input: &CoordinatorInput) -> Result<(), CoordinatorError> {
        if input.prompt.len() > TranscriptLimits::DEFAULT_MAX_TEXT_BYTES {
            return Err(CoordinatorError::DurableTextTooLarge);
        }
        for value in [
            &input.run.run_id.0,
            &input.run.project_id,
            &input.run.owner_actor_id,
            &input.turn_id.0,
            &input.step_id.0,
            &input.model_call_id.0,
            &input.action_id.0,
        ] {
            if !self
                .loop_engine
                .is_durable_field(value, MAX_COORDINATOR_ID_BYTES)
            {
                return Err(CoordinatorError::InvalidInput);
            }
        }
        for value in [
            &input.run.workspace_identity,
            &input.run.policy_snapshot_hash,
            &input.run.config_snapshot_hash,
        ] {
            if !self
                .loop_engine
                .is_durable_field(value, MAX_COORDINATOR_FIELD_BYTES)
            {
                return Err(CoordinatorError::InvalidInput);
            }
        }
        if !self
            .loop_engine
            .is_durable_field(&input.run.canonical_root, MAX_COORDINATOR_PATH_BYTES)
        {
            return Err(CoordinatorError::InvalidInput);
        }
        Ok(())
    }

    fn durable_timestamp(&self) -> Result<String, CoordinatorError> {
        let timestamp = self.clock.now();
        if !self
            .loop_engine
            .is_durable_field(&timestamp, MAX_COORDINATOR_FIELD_BYTES)
        {
            return Err(CoordinatorError::InvalidInput);
        }
        Ok(timestamp)
    }
}
