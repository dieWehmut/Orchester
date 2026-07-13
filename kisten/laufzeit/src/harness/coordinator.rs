//! Durable boundary for one self-agent model step.
//!
//! The coordinator deliberately stops after recording a model response and,
//! when present, its decoded action.  Governance, permit-bound execution, and
//! validator-gated completion are subsequent phases.  Keeping this boundary
//! narrow makes the crash point explicit: a provider is never called until the
//! durable store has accepted `model.started`.

use orchester_modell::{LanguageModel, ModelError, ModelUsage};
use orchester_protokoll::{ActionId, AgentAction, CallId, HarnessEventKind, RunId, StepId, TurnId};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::agent_loop::{AgentLoopError, PreparedOutcome, SelfAgentLoop};
use super::feedback::SecretSetId;
use super::governance::PolicyEngine;
use super::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, RunStore, SqliteRunStore, StoreError,
    Transition,
};
use super::transcript::TranscriptLimits;
use super::transcript::TranscriptRecord;

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
                model_calls,
                usage,
            } => formatter
                .debug_struct("Action")
                .field("action_summary", &action.action_summary())
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
    ) -> Result<String, StoreError>;

    fn append_model_completed_with_action(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        action: ActionRecord,
    ) -> Result<(), StoreError>;
}

impl CoordinatorStore for SqliteRunStore {
    fn secret_set_id(&self) -> SecretSetId {
        SqliteRunStore::secret_set_id(self)
    }

    fn create_run(&self, input: NewRun) -> Result<(), StoreError> {
        <Self as RunStore>::create_run(self, input).map(|_| ())
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
    ) -> Result<String, StoreError> {
        let event = <Self as RunStore>::append_event(self, owner_actor_id, run_id, input)?;
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
    ) -> Result<(), StoreError> {
        SqliteRunStore::append_model_completed_with_action(
            self,
            owner_actor_id,
            run_id,
            input,
            action,
        )
        .map(|_| ())
    }
}

/// Drives exactly one durable model call for a newly created run.  Resume and
/// subsequent-step entry points are added separately so neither path can
/// accidentally recreate an existing run.
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

        // The store boundary above is the last fallible operation before the
        // provider call.  If the provider or later persistence fails, resume
        // projection conservatively reports ReconcileModelCall.
        let response = self
            .loop_engine
            .model()
            .complete(prepared.request().clone(), cancel)
            .await?;
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
                    &owner,
                    &run_id,
                    completion_input(assistant_text),
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
                    run_id: run_id.clone(),
                    step_id: input.step_id.clone(),
                    call_id: pending.call_id().clone(),
                    origin_model_call_id: input.model_call_id.clone(),
                    action_hash: action_hash(&action)?,
                    effect_class,
                    action,
                    occurred_at: model_completed_at.clone(),
                };
                self.store.append_model_completed_with_action(
                    &owner,
                    &run_id,
                    completion_input(assistant_text),
                    record,
                )?;
                Ok(CoordinatorOutcome::Action {
                    action_id: input.action_id,
                    call_id: pending.call_id().clone(),
                    action: pending.action().clone(),
                    model_calls: pending.model_calls(),
                    usage: pending.usage(),
                })
            }
        }
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
