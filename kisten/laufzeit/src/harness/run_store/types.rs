use orchester_protokoll::{
    ActionId, AgentAction, CallId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
    StopReason, TurnId,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("run was not found")]
    NotFound,
    #[error("run-store invariant failed: {0}")]
    Invariant(String),
    #[error("run-store data is corrupt")]
    Corrupt,
    #[error("run-store lock is poisoned")]
    LockPoisoned,
    #[error("run-store database operation failed")]
    Database(#[source] rusqlite::Error),
    #[error("run-store event serialization failed")]
    Serialization(#[source] serde_json::Error),
    #[error("run-store filesystem operation failed")]
    Io(#[source] std::io::Error),
    #[error("run-store path does not have user-only permissions")]
    InsecurePermissions,
    #[error("approval was not found")]
    ApprovalNotFound,
    #[error("approval operation is not authorized")]
    ApprovalUnauthorized,
    #[error("approval has expired")]
    ApprovalExpired,
    #[error("approval binding no longer matches")]
    ApprovalBindingMismatch,
    #[error("approval is in an invalid state")]
    ApprovalInvalidState,
    #[error("approval requires a durable audit checkpoint")]
    ApprovalAuditRequired,
    #[error("approval capability is invalid")]
    ApprovalNonceMismatch,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Created,
    Running,
    AwaitingApproval,
    Validating,
    Succeeded,
    Failed,
    Cancelled,
    BudgetExceeded,
    RepeatedFailure,
    InterruptedUnknownOutcome,
}

impl RunStatus {
    pub(super) fn as_db(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Validating => "validating",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::BudgetExceeded => "budget_exceeded",
            Self::RepeatedFailure => "repeated_failure",
            Self::InterruptedUnknownOutcome => "interrupted_unknown_outcome",
        }
    }

    pub(super) fn from_db(value: &str) -> Result<Self, StoreError> {
        match value {
            "created" => Ok(Self::Created),
            "running" => Ok(Self::Running),
            "awaiting_approval" => Ok(Self::AwaitingApproval),
            "validating" => Ok(Self::Validating),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "budget_exceeded" => Ok(Self::BudgetExceeded),
            "repeated_failure" => Ok(Self::RepeatedFailure),
            "interrupted_unknown_outcome" => Ok(Self::InterruptedUnknownOutcome),
            _ => Err(StoreError::Corrupt),
        }
    }

    pub(super) fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Failed
                | Self::Cancelled
                | Self::BudgetExceeded
                | Self::RepeatedFailure
                | Self::InterruptedUnknownOutcome
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRun {
    pub run_id: RunId,
    pub project_id: String,
    pub owner_actor_id: String,
    pub canonical_root: String,
    pub workspace_identity: String,
    pub policy_snapshot_hash: String,
    pub config_snapshot_hash: String,
    pub max_steps: u64,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSnapshot {
    pub run_id: RunId,
    pub project_id: String,
    pub owner_actor_id: String,
    pub status: RunStatus,
    pub next_sequence: u64,
    pub current_turn_id: Option<TurnId>,
    pub current_step_id: Option<StepId>,
    pub mutation_generation: u64,
    pub policy_snapshot_hash: String,
    pub config_snapshot_hash: String,
    pub max_steps: u64,
    pub steps_used: u64,
    pub row_version: u64,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    StartStep {
        turn_id: TurnId,
        step_id: StepId,
        occurred_at: String,
    },
    Complete {
        reason: StopReason,
        summary: String,
        occurred_at: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EffectClass {
    ReadOnlyIdempotent,
    WorkspaceMutation,
    MayMutate,
    ExternalEffect,
}

impl EffectClass {
    pub(super) fn as_db(self) -> &'static str {
        match self {
            Self::ReadOnlyIdempotent => "read_only_idempotent",
            Self::WorkspaceMutation => "workspace_mutation",
            Self::MayMutate => "may_mutate",
            Self::ExternalEffect => "external_effect",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActionRecord {
    pub action_id: ActionId,
    pub run_id: RunId,
    pub step_id: StepId,
    pub call_id: CallId,
    pub origin_model_call_id: CallId,
    pub action: AgentAction,
    pub action_hash: String,
    pub effect_class: EffectClass,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventAppend {
    pub turn_id: Option<TurnId>,
    pub step_id: Option<StepId>,
    pub call_id: Option<CallId>,
    pub occurred_at: String,
    pub kind: HarnessEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditCheckpoint {
    pub audit_file: String,
    pub audit_sequence: u64,
    pub head_hash: String,
    pub synced_at: String,
}

pub trait RunStore: Send + Sync {
    fn create_run(&self, input: NewRun) -> Result<RunSnapshot, StoreError>;
    fn append_transition(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        transition: Transition,
    ) -> Result<HarnessEvent, StoreError>;
    fn record_action(
        &self,
        owner_actor_id: &str,
        action: ActionRecord,
    ) -> Result<HarnessEvent, StoreError>;
    fn append_event(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
    ) -> Result<HarnessEvent, StoreError>;
    fn load_run_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<RunSnapshot, StoreError>;
    fn events_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<Vec<HarnessEvent>, StoreError>;
    fn mark_audit_checkpoint(
        &self,
        owner_actor_id: &str,
        event_id: &EventId,
        checkpoint: AuditCheckpoint,
    ) -> Result<(), StoreError>;
}
