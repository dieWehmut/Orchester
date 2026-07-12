//! Transactional source of truth for self-harness runs.
//!
//! Every state transition and its public event are committed in one
//! `BEGIN IMMEDIATE` transaction.  The store never infers progress from logs:
//! callers can close the process, reopen the database, and resume from the
//! exact persisted snapshot.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use orchester_protokoll::{
    ActionId, AgentAction, CallId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
    StopReason, TurnId, HARNESS_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::harness::approval::{
    ApprovalBinding, ApprovalRequestInput, ApprovalSnapshot, ApprovalState,
};
use crate::harness::audit::{AuditInput, AuditReceipt};
use crate::harness::barrier::{ExecutionPermit, StartedTool};

const MIGRATION_V1: &str = include_str!("../../migrations/0001_state.sql");
const MIGRATION_V2: &str = include_str!("../../migrations/0002_approval_barrier.sql");
const MIGRATION_V3: &str = include_str!("../../migrations/0003_model_phase.sql");
const CURRENT_SCHEMA_VERSION: u32 = 3;

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
    fn as_db(self) -> &'static str {
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

    fn from_db(value: &str) -> Result<Self, StoreError> {
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

    fn is_terminal(self) -> bool {
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
    fn as_db(self) -> &'static str {
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

pub struct SqliteRunStore {
    connection: Mutex<Connection>,
}

impl SqliteRunStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            let created = !parent.exists();
            std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
            ensure_private_state_dir(parent, created)?;
        }
        let file_existed = path.exists();
        let wal_existed = state_sidecar(path, "-wal").exists();
        let shm_existed = state_sidecar(path, "-shm").exists();
        let connection = Connection::open(path)?;
        ensure_private_state_file(path, !file_existed)?;
        let store = Self::initialize(connection, true)?;
        ensure_private_state_sidecars(path, wal_existed, shm_existed)?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::initialize(Connection::open_in_memory()?, false)
    }

    fn initialize(mut connection: Connection, enable_wal: bool) -> Result<Self, StoreError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")?;
        apply_migrations(&mut connection)?;
        if enable_wal {
            enable_wal_mode(&connection)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.connection.lock().map_err(|_| StoreError::LockPoisoned)
    }

    pub fn foreign_key_violations(&self) -> Result<Vec<String>, StoreError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let rows = statement.query_map([], |row| {
            let table: String = row.get(0)?;
            let row_id: Option<i64> = row.get(1)?;
            let parent: String = row.get(2)?;
            let foreign_key: i64 = row.get(3)?;
            Ok(format!("{table}:{row_id:?}:{parent}:{foreign_key}"))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        let connection = self.connection()?;
        let version = connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        Ok(version)
    }

    pub(crate) fn persist_approval_request(
        &self,
        input: &ApprovalRequestInput,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load_snapshot(
            &transaction,
            &input.binding.run_id,
            Some(&input.owner_actor_id),
        )?;
        if run.status != RunStatus::AwaitingApproval {
            return Err(StoreError::Invariant(
                "approval request requires an awaiting run".into(),
            ));
        }
        let action_context = transaction
            .query_row(
                "SELECT a.action_hash, a.state, a.policy_decision, a.policy_rule_id,
                        r.policy_snapshot_hash, r.config_snapshot_hash,
                        p.workspace_identity
                 FROM actions a
                 JOIN runs r ON r.run_id = a.run_id
                 JOIN projects p ON p.project_id = r.project_id
                 WHERE a.action_id = ?1 AND a.run_id = ?2 AND r.owner_actor_id = ?3",
                params![
                    input.binding.action_id.0,
                    input.binding.run_id.0,
                    input.owner_actor_id
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((action_hash, state, decision, rule_id, policy_hash, config_hash, workspace)) =
            action_context
        else {
            return Err(StoreError::NotFound);
        };
        if state != "awaiting_approval"
            || decision.as_deref() != Some("ask")
            || action_hash != input.binding.action_hash
            || policy_hash != input.binding.policy_snapshot_hash
            || config_hash != input.binding.config_snapshot_hash
            || workspace != input.binding.workspace_identity
            || rule_id.as_deref() != Some(input.rule_id.as_str())
        {
            return Err(StoreError::Invariant(
                "approval request binding does not match the action".into(),
            ));
        }
        let request = input.protocol_request();
        let created_at = request.created_at.clone();
        let expires_at = request.expires_at.clone();
        let event = HarnessEvent {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id: event_id(&input.binding.run_id, run.next_sequence),
            run_id: input.binding.run_id.clone(),
            turn_id: run.current_turn_id.clone(),
            step_id: run.current_step_id.clone(),
            call_id: None,
            sequence: run.next_sequence,
            occurred_at: created_at.clone(),
            kind: HarnessEventKind::ApprovalRequested { request },
        };
        persist_event(&transaction, &event)?;
        transaction
            .execute(
                "INSERT INTO approvals(
                approval_id, run_id, action_id, owner_actor_id, state,
                action_hash, action_summary, workspace_identity,
                policy_snapshot_hash, config_snapshot_hash, risk, rule_id,
                created_at, created_at_unix, expires_at, expires_at_unix,
                approval_event_id, row_version
             ) VALUES(?1, ?2, ?3, ?4, 'awaiting', ?5, ?6, ?7, ?8, ?9,
                      ?10, ?11, ?12, ?13, ?14, ?15, ?16, 0)",
                params![
                    input.approval_id.0,
                    input.binding.run_id.0,
                    input.binding.action_id.0,
                    input.owner_actor_id,
                    input.binding.action_hash,
                    input.action_summary,
                    input.binding.workspace_identity,
                    input.binding.policy_snapshot_hash,
                    input.binding.config_snapshot_hash,
                    input.risk,
                    input.rule_id,
                    created_at,
                    input.created_at_unix,
                    expires_at,
                    input.expires_at_unix,
                    event.event_id.0,
                ],
            )
            .map_err(|error| map_constraint(error, "approval already exists or is invalid"))?;
        advance_run(&transaction, &run, &event.occurred_at)?;
        transaction.commit()?;
        drop(connection);
        self.load_approval(&input.approval_id, &input.owner_actor_id)
    }

    pub fn load_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let connection = self.connection()?;
        load_approval_row(&connection, approval_id, Some(owner_actor_id))
    }

    pub(crate) fn approve_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
        binding: &ApprovalBinding,
        capability_nonce_hash: String,
        now_unix: u64,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let approval = load_approval_row(&transaction, approval_id, Some(owner_actor_id))?;
        if approval.state != ApprovalState::Awaiting {
            return Err(StoreError::ApprovalInvalidState);
        }
        let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
        if now_unix >= approval.expires_at_unix {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "expired",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Expired,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_waiting_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            drop(connection);
            return Err(StoreError::ApprovalExpired);
        }
        if approval.binding != *binding
            || !approval_context_matches(
                &transaction,
                &approval,
                "awaiting_approval",
                RunStatus::AwaitingApproval,
            )?
        {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "invalidated",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Invalidated,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_waiting_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            drop(connection);
            return Err(StoreError::ApprovalBindingMismatch);
        }
        let event = append_approval_event(
            &transaction,
            &run,
            approval_id,
            "approved",
            &approval,
            now_unix,
        )?;
        update_approval_state(
            &transaction,
            approval_id,
            approval.row_version,
            ApprovalState::Approved,
            Some(&capability_nonce_hash),
            Some(owner_actor_id),
            Some(&event.event_id),
        )?;
        advance_run(&transaction, &run, &event.occurred_at)?;
        transaction.commit()?;
        drop(connection);
        self.load_approval(approval_id, owner_actor_id)
    }

    pub(crate) fn deny_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
        now_unix: u64,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let approval = load_approval_row(&transaction, approval_id, Some(owner_actor_id))?;
        if approval.state != ApprovalState::Awaiting {
            return Err(StoreError::ApprovalInvalidState);
        }
        let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
        if now_unix >= approval.expires_at_unix {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "expired",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Expired,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_waiting_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            drop(connection);
            return Err(StoreError::ApprovalExpired);
        }
        let event = append_approval_event(
            &transaction,
            &run,
            approval_id,
            "denied",
            &approval,
            now_unix,
        )?;
        update_approval_state(
            &transaction,
            approval_id,
            approval.row_version,
            ApprovalState::Denied,
            None,
            Some(owner_actor_id),
            Some(&event.event_id),
        )?;
        close_waiting_approval(&transaction, &approval)?;
        advance_run(&transaction, &run, &event.occurred_at)?;
        transaction.commit()?;
        drop(connection);
        self.load_approval(approval_id, owner_actor_id)
    }

    pub(crate) fn reissue_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
        binding: &ApprovalBinding,
        capability_nonce_hash: String,
        now_unix: u64,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let approval = load_approval_row(&transaction, approval_id, Some(owner_actor_id))?;
        let (action_state, run_status) = match approval.state {
            ApprovalState::Approved => ("awaiting_approval", RunStatus::AwaitingApproval),
            ApprovalState::Executing => ("ready", RunStatus::Running),
            _ => return Err(StoreError::ApprovalInvalidState),
        };
        let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
        if now_unix >= approval.expires_at_unix {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "expired",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Expired,
                None,
                None,
                Some(&event.event_id),
            )?;
            match approval.state {
                ApprovalState::Approved => close_waiting_approval(&transaction, &approval)?,
                ApprovalState::Executing => close_ready_approval(&transaction, &approval)?,
                _ => unreachable!(),
            }
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            return Err(StoreError::ApprovalExpired);
        }
        if approval.binding != *binding
            || !approval_context_matches(&transaction, &approval, action_state, run_status)?
        {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "invalidated",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Invalidated,
                None,
                None,
                Some(&event.event_id),
            )?;
            match approval.state {
                ApprovalState::Approved => close_waiting_approval(&transaction, &approval)?,
                ApprovalState::Executing => close_ready_approval(&transaction, &approval)?,
                _ => unreachable!(),
            }
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            return Err(StoreError::ApprovalBindingMismatch);
        }
        let event = append_approval_event(
            &transaction,
            &run,
            approval_id,
            "reissued",
            &approval,
            now_unix,
        )?;
        update_approval_state(
            &transaction,
            approval_id,
            approval.row_version,
            approval.state,
            Some(&capability_nonce_hash),
            Some(owner_actor_id),
            Some(&event.event_id),
        )?;
        advance_run(&transaction, &run, &event.occurred_at)?;
        transaction.commit()?;
        drop(connection);
        self.load_approval(approval_id, owner_actor_id)
    }

    pub(crate) fn consume_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
        binding: &ApprovalBinding,
        capability_nonce_hash: String,
        now_unix: u64,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let approval = load_approval_row(&transaction, approval_id, Some(owner_actor_id))?;
        if approval.state != ApprovalState::Approved {
            return Err(StoreError::ApprovalInvalidState);
        }
        let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
        if now_unix >= approval.expires_at_unix {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "expired",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Expired,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_waiting_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            drop(connection);
            return Err(StoreError::ApprovalExpired);
        }
        if approval.binding != *binding
            || !approval_context_matches(
                &transaction,
                &approval,
                "awaiting_approval",
                RunStatus::AwaitingApproval,
            )?
        {
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "invalidated",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Invalidated,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_waiting_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            drop(connection);
            return Err(StoreError::ApprovalBindingMismatch);
        }
        if approval.approval_event_id.as_ref().is_none()
            || !capability_hash_matches(&transaction, &approval, &capability_nonce_hash)?
        {
            return Err(StoreError::ApprovalNonceMismatch);
        }
        let checkpoint: Option<i64> = transaction
            .query_row(
                "SELECT a.audit_sequence FROM actions a
                 JOIN audit_checkpoints c ON c.event_id = a.audit_event_id
                 WHERE a.action_id = ?1 AND a.run_id = ?2",
                params![approval.action_id.0, approval.run_id.0],
                |row| row.get(0),
            )
            .optional()?;
        if checkpoint.is_none() {
            return Err(StoreError::ApprovalAuditRequired);
        }
        let event = append_approval_event(
            &transaction,
            &run,
            approval_id,
            "executing",
            &approval,
            now_unix,
        )?;
        update_approval_state(
            &transaction,
            approval_id,
            approval.row_version,
            ApprovalState::Executing,
            Some(&capability_nonce_hash),
            None,
            Some(&event.event_id),
        )?;
        let updated = transaction.execute(
            "UPDATE actions SET state = 'ready'
             WHERE action_id = ?1 AND run_id = ?2 AND state = 'awaiting_approval'",
            params![approval.action_id.0, approval.run_id.0],
        )?;
        ensure_single_update(updated)?;
        let step_updated = transaction.execute(
            "UPDATE steps SET status = 'action_recorded'
             WHERE step_id = ?1 AND status = 'awaiting_approval'",
            params![current_step_id(
                &transaction,
                &approval.run_id,
                &approval.action_id
            )?],
        )?;
        ensure_single_update(step_updated)?;
        let run_resumed = transaction.execute(
            "UPDATE runs SET status = 'running'
             WHERE run_id = ?1 AND status = 'awaiting_approval'",
            params![approval.run_id.0],
        )?;
        ensure_single_update(run_resumed)?;
        advance_run(&transaction, &run, &event.occurred_at)?;
        transaction.commit()?;
        drop(connection);
        self.load_approval(approval_id, owner_actor_id)
    }

    pub(crate) fn recover_execution_approval(
        &self,
        approval_id: &orchester_protokoll::ApprovalId,
        owner_actor_id: &str,
        binding: &ApprovalBinding,
        capability_nonce_hash: String,
        now_unix: u64,
    ) -> Result<ApprovalSnapshot, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let approval = load_approval_row(&transaction, approval_id, Some(owner_actor_id))?;
        if approval.state != ApprovalState::Executing {
            return Err(StoreError::ApprovalInvalidState);
        }
        if now_unix >= approval.expires_at_unix {
            let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "expired",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Expired,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_ready_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            return Err(StoreError::ApprovalExpired);
        }
        if approval.binding != *binding
            || !approval_context_matches(&transaction, &approval, "ready", RunStatus::Running)?
        {
            let run = load_snapshot(&transaction, &approval.run_id, Some(owner_actor_id))?;
            let event = append_approval_event(
                &transaction,
                &run,
                approval_id,
                "invalidated",
                &approval,
                now_unix,
            )?;
            update_approval_state(
                &transaction,
                approval_id,
                approval.row_version,
                ApprovalState::Invalidated,
                None,
                None,
                Some(&event.event_id),
            )?;
            close_ready_approval(&transaction, &approval)?;
            advance_run(&transaction, &run, &event.occurred_at)?;
            transaction.commit()?;
            return Err(StoreError::ApprovalBindingMismatch);
        }
        if !capability_hash_matches(&transaction, &approval, &capability_nonce_hash)? {
            return Err(StoreError::ApprovalNonceMismatch);
        }
        let checkpoint: Option<i64> = transaction
            .query_row(
                "SELECT a.audit_sequence FROM actions a
                 JOIN audit_checkpoints c ON c.event_id = a.audit_event_id
                 WHERE a.action_id = ?1 AND a.run_id = ?2 AND a.state = 'ready'",
                params![approval.action_id.0, approval.run_id.0],
                |row| row.get(0),
            )
            .optional()?;
        if checkpoint.is_none() {
            return Err(StoreError::ApprovalAuditRequired);
        }
        transaction.commit()?;
        Ok(approval)
    }

    pub fn execution_candidate(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
    ) -> Result<ExecutionCandidate, StoreError> {
        let connection = self.connection()?;
        let row: Option<ExecutionCandidateRow> = connection
            .query_row(
                "SELECT a.run_id, a.action_id, a.action_hash, a.state, a.policy_decision,
                        a.policy_rule_id, a.policy_event_id, a.audit_event_id,
                        ap.approval_id, r.owner_actor_id
                 FROM actions a
                 JOIN runs r ON r.run_id = a.run_id
                 LEFT JOIN approvals ap ON ap.action_id = a.action_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2 AND r.owner_actor_id = ?3",
                params![run_id.0, action_id.0, owner_actor_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            row_run,
            row_action,
            action_hash,
            state,
            decision,
            rule,
            policy_event,
            audit_event,
            approval_id,
            owner,
        )) = row
        else {
            return Err(StoreError::NotFound);
        };
        let chosen_event =
            audit_event.or_else(|| if state == "ready" { policy_event } else { None });
        let event_id = if let Some(event) = chosen_event {
            event
        } else if matches!(
            state.as_str(),
            "awaiting_approval" | "approved" | "executing"
        ) {
            connection
                .query_row(
                    "SELECT approval_event_id FROM approvals
                 WHERE approval_id = ?1 AND state IN ('approved', 'executing')",
                    params![approval_id.as_deref().ok_or(StoreError::NotFound)?],
                    |row| row.get::<_, Option<String>>(0),
                )?
                .ok_or(StoreError::NotFound)?
        } else {
            return Err(StoreError::Invariant(
                "action has no policy or approval event candidate".into(),
            ));
        };
        let occurred_at: String = connection.query_row(
            "SELECT occurred_at FROM events WHERE event_id = ?1 AND run_id = ?2",
            params![event_id, row_run],
            |row| row.get(0),
        )?;
        let approved = approval_id.is_some();
        Ok(ExecutionCandidate {
            event_id: EventId::from(event_id),
            run_id: RunId::from(row_run),
            action_id: ActionId::from(row_action),
            action_hash,
            owner_actor_id: owner,
            approval_id,
            policy_rule: rule.unwrap_or_else(|| "unknown".into()),
            decision: if approved {
                "approved".into()
            } else {
                decision.unwrap_or_else(|| "allow".into())
            },
            occurred_at,
        })
    }

    pub fn mark_execution_checkpoint(
        &self,
        owner_actor_id: &str,
        candidate: &ExecutionCandidate,
        receipt: &AuditReceipt,
    ) -> Result<(), StoreError> {
        if receipt.event_id() != candidate.event_id.0
            || receipt.sequence() == 0
            || receipt.synced_at() != candidate.occurred_at
            || receipt.head_hash().len() != 64
            || !receipt
                .head_hash()
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StoreError::Invariant(
                "audit receipt does not match execution candidate".into(),
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owned: bool = transaction
            .query_row(
                "SELECT 1 FROM actions a JOIN runs r ON r.run_id = a.run_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2 AND r.owner_actor_id = ?3",
                params![candidate.run_id.0, candidate.action_id.0, owner_actor_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !owned {
            return Err(StoreError::NotFound);
        }
        let event_exists: bool = transaction
            .query_row(
                "SELECT 1 FROM events WHERE event_id = ?1 AND run_id = ?2",
                params![candidate.event_id.0, candidate.run_id.0],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !event_exists {
            return Err(StoreError::Invariant(
                "audit receipt references an unknown database event".into(),
            ));
        }
        let candidate_matches: bool = transaction
            .query_row(
                "SELECT 1 FROM actions a
                 LEFT JOIN approvals ap ON ap.action_id = a.action_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2
                   AND a.state IN ('ready', 'awaiting_approval')
                   AND (
                     a.audit_event_id = ?3
                     OR (a.audit_event_id IS NULL AND (
                       (a.policy_decision = 'allow' AND a.policy_event_id = ?3)
                       OR (a.policy_decision = 'ask'
                           AND ap.state = 'approved'
                           AND ap.approval_event_id = ?3)
                     ))
                   )",
                params![
                    candidate.run_id.0,
                    candidate.action_id.0,
                    candidate.event_id.0
                ],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !candidate_matches {
            return Err(StoreError::Invariant(
                "audit receipt does not match the current execution candidate".into(),
            ));
        }
        let existing: Option<(String, i64, String, String)> = transaction
            .query_row(
                "SELECT audit_file, audit_sequence, head_hash, synced_at
                 FROM audit_checkpoints WHERE event_id = ?1",
                params![candidate.event_id.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if let Some((file, sequence, head, synced)) = existing {
            if file != receipt.audit_file().to_string_lossy()
                || sequence != receipt.sequence() as i64
                || head != receipt.head_hash()
                || synced != receipt.synced_at()
            {
                return Err(StoreError::Invariant(
                    "audit checkpoint conflicts with existing durable record".into(),
                ));
            }
        } else {
            transaction.execute(
                "INSERT INTO audit_checkpoints(
                    event_id, audit_file, audit_sequence, head_hash, synced_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5)",
                params![
                    candidate.event_id.0,
                    receipt.audit_file().to_string_lossy().to_string(),
                    receipt.sequence(),
                    receipt.head_hash(),
                    receipt.synced_at(),
                ],
            )?;
        }
        let updated = transaction.execute(
            "UPDATE actions SET audit_event_id = ?1, audit_sequence = ?2
             WHERE run_id = ?3 AND action_id = ?4
               AND state IN ('ready', 'awaiting_approval')
               AND (audit_event_id IS NULL OR audit_event_id = ?1)",
            params![
                candidate.event_id.0,
                receipt.sequence(),
                candidate.run_id.0,
                candidate.action_id.0
            ],
        )?;
        ensure_single_update(updated)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn has_audit_checkpoint(&self, action_id: &ActionId) -> Result<bool, StoreError> {
        let connection = self.connection()?;
        let found: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM actions a JOIN audit_checkpoints c
                 ON c.event_id = a.audit_event_id WHERE a.action_id = ?1",
                params![action_id.0],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.is_some())
    }
}

/// A database-derived action/event binding used by the execution barrier.
/// Callers cannot substitute an arbitrary event ID or policy snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionCandidate {
    event_id: EventId,
    run_id: RunId,
    action_id: ActionId,
    action_hash: String,
    owner_actor_id: String,
    approval_id: Option<String>,
    policy_rule: String,
    decision: String,
    occurred_at: String,
}

type ExecutionCandidateRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

type ApprovalRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
    i64,
);

impl ExecutionCandidate {
    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn action_id(&self) -> &ActionId {
        &self.action_id
    }

    pub fn action_hash(&self) -> &str {
        &self.action_hash
    }

    pub fn approval_id(&self) -> Option<&str> {
        self.approval_id.as_deref()
    }

    pub fn audit_input(&self, _occurred_at: impl Into<String>) -> AuditInput {
        AuditInput {
            event_id: self.event_id.0.clone(),
            occurred_at: self.occurred_at.clone(),
            actor: self.owner_actor_id.clone(),
            run_id: self.run_id.0.clone(),
            action_id: Some(self.action_id.0.clone()),
            approval_id: self.approval_id.clone(),
            policy_rule: self.policy_rule.clone(),
            decision: self.decision.clone(),
            result_summary: "execution candidate".into(),
        }
    }
}

fn apply_migrations(connection: &mut Connection) -> Result<(), StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(MIGRATION_V1)?;
    transaction.commit()?;
    let version = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
        [],
        |row| row.get::<_, u32>(0),
    )?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::Invariant(
            "state database schema is newer than this binary".into(),
        ));
    }
    if version < 2 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let locked_version = transaction.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        if locked_version > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Invariant(
                "state database schema is newer than this binary".into(),
            ));
        }
        if locked_version < 2 {
            transaction.execute_batch(MIGRATION_V2)?;
        }
        transaction.commit()?;
    }
    let version = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
        [],
        |row| row.get::<_, u32>(0),
    )?;
    if version < 3 {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let locked_version = transaction.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get::<_, u32>(0),
        )?;
        if locked_version > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Invariant(
                "state database schema is newer than this binary".into(),
            ));
        }
        if locked_version < 3 {
            transaction.execute_batch(MIGRATION_V3)?;
        }
        transaction.commit()?;
    }
    let migrated = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
        [],
        |row| row.get::<_, u32>(0),
    )?;
    let user_version =
        connection.query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))?;
    if migrated != CURRENT_SCHEMA_VERSION || user_version != CURRENT_SCHEMA_VERSION {
        return Err(StoreError::Corrupt);
    }
    verify_schema_shape(connection)
}

fn enable_wal_mode(connection: &Connection) -> Result<(), StoreError> {
    let mut last_busy = None;
    for _ in 0..100 {
        match connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(mode) if mode.eq_ignore_ascii_case("wal") => return Ok(()),
            Ok(_) => {
                return Err(StoreError::Invariant(
                    "state database could not enter WAL mode".into(),
                ))
            }
            Err(error) if sqlite_is_busy(&error) => {
                last_busy = Some(error);
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(StoreError::Database(error)),
        }
    }
    Err(StoreError::Database(
        last_busy.unwrap_or_else(|| rusqlite::Error::InvalidQuery),
    ))
}

fn sqlite_is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(details, _)
            if matches!(
                details.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

fn verify_schema_shape(connection: &Connection) -> Result<(), StoreError> {
    let (count, minimum, maximum): (u32, Option<u32>, Option<u32>) = connection.query_row(
        "SELECT COUNT(*), MIN(version), MAX(version) FROM schema_versions",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if count != CURRENT_SCHEMA_VERSION
        || minimum != Some(1)
        || maximum != Some(CURRENT_SCHEMA_VERSION)
    {
        return Err(StoreError::Corrupt);
    }
    require_columns(
        connection,
        "actions",
        &["policy_event_id", "audit_event_id", "audit_sequence"],
    )?;
    require_model_phase_schema(connection)?;
    require_columns(
        connection,
        "approvals",
        &[
            "approval_id",
            "run_id",
            "action_id",
            "owner_actor_id",
            "state",
            "action_hash",
            "action_summary",
            "workspace_identity",
            "policy_snapshot_hash",
            "config_snapshot_hash",
            "created_at_unix",
            "expires_at_unix",
            "capability_nonce_hash",
            "approval_event_id",
            "row_version",
        ],
    )?;
    for index in [
        "idx_actions_policy_event",
        "idx_actions_audit_event",
        "idx_audit_file_sequence",
        "idx_approvals_run_owner",
        "idx_approvals_state_expiry",
        "idx_approvals_nonce_hash",
    ] {
        let present: bool = connection
            .query_row(
                "SELECT 1 FROM sqlite_schema WHERE type = 'index' AND name = ?1",
                params![index],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !present {
            return Err(StoreError::Corrupt);
        }
    }
    let mut foreign_keys = connection.prepare("PRAGMA foreign_key_check")?;
    if foreign_keys.query([])?.next()?.is_some() {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

fn require_columns(
    connection: &Connection,
    table: &str,
    required: &[&str],
) -> Result<(), StoreError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<HashSet<_>, _>>()?;
    if required.iter().all(|column| columns.contains(*column)) {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn require_model_phase_schema(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare("PRAGMA table_info(steps)")?;
    let mut rows = statement.query([])?;
    let mut column_is_valid = false;
    while let Some(row) = rows.next()? {
        let name = row.get::<_, String>(1)?;
        if name != "model_phase" {
            continue;
        }
        let declared_type = row.get::<_, String>(2)?;
        let not_null = row.get::<_, u32>(3)?;
        let default_value = row.get::<_, Option<String>>(4)?;
        column_is_valid = declared_type.eq_ignore_ascii_case("TEXT")
            && not_null == 1
            && default_value.as_deref() == Some("'not_started'");
        break;
    }
    if !column_is_valid {
        return Err(StoreError::Corrupt);
    }

    let schema_sql = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'steps'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let compact_schema = schema_sql
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if compact_schema.contains("check(model_phasein('not_started','running','completed'))") {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

#[cfg(unix)]
fn ensure_private_state_dir(path: &Path, created: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if created {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(StoreError::Io)?;
    }
    let mode = std::fs::metadata(path)
        .map_err(StoreError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if mode == 0o700 {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(windows)]
fn ensure_private_state_dir(_path: &Path, _created: bool) -> Result<(), StoreError> {
    if crate::harness::config::check_permissions(_path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
fn ensure_private_state_dir(_path: &Path, _created: bool) -> Result<(), StoreError> {
    Err(StoreError::InsecurePermissions)
}

#[cfg(unix)]
fn ensure_private_state_file(path: &Path, created: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if created {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(StoreError::Io)?;
    }
    let mode = std::fs::metadata(path)
        .map_err(StoreError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if mode == 0o600 {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(unix)]
fn ensure_private_state_sidecars(
    path: &Path,
    wal_existed: bool,
    shm_existed: bool,
) -> Result<(), StoreError> {
    for (suffix, existed) in [("-wal", wal_existed), ("-shm", shm_existed)] {
        let sidecar = state_sidecar(path, suffix);
        if sidecar.exists() {
            ensure_private_state_file(&sidecar, !existed)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_private_state_sidecars(
    _path: &Path,
    _wal_existed: bool,
    _shm_existed: bool,
) -> Result<(), StoreError> {
    Ok(())
}

fn state_sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

#[cfg(windows)]
fn ensure_private_state_file(_path: &Path, _created: bool) -> Result<(), StoreError> {
    if crate::harness::config::check_permissions(_path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
fn ensure_private_state_file(_path: &Path, _created: bool) -> Result<(), StoreError> {
    Err(StoreError::InsecurePermissions)
}

impl SqliteRunStore {
    fn append_event_internal(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        permit_event_id: Option<&EventId>,
        permit_action_hash: Option<&str>,
    ) -> Result<(HarnessEvent, Option<AgentAction>), StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        if snapshot.status.is_terminal() {
            return Err(StoreError::Invariant(
                "terminal run cannot append an event".into(),
            ));
        }
        let started_action = match (&input.kind, permit_action_hash) {
            (HarnessEventKind::ToolStarted { action_id }, Some(expected_hash)) => {
                let durable: Option<(String, String)> = transaction
                    .query_row(
                        "SELECT canonical_json, action_hash FROM actions
                         WHERE run_id = ?1 AND action_id = ?2",
                        params![run_id.0, action_id.0],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (canonical_json, stored_hash) = durable.ok_or(StoreError::NotFound)?;
                let action: AgentAction = serde_json::from_str(&canonical_json)?;
                if stored_hash != expected_hash || action_hash(&action)? != expected_hash {
                    return Err(StoreError::Invariant(
                        "durable action changed after execution authorization".into(),
                    ));
                }
                Some(action)
            }
            (HarnessEventKind::ToolStarted { .. }, None) => None,
            (_, Some(_)) => {
                return Err(StoreError::Invariant(
                    "execution action hash requires a tool-start event".into(),
                ))
            }
            (_, None) => None,
        };
        apply_event_transition(&transaction, &snapshot, &input, permit_event_id)?;
        let event = HarnessEvent {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id: event_id(run_id, snapshot.next_sequence),
            run_id: run_id.clone(),
            turn_id: input.turn_id,
            step_id: input.step_id,
            call_id: input.call_id,
            sequence: snapshot.next_sequence,
            occurred_at: input.occurred_at.clone(),
            kind: input.kind,
        };
        persist_event(&transaction, &event)?;
        let mut events_written = 1u64;
        if let HarnessEventKind::PolicyDecided { action_id, .. } = &event.kind {
            let updated = transaction.execute(
                "UPDATE actions SET policy_event_id = ?1
                 WHERE run_id = ?2 AND action_id = ?3",
                params![event.event_id.0, run_id.0, action_id.0],
            )?;
            ensure_single_update(updated)?;
        }
        if let HarnessEventKind::ToolStarted { action_id } = &event.kind {
            let approval_id: Option<String> = transaction
                .query_row(
                    "SELECT approval_id FROM approvals
                     WHERE action_id = ?1 AND state = 'consumed'",
                    params![action_id.0],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(approval_id) = approval_id {
                let resolution = HarnessEvent {
                    schema_version: HARNESS_SCHEMA_VERSION,
                    event_id: event_id(run_id, snapshot.next_sequence + 1),
                    run_id: run_id.clone(),
                    turn_id: event.turn_id.clone(),
                    step_id: event.step_id.clone(),
                    call_id: None,
                    sequence: snapshot.next_sequence + 1,
                    occurred_at: event.occurred_at.clone(),
                    kind: HarnessEventKind::ApprovalResolved {
                        approval_id: approval_id.clone().into(),
                        decision: "consumed".into(),
                    },
                };
                persist_event(&transaction, &resolution)?;
                let updated = transaction.execute(
                    "UPDATE approvals SET approval_event_id = ?1
                     WHERE approval_id = ?2 AND state = 'consumed'",
                    params![resolution.event_id.0, approval_id],
                )?;
                ensure_single_update(updated)?;
                events_written = 2;
            }
        }
        let updated = transaction.execute(
            "UPDATE runs SET next_sequence = ?1, row_version = row_version + 1,
               updated_at = ?2 WHERE run_id = ?3 AND row_version = ?4",
            params![
                snapshot.next_sequence + events_written,
                input.occurred_at,
                run_id.0,
                snapshot.row_version,
            ],
        )?;
        ensure_single_update(updated)?;
        transaction.commit()?;
        Ok((event, started_action))
    }

    pub(crate) fn append_tool_started_with_permit(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        permit: ExecutionPermit,
        input: EventAppend,
    ) -> Result<StartedTool, StoreError> {
        if !matches!(&input.kind, HarnessEventKind::ToolStarted { action_id: id } if id == permit.action_id())
        {
            return Err(StoreError::Invariant(
                "ExecutionPermit does not match tool start action".into(),
            ));
        }
        let (event, action) = self.append_event_internal(
            owner_actor_id,
            run_id,
            input,
            Some(permit.event_id()),
            Some(permit.action_hash()),
        )?;
        Ok(StartedTool::new(
            event,
            action.ok_or_else(|| StoreError::Invariant("tool start lost durable action".into()))?,
        ))
    }
}

impl RunStore for SqliteRunStore {
    fn create_run(&self, input: NewRun) -> Result<RunSnapshot, StoreError> {
        if input.max_steps == 0 {
            return Err(StoreError::Invariant(
                "max_steps must be greater than zero".into(),
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        transaction.execute(
            "INSERT OR IGNORE INTO actors(actor_id, kind, subject_hash, created_at) VALUES(?1, 'local_user', ?2, ?3)",
            params![input.owner_actor_id, input.owner_actor_id, input.occurred_at],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?4)",
            params![input.project_id, input.canonical_root, input.workspace_identity, input.occurred_at],
        )?;

        let inserted = transaction.execute(
            "INSERT INTO runs(
                run_id, project_id, owner_actor_id, status, next_sequence,
                policy_snapshot_hash, config_snapshot_hash, max_steps,
                created_at, updated_at
             ) VALUES(?1, ?2, ?3, 'created', 1, ?4, ?5, ?6, ?7, ?7)",
            params![
                input.run_id.0,
                input.project_id,
                input.owner_actor_id,
                input.policy_snapshot_hash,
                input.config_snapshot_hash,
                input.max_steps,
                input.occurred_at,
            ],
        );
        if let Err(error) = inserted {
            return Err(map_constraint(
                error,
                "run or project identity already exists",
            ));
        }

        let event = HarnessEvent {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id: event_id(&input.run_id, 1),
            run_id: input.run_id.clone(),
            turn_id: None,
            step_id: None,
            call_id: None,
            sequence: 1,
            occurred_at: input.occurred_at.clone(),
            kind: HarnessEventKind::RunCreated,
        };
        persist_event(&transaction, &event)?;
        let updated = transaction.execute(
            "UPDATE runs SET next_sequence = 2, row_version = 1 WHERE run_id = ?1 AND row_version = 0",
            params![input.run_id.0],
        )?;
        ensure_single_update(updated)?;
        transaction.commit()?;
        drop(connection);
        self.load_run_owned(&input.run_id, &input.owner_actor_id)
    }

    fn append_transition(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        transition: Transition,
    ) -> Result<HarnessEvent, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        if snapshot.status.is_terminal() {
            return Err(StoreError::Invariant(
                "terminal run cannot transition again".into(),
            ));
        }

        let event = match transition {
            Transition::StartStep {
                turn_id,
                step_id,
                occurred_at,
            } => {
                match snapshot.status {
                    RunStatus::Created if snapshot.current_step_id.is_none() => {}
                    RunStatus::Running => {
                        let status = current_step_status(&transaction, &snapshot)?;
                        if !matches!(
                            status.as_deref(),
                            Some("observed" | "completed" | "failed" | "cancelled")
                        ) {
                            return Err(StoreError::Invariant(
                                "current step must be terminal before starting another".into(),
                            ));
                        }
                    }
                    _ => {
                        return Err(StoreError::Invariant(
                            "run state does not allow a new step".into(),
                        ));
                    }
                }
                if snapshot.steps_used >= snapshot.max_steps {
                    return Err(StoreError::Invariant("step budget exhausted".into()));
                }
                let step_ordinal = snapshot.steps_used + 1;
                let inserted = transaction.execute(
                    "INSERT INTO steps(run_id, step_ordinal, step_id, turn_id, status, started_at)
                     VALUES(?1, ?2, ?3, ?4, 'created', ?5)",
                    params![run_id.0, step_ordinal, step_id.0, turn_id.0, occurred_at],
                );
                if let Err(error) = inserted {
                    return Err(map_constraint(error, "step identity already exists"));
                }

                let event = HarnessEvent {
                    schema_version: HARNESS_SCHEMA_VERSION,
                    event_id: event_id(run_id, snapshot.next_sequence),
                    run_id: run_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    step_id: Some(step_id.clone()),
                    call_id: None,
                    sequence: snapshot.next_sequence,
                    occurred_at: occurred_at.clone(),
                    kind: HarnessEventKind::StepStarted,
                };
                persist_event(&transaction, &event)?;
                let updated = transaction.execute(
                    "UPDATE runs SET status = 'running', current_turn_id = ?1,
                       current_step_id = ?2, steps_used = ?3, next_sequence = ?4,
                       row_version = row_version + 1, updated_at = ?5
                     WHERE run_id = ?6 AND row_version = ?7",
                    params![
                        turn_id.0,
                        step_id.0,
                        step_ordinal,
                        snapshot.next_sequence + 1,
                        occurred_at,
                        run_id.0,
                        snapshot.row_version,
                    ],
                )?;
                ensure_single_update(updated)?;
                event
            }
            Transition::Complete {
                reason,
                summary,
                occurred_at,
            } => {
                if snapshot.status == RunStatus::AwaitingApproval
                    && !matches!(reason, StopReason::Cancelled | StopReason::Failed)
                {
                    return Err(StoreError::Invariant(
                        "awaiting approval cannot be completed without a decision".into(),
                    ));
                }
                let status = status_for_stop(&reason);
                let kind = if reason == StopReason::AwaitingApproval {
                    HarnessEventKind::RunPaused {
                        reason: reason.clone(),
                    }
                } else {
                    HarnessEventKind::RunCompleted {
                        reason: reason.clone(),
                        summary,
                    }
                };
                let event = HarnessEvent {
                    schema_version: HARNESS_SCHEMA_VERSION,
                    event_id: event_id(run_id, snapshot.next_sequence),
                    run_id: run_id.clone(),
                    turn_id: snapshot.current_turn_id.clone(),
                    step_id: snapshot.current_step_id.clone(),
                    call_id: None,
                    sequence: snapshot.next_sequence,
                    occurred_at: occurred_at.clone(),
                    kind,
                };
                persist_event(&transaction, &event)?;
                let updated = transaction.execute(
                    "UPDATE runs SET status = ?1, stop_reason = ?2,
                       next_sequence = ?3, row_version = row_version + 1,
                       updated_at = ?4
                     WHERE run_id = ?5 AND row_version = ?6",
                    params![
                        status.as_db(),
                        stop_reason_name(&reason),
                        snapshot.next_sequence + 1,
                        occurred_at,
                        run_id.0,
                        snapshot.row_version,
                    ],
                )?;
                ensure_single_update(updated)?;
                event
            }
        };

        transaction.commit()?;
        Ok(event)
    }

    fn record_action(
        &self,
        owner_actor_id: &str,
        action: ActionRecord,
    ) -> Result<HarnessEvent, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, &action.run_id, Some(owner_actor_id))?;
        if snapshot.status != RunStatus::Running {
            return Err(StoreError::Invariant(
                "actions require a running run".into(),
            ));
        }
        if snapshot.current_step_id.as_ref() != Some(&action.step_id) {
            return Err(StoreError::Invariant(
                "action does not belong to the current step".into(),
            ));
        }
        let existing: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM actions WHERE step_id = ?1",
            params![action.step_id.0],
            |row| row.get(0),
        )?;
        if existing != 0 {
            return Err(StoreError::Invariant(
                "a step can record at most one action".into(),
            ));
        }

        let canonical_json = serde_json::to_string(&action.action)?;
        let expected_hash = hash_canonical_action(&canonical_json);
        if action.action_hash != expected_hash {
            return Err(StoreError::Invariant(
                "action hash does not match canonical action".into(),
            ));
        }
        if current_step_status(&transaction, &snapshot)?.as_deref() != Some("model_running") {
            return Err(StoreError::Invariant(
                "action requires a completed model call in the current step".into(),
            ));
        }
        let policy_effect = crate::harness::governance::PolicyEngine::new()
            .evaluate(&action.action)
            .map_err(|_| StoreError::Invariant("action policy classification failed".into()))?
            .effect;
        if action.effect_class != policy_effect {
            return Err(StoreError::Invariant(
                "action effect class does not match core policy classification".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO actions(
                action_id, run_id, step_id, call_id, kind, canonical_json,
                action_hash, effect_class, state, created_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'recorded', ?9)",
            params![
                action.action_id.0,
                action.run_id.0,
                action.step_id.0,
                action.call_id.0,
                action_kind(&action.action),
                canonical_json,
                action.action_hash,
                action.effect_class.as_db(),
                action.occurred_at,
            ],
        )?;

        let event = HarnessEvent {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id: event_id(&action.run_id, snapshot.next_sequence),
            run_id: action.run_id.clone(),
            turn_id: snapshot.current_turn_id.clone(),
            step_id: Some(action.step_id.clone()),
            call_id: Some(action.call_id.clone()),
            sequence: snapshot.next_sequence,
            occurred_at: action.occurred_at.clone(),
            kind: HarnessEventKind::ActionRecorded {
                action_id: action.action_id.clone(),
                action: action.action,
            },
        };
        persist_event(&transaction, &event)?;
        let step_updated = transaction.execute(
            "UPDATE steps SET status = 'action_recorded', action_id = ?1
             WHERE run_id = ?2 AND step_id = ?3 AND action_id IS NULL",
            params![action.action_id.0, action.run_id.0, action.step_id.0],
        )?;
        ensure_single_update(step_updated)?;
        let run_updated = transaction.execute(
            "UPDATE runs SET next_sequence = ?1, row_version = row_version + 1,
               updated_at = ?2 WHERE run_id = ?3 AND row_version = ?4",
            params![
                snapshot.next_sequence + 1,
                action.occurred_at,
                action.run_id.0,
                snapshot.row_version,
            ],
        )?;
        ensure_single_update(run_updated)?;
        transaction.commit()?;
        Ok(event)
    }

    fn append_event(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
    ) -> Result<HarnessEvent, StoreError> {
        if matches!(input.kind, HarnessEventKind::ToolStarted { .. }) {
            return Err(StoreError::Invariant(
                "tool start requires an ExecutionPermit".into(),
            ));
        }
        self.append_event_internal(owner_actor_id, run_id, input, None, None)
            .map(|(event, _)| event)
    }

    fn load_run_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<RunSnapshot, StoreError> {
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, Some(owner_actor_id))
    }

    fn events_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<Vec<HarnessEvent>, StoreError> {
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, Some(owner_actor_id))?;
        load_events(&connection, run_id)
    }

    fn mark_audit_checkpoint(
        &self,
        owner_actor_id: &str,
        event_id: &EventId,
        checkpoint: AuditCheckpoint,
    ) -> Result<(), StoreError> {
        let connection = self.connection()?;
        if checkpoint.audit_file.trim().is_empty()
            || checkpoint.audit_sequence == 0
            || checkpoint.head_hash.len() != 64
            || !checkpoint
                .head_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(StoreError::Invariant(
                "audit checkpoint is malformed".into(),
            ));
        }
        let event_owned: bool = connection
            .query_row(
                "SELECT 1 FROM events e JOIN runs r ON r.run_id = e.run_id
                 WHERE e.event_id = ?1 AND r.owner_actor_id = ?2",
                params![event_id.0, owner_actor_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !event_owned {
            return Err(StoreError::NotFound);
        }
        let inserted = connection.execute(
            "INSERT INTO audit_checkpoints(
                event_id, audit_file, audit_sequence, head_hash, synced_at
             ) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![
                event_id.0,
                checkpoint.audit_file,
                checkpoint.audit_sequence,
                checkpoint.head_hash,
                checkpoint.synced_at,
            ],
        );
        inserted
            .map(|_| ())
            .map_err(|error| map_constraint(error, "audit checkpoint already exists"))
    }
}

fn load_snapshot(
    connection: &Connection,
    run_id: &RunId,
    owner_actor_id: Option<&str>,
) -> Result<RunSnapshot, StoreError> {
    let columns = "run_id, project_id, owner_actor_id, status, next_sequence,
        current_turn_id, current_step_id, mutation_generation,
        policy_snapshot_hash, config_snapshot_hash, max_steps, steps_used,
        row_version, stop_reason";
    let row = if let Some(owner) = owner_actor_id {
        connection
            .query_row(
                &format!("SELECT {columns} FROM runs WHERE run_id = ?1 AND owner_actor_id = ?2"),
                params![run_id.0, owner],
                snapshot_from_row,
            )
            .optional()?
    } else {
        connection
            .query_row(
                &format!("SELECT {columns} FROM runs WHERE run_id = ?1"),
                params![run_id.0],
                snapshot_from_row,
            )
            .optional()?
    };
    row.ok_or(StoreError::NotFound)?
}

fn load_approval_row(
    connection: &Connection,
    approval_id: &orchester_protokoll::ApprovalId,
    owner_actor_id: Option<&str>,
) -> Result<ApprovalSnapshot, StoreError> {
    let row: Option<ApprovalRow> = if let Some(owner) = owner_actor_id {
        connection
            .query_row(
                "SELECT approval_id, run_id, action_id, owner_actor_id, state,
                        action_hash, action_summary, workspace_identity,
                        policy_snapshot_hash, config_snapshot_hash, risk, rule_id,
                        created_at_unix, expires_at_unix, approval_event_id,
                        row_version
                 FROM approvals WHERE approval_id = ?1 AND owner_actor_id = ?2",
                params![approval_id.0, owner],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                        row.get(14)?,
                        row.get(15)?,
                    ))
                },
            )
            .optional()?
    } else {
        connection
            .query_row(
                "SELECT approval_id, run_id, action_id, owner_actor_id, state,
                        action_hash, action_summary, workspace_identity,
                        policy_snapshot_hash, config_snapshot_hash, risk, rule_id,
                        created_at_unix, expires_at_unix, approval_event_id,
                        row_version
                 FROM approvals WHERE approval_id = ?1",
                params![approval_id.0],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                        row.get(14)?,
                        row.get(15)?,
                    ))
                },
            )
            .optional()?
    };
    let Some((
        approval_id,
        run_id,
        action_id,
        owner_actor_id,
        state,
        action_hash,
        action_summary,
        workspace_identity,
        policy_snapshot_hash,
        config_snapshot_hash,
        risk,
        rule_id,
        created_at_unix,
        expires_at_unix,
        approval_event_id,
        row_version,
    )) = row
    else {
        return Err(StoreError::NotFound);
    };
    Ok(ApprovalSnapshot {
        approval_id: orchester_protokoll::ApprovalId::from(approval_id),
        run_id: RunId::from(run_id.clone()),
        action_id: ActionId::from(action_id.clone()),
        owner_actor_id,
        state: ApprovalState::from_db(&state).map_err(|_| StoreError::Corrupt)?,
        binding: ApprovalBinding {
            run_id: RunId::from(run_id),
            action_id: ActionId::from(action_id),
            action_hash,
            workspace_identity,
            policy_snapshot_hash,
            config_snapshot_hash,
        },
        action_summary,
        risk,
        rule_id,
        created_at_unix: u64::try_from(created_at_unix).map_err(|_| StoreError::Corrupt)?,
        expires_at_unix: u64::try_from(expires_at_unix).map_err(|_| StoreError::Corrupt)?,
        approval_event_id: approval_event_id.map(EventId::from),
        row_version: u64::try_from(row_version).map_err(|_| StoreError::Corrupt)?,
    })
}

fn advance_run(
    transaction: &Transaction<'_>,
    snapshot: &RunSnapshot,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE runs SET next_sequence = ?1, row_version = row_version + 1,
                updated_at = ?2 WHERE run_id = ?3 AND row_version = ?4",
        params![
            snapshot.next_sequence + 1,
            occurred_at,
            snapshot.run_id.0,
            snapshot.row_version
        ],
    )?;
    ensure_single_update(updated)
}

fn append_approval_event(
    transaction: &Transaction<'_>,
    run: &RunSnapshot,
    approval_id: &orchester_protokoll::ApprovalId,
    decision: &str,
    _approval: &ApprovalSnapshot,
    now_unix: u64,
) -> Result<HarnessEvent, StoreError> {
    let event = HarnessEvent {
        schema_version: HARNESS_SCHEMA_VERSION,
        event_id: event_id(&run.run_id, run.next_sequence),
        run_id: run.run_id.clone(),
        turn_id: run.current_turn_id.clone(),
        step_id: run.current_step_id.clone(),
        call_id: None,
        sequence: run.next_sequence,
        occurred_at: format!("unix:{now_unix}"),
        kind: HarnessEventKind::ApprovalResolved {
            approval_id: approval_id.clone(),
            decision: decision.to_owned(),
        },
    };
    persist_event(transaction, &event)?;
    Ok(event)
}

fn update_approval_state(
    transaction: &Transaction<'_>,
    approval_id: &orchester_protokoll::ApprovalId,
    row_version: u64,
    state: ApprovalState,
    capability_nonce_hash: Option<&str>,
    decided_by_actor_id: Option<&str>,
    approval_event_id: Option<&EventId>,
) -> Result<(), StoreError> {
    let updated = transaction.execute(
        "UPDATE approvals SET state = ?1, capability_nonce_hash = ?2,
                decided_by_actor_id = COALESCE(?3, decided_by_actor_id),
                approval_event_id = ?4,
                decided_at = CURRENT_TIMESTAMP,
                row_version = row_version + 1
         WHERE approval_id = ?5 AND row_version = ?6",
        params![
            state.as_db(),
            capability_nonce_hash,
            decided_by_actor_id,
            approval_event_id.map(|id| id.0.as_str()),
            approval_id.0,
            row_version,
        ],
    )?;
    ensure_single_update(updated)
}

fn capability_hash_matches(
    transaction: &Transaction<'_>,
    approval: &ApprovalSnapshot,
    expected: &str,
) -> Result<bool, StoreError> {
    let stored: Option<String> = transaction
        .query_row(
            "SELECT capability_nonce_hash FROM approvals WHERE approval_id = ?1",
            params![approval.approval_id.0],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(stored.as_deref() == Some(expected))
}

fn approval_context_matches(
    transaction: &Transaction<'_>,
    approval: &ApprovalSnapshot,
    action_state: &str,
    run_status: RunStatus,
) -> Result<bool, StoreError> {
    let canonical_json: Option<String> = transaction
        .query_row(
            "SELECT a.canonical_json FROM actions a
             JOIN runs r ON r.run_id = a.run_id
             JOIN projects p ON p.project_id = r.project_id
             WHERE a.run_id = ?1 AND a.action_id = ?2
               AND r.owner_actor_id = ?3 AND a.action_hash = ?4
               AND p.workspace_identity = ?5
               AND r.policy_snapshot_hash = ?6
               AND r.config_snapshot_hash = ?7
               AND a.policy_decision = 'ask' AND a.policy_rule_id = ?8
               AND a.state = ?9 AND r.status = ?10",
            params![
                approval.run_id.0,
                approval.action_id.0,
                approval.owner_actor_id,
                approval.binding.action_hash,
                approval.binding.workspace_identity,
                approval.binding.policy_snapshot_hash,
                approval.binding.config_snapshot_hash,
                approval.rule_id,
                action_state,
                run_status.as_db(),
            ],
            |row| row.get(0),
        )
        .optional()?;
    Ok(canonical_json
        .as_deref()
        .map(|json| hash_canonical_action(json) == approval.binding.action_hash)
        .unwrap_or(false))
}

fn current_step_id(
    transaction: &Transaction<'_>,
    run_id: &RunId,
    action_id: &ActionId,
) -> Result<String, StoreError> {
    transaction
        .query_row(
            "SELECT step_id FROM actions WHERE run_id = ?1 AND action_id = ?2",
            params![run_id.0, action_id.0],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound)
}

fn close_waiting_approval(
    transaction: &Transaction<'_>,
    approval: &ApprovalSnapshot,
) -> Result<(), StoreError> {
    let action_updated = transaction.execute(
        "UPDATE actions SET state = 'denied'
         WHERE action_id = ?1 AND run_id = ?2 AND state = 'awaiting_approval'",
        params![approval.action_id.0, approval.run_id.0],
    )?;
    ensure_single_update(action_updated)?;
    let step_updated = transaction.execute(
        "UPDATE steps SET status = 'observed'
         WHERE step_id = ?1 AND status = 'awaiting_approval'",
        params![current_step_id(
            transaction,
            &approval.run_id,
            &approval.action_id
        )?],
    )?;
    ensure_single_update(step_updated)?;
    let run_resumed = transaction.execute(
        "UPDATE runs SET status = 'running'
         WHERE run_id = ?1 AND status = 'awaiting_approval'",
        params![approval.run_id.0],
    )?;
    ensure_single_update(run_resumed)
}

fn close_ready_approval(
    transaction: &Transaction<'_>,
    approval: &ApprovalSnapshot,
) -> Result<(), StoreError> {
    let action_updated = transaction.execute(
        "UPDATE actions SET state = 'denied'
         WHERE action_id = ?1 AND run_id = ?2 AND state = 'ready'",
        params![approval.action_id.0, approval.run_id.0],
    )?;
    ensure_single_update(action_updated)?;
    let step_updated = transaction.execute(
        "UPDATE steps SET status = 'observed'
         WHERE step_id = ?1 AND status = 'action_recorded'",
        params![current_step_id(
            transaction,
            &approval.run_id,
            &approval.action_id
        )?],
    )?;
    ensure_single_update(step_updated)
}

fn snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<RunSnapshot, StoreError>> {
    let status: String = row.get(3)?;
    let run_id = RunId::from(row.get::<_, String>(0)?);
    let project_id = row.get(1)?;
    let owner_actor_id = row.get(2)?;
    let next_sequence = row.get(4)?;
    let current_turn_id = row.get::<_, Option<String>>(5)?.map(TurnId::from);
    let current_step_id = row.get::<_, Option<String>>(6)?.map(StepId::from);
    let mutation_generation = row.get(7)?;
    let policy_snapshot_hash = row.get(8)?;
    let config_snapshot_hash = row.get(9)?;
    let max_steps = row.get(10)?;
    let steps_used = row.get(11)?;
    let row_version = row.get(12)?;
    let stop_reason = row.get(13)?;
    Ok(RunStatus::from_db(&status).map(|status| RunSnapshot {
        run_id,
        project_id,
        owner_actor_id,
        status,
        next_sequence,
        current_turn_id,
        current_step_id,
        mutation_generation,
        policy_snapshot_hash,
        config_snapshot_hash,
        max_steps,
        steps_used,
        row_version,
        stop_reason,
    }))
}

fn load_events(connection: &Connection, run_id: &RunId) -> Result<Vec<HarnessEvent>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT schema_version, event_id, turn_id, step_id, call_id, sequence,
                occurred_at, kind, sanitized_payload
         FROM events WHERE run_id = ?1 ORDER BY sequence",
    )?;
    let rows = statement.query_map(params![run_id.0], |row| {
        Ok((
            row.get::<_, u16>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, u64>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let mut events = Vec::new();
    for row in rows {
        let (schema, event_id, turn_id, step_id, call_id, sequence, occurred_at, kind, payload) =
            row?;
        let payload: Value = serde_json::from_str(&payload)?;
        let value = json!({
            "schema_version": schema,
            "event_id": event_id,
            "run_id": run_id.0,
            "turn_id": turn_id,
            "step_id": step_id,
            "call_id": call_id,
            "sequence": sequence,
            "occurred_at": occurred_at,
            "kind": kind,
            "payload": payload,
        });
        events.push(serde_json::from_value(value)?);
    }
    Ok(events)
}

fn persist_event(transaction: &Transaction<'_>, event: &HarnessEvent) -> Result<(), StoreError> {
    let encoded = serde_json::to_value(event)?;
    let kind = encoded
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(StoreError::Corrupt)?;
    let payload = encoded.get("payload").ok_or(StoreError::Corrupt)?;
    transaction.execute(
        "INSERT INTO events(
            run_id, sequence, schema_version, event_id, turn_id, step_id,
            call_id, kind, sanitized_payload, occurred_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            event.run_id.0,
            event.sequence,
            event.schema_version,
            event.event_id.0,
            event.turn_id.as_ref().map(|id| id.0.as_str()),
            event.step_id.as_ref().map(|id| id.0.as_str()),
            event.call_id.as_ref().map(|id| id.0.as_str()),
            kind,
            serde_json::to_string(payload)?,
            event.occurred_at,
        ],
    )?;
    Ok(())
}

fn event_id(run_id: &RunId, sequence: u64) -> EventId {
    EventId::from(format!("event:{}:{sequence}", run_id.0))
}

fn ensure_single_update(updated: usize) -> Result<(), StoreError> {
    if updated == 1 {
        Ok(())
    } else {
        Err(StoreError::Invariant(
            "concurrent state transition detected".into(),
        ))
    }
}

fn map_constraint(error: rusqlite::Error, message: &str) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Invariant(message.into())
        }
        _ => StoreError::Database(error),
    }
}

fn status_for_stop(reason: &StopReason) -> RunStatus {
    match reason {
        StopReason::Succeeded => RunStatus::Succeeded,
        StopReason::Failed => RunStatus::Failed,
        StopReason::Cancelled => RunStatus::Cancelled,
        StopReason::AwaitingApproval => RunStatus::AwaitingApproval,
        StopReason::BudgetExceeded => RunStatus::BudgetExceeded,
        StopReason::RepeatedFailure => RunStatus::RepeatedFailure,
        StopReason::InterruptedUnknownOutcome => RunStatus::InterruptedUnknownOutcome,
    }
}

fn stop_reason_name(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Succeeded => "succeeded",
        StopReason::Failed => "failed",
        StopReason::Cancelled => "cancelled",
        StopReason::AwaitingApproval => "awaiting_approval",
        StopReason::BudgetExceeded => "budget_exceeded",
        StopReason::RepeatedFailure => "repeated_failure",
        StopReason::InterruptedUnknownOutcome => "interrupted_unknown_outcome",
    }
}

fn action_kind(action: &AgentAction) -> &'static str {
    match action {
        AgentAction::ListFiles { .. } => "list_files",
        AgentAction::SearchText { .. } => "search_text",
        AgentAction::ReadFile { .. } => "read_file",
        AgentAction::WriteFile { .. } => "write_file",
        AgentAction::ApplyPatch { .. } => "apply_patch",
        AgentAction::RunCommand { .. } => "run_command",
        AgentAction::RunChecks { .. } => "run_checks",
        AgentAction::Remember { .. } => "remember",
        AgentAction::Recall { .. } => "recall",
        AgentAction::RequestApproval { .. } => "request_approval",
        AgentAction::Finish { .. } => "finish",
    }
}

fn policy_decision_name(decision: orchester_protokoll::PolicyDecision) -> &'static str {
    match decision {
        orchester_protokoll::PolicyDecision::Allow => "allow",
        orchester_protokoll::PolicyDecision::Ask => "ask",
        orchester_protokoll::PolicyDecision::Deny => "deny",
    }
}

fn current_step_status(
    connection: &Connection,
    snapshot: &RunSnapshot,
) -> Result<Option<String>, StoreError> {
    let Some(step_id) = snapshot.current_step_id.as_ref() else {
        return Ok(None);
    };
    connection
        .query_row(
            "SELECT status FROM steps WHERE run_id = ?1 AND step_id = ?2",
            params![snapshot.run_id.0, step_id.0],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
}

fn apply_event_transition(
    transaction: &Transaction<'_>,
    snapshot: &RunSnapshot,
    input: &EventAppend,
    permit_event_id: Option<&EventId>,
) -> Result<(), StoreError> {
    if input.turn_id != snapshot.current_turn_id || input.step_id != snapshot.current_step_id {
        return Err(StoreError::Invariant(
            "event does not belong to the current turn and step".into(),
        ));
    }
    let step_id = snapshot
        .current_step_id
        .as_ref()
        .ok_or_else(|| StoreError::Invariant("event requires a current step".into()))?;
    let step_status = current_step_status(transaction, snapshot)?
        .ok_or_else(|| StoreError::Invariant("current step is missing".into()))?;

    match &input.kind {
        HarnessEventKind::ModelStarted => {
            if snapshot.status != RunStatus::Running
                || step_status != "created"
                || input.call_id.is_none()
            {
                return Err(StoreError::Invariant(
                    "model start is illegal for the current step".into(),
                ));
            }
            transaction.execute(
                "UPDATE steps SET status = 'model_running', model_call_id = ?1
                 WHERE run_id = ?2 AND step_id = ?3 AND status = 'created'",
                params![
                    input.call_id.as_ref().map(|id| id.0.as_str()),
                    snapshot.run_id.0,
                    step_id.0,
                ],
            )?;
        }
        HarnessEventKind::ModelCompleted { assistant_text } => {
            if snapshot.status != RunStatus::Running
                || step_status != "model_running"
                || input.call_id.is_none()
                || assistant_text.len() > 65_536
            {
                return Err(StoreError::Invariant(
                    "model completion is illegal or oversized".into(),
                ));
            }
            let expected: Option<String> = transaction.query_row(
                "SELECT model_call_id FROM steps WHERE run_id = ?1 AND step_id = ?2",
                params![snapshot.run_id.0, step_id.0],
                |row| row.get(0),
            )?;
            if expected.as_deref() != input.call_id.as_ref().map(|id| id.0.as_str()) {
                return Err(StoreError::Invariant(
                    "model completion call does not match model start".into(),
                ));
            }
        }
        HarnessEventKind::PolicyDecided {
            action_id,
            decision,
            rule_id,
        } => {
            if snapshot.status != RunStatus::Running || step_status != "action_recorded" {
                return Err(StoreError::Invariant(
                    "policy decision requires one recorded action".into(),
                ));
            }
            let state = match decision {
                orchester_protokoll::PolicyDecision::Allow => "ready",
                orchester_protokoll::PolicyDecision::Ask => "awaiting_approval",
                orchester_protokoll::PolicyDecision::Deny => "denied",
            };
            let updated = transaction.execute(
                "UPDATE actions SET policy_decision = ?1, policy_rule_id = ?2, state = ?3
                 WHERE run_id = ?4 AND step_id = ?5 AND action_id = ?6 AND state = 'recorded'",
                params![
                    policy_decision_name(*decision),
                    rule_id,
                    state,
                    snapshot.run_id.0,
                    step_id.0,
                    action_id.0,
                ],
            )?;
            ensure_single_update(updated)?;
            match decision {
                orchester_protokoll::PolicyDecision::Ask => {
                    transaction.execute(
                        "UPDATE steps SET status = 'awaiting_approval' WHERE step_id = ?1",
                        params![step_id.0],
                    )?;
                    transaction.execute(
                        "UPDATE runs SET status = 'awaiting_approval' WHERE run_id = ?1",
                        params![snapshot.run_id.0],
                    )?;
                }
                orchester_protokoll::PolicyDecision::Deny => {
                    transaction.execute(
                        "UPDATE steps SET status = 'observed' WHERE step_id = ?1",
                        params![step_id.0],
                    )?;
                }
                orchester_protokoll::PolicyDecision::Allow => {}
            }
        }
        HarnessEventKind::ApprovalRequested { request } => {
            if snapshot.status != RunStatus::AwaitingApproval
                || step_status != "awaiting_approval"
                || request.run_id != snapshot.run_id
            {
                return Err(StoreError::Invariant(
                    "approval request is not bound to the paused action".into(),
                ));
            }
            let action_matches: bool = transaction
                .query_row(
                    "SELECT 1 FROM actions WHERE run_id = ?1 AND step_id = ?2
                     AND action_id = ?3 AND state = 'awaiting_approval'",
                    params![snapshot.run_id.0, step_id.0, request.action_id.0],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if !action_matches {
                return Err(StoreError::Invariant(
                    "approval request action is not awaiting approval".into(),
                ));
            }
        }
        HarnessEventKind::ToolStarted { action_id } => {
            let permit_event_id = permit_event_id.ok_or_else(|| {
                StoreError::Invariant("tool start requires an ExecutionPermit".into())
            })?;
            let call_id = input.call_id.as_ref().ok_or_else(|| {
                StoreError::Invariant("tool start requires a call identifier".into())
            })?;
            if snapshot.status != RunStatus::Running || step_status != "action_recorded" {
                return Err(StoreError::Invariant(
                    "tool start requires an allowed action".into(),
                ));
            }
            let checkpoint: Option<(i64, String)> = transaction
                .query_row(
                    "SELECT a.audit_sequence, a.policy_decision FROM actions a
                     JOIN audit_checkpoints c ON c.event_id = a.audit_event_id
                     WHERE a.run_id = ?1 AND a.action_id = ?2 AND a.state = 'ready'
                       AND a.audit_event_id = ?3
                       AND (a.policy_decision = 'allow' OR EXISTS(
                         SELECT 1 FROM approvals ap
                         WHERE ap.action_id = a.action_id AND ap.state = 'executing'
                           AND ap.expires_at_unix > ?4
                       ))",
                    params![
                        snapshot.run_id.0,
                        action_id.0,
                        permit_event_id.0,
                        system_unix_now()
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((_, policy_decision)) = checkpoint else {
                return Err(StoreError::Invariant(
                    "tool start requires a durable audit checkpoint".into(),
                ));
            };
            let updated = transaction.execute(
                "UPDATE actions SET state = 'executing' WHERE run_id = ?1 AND step_id = ?2
                 AND action_id = ?3 AND state = 'ready'",
                params![snapshot.run_id.0, step_id.0, action_id.0],
            )?;
            ensure_single_update(updated)?;
            let approval_updated = transaction.execute(
                "UPDATE approvals SET state = 'consumed', consumed_at = CURRENT_TIMESTAMP,
                        capability_nonce_hash = NULL, row_version = row_version + 1
                 WHERE action_id = ?1 AND state = 'executing'",
                params![action_id.0],
            )?;
            if policy_decision == "ask" {
                ensure_single_update(approval_updated)?;
            } else if approval_updated != 0 {
                return Err(StoreError::Invariant(
                    "allow action unexpectedly consumed an approval".into(),
                ));
            }
            transaction.execute(
                "INSERT INTO tool_attempts(call_id, action_id, attempt_no, state, started_at)
                 VALUES(?1, ?2, 1, 'started', ?3)",
                params![call_id.0, action_id.0, input.occurred_at],
            )?;
            let step_updated = transaction.execute(
                "UPDATE steps SET status = 'tool_running' WHERE step_id = ?1",
                params![step_id.0],
            )?;
            ensure_single_update(step_updated)?;
        }
        HarnessEventKind::ToolCompleted { observation } => {
            let call_id = input.call_id.as_ref().ok_or_else(|| {
                StoreError::Invariant("tool completion requires a call identifier".into())
            })?;
            if snapshot.status != RunStatus::Running
                || step_status != "tool_running"
                || observation.call_id != *call_id
            {
                return Err(StoreError::Invariant(
                    "tool completion does not match a started attempt".into(),
                ));
            }
            finish_tool_attempt(transaction, snapshot, step_id, call_id, "completed")?;
        }
        HarnessEventKind::ToolFailed { .. } => {
            let call_id = input.call_id.as_ref().ok_or_else(|| {
                StoreError::Invariant("tool failure requires a call identifier".into())
            })?;
            if snapshot.status != RunStatus::Running || step_status != "tool_running" {
                return Err(StoreError::Invariant(
                    "tool failure does not match a started attempt".into(),
                ));
            }
            finish_tool_attempt(transaction, snapshot, step_id, call_id, "failed")?;
        }
        HarnessEventKind::ValidatorCompleted { .. } => {
            if snapshot.status != RunStatus::Running || step_status != "observed" {
                return Err(StoreError::Invariant(
                    "validator result requires an observed step".into(),
                ));
            }
        }
        HarnessEventKind::RunCreated
        | HarnessEventKind::StepStarted
        | HarnessEventKind::ActionRecorded { .. }
        | HarnessEventKind::ApprovalResolved { .. }
        | HarnessEventKind::RunPaused { .. }
        | HarnessEventKind::RunCompleted { .. } => {
            return Err(StoreError::Invariant(
                "event kind requires a specialized state transition".into(),
            ));
        }
    }
    Ok(())
}

fn finish_tool_attempt(
    transaction: &Transaction<'_>,
    snapshot: &RunSnapshot,
    step_id: &StepId,
    call_id: &CallId,
    terminal: &str,
) -> Result<(), StoreError> {
    let attempt = transaction.execute(
        "UPDATE tool_attempts SET state = ?1, terminal_at = CURRENT_TIMESTAMP
         WHERE call_id = ?2 AND state = 'started'",
        params![terminal, call_id.0],
    )?;
    ensure_single_update(attempt)?;
    let action = transaction.execute(
        "UPDATE actions SET state = ?1, terminal_at = CURRENT_TIMESTAMP
         WHERE run_id = ?2 AND step_id = ?3 AND state = 'executing'",
        params![terminal, snapshot.run_id.0, step_id.0],
    )?;
    ensure_single_update(action)?;
    transaction.execute(
        "UPDATE steps SET status = 'observed', finished_at = CURRENT_TIMESTAMP WHERE step_id = ?1",
        params![step_id.0],
    )?;
    Ok(())
}

/// Compute the durable hash used to bind an action to policy/approval records.
pub fn action_hash(action: &AgentAction) -> Result<String, StoreError> {
    let canonical_json = serde_json::to_string(action)?;
    Ok(hash_canonical_action(&canonical_json))
}

fn hash_canonical_action(canonical_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn system_unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
