//! Transactional source of truth for self-harness runs.
//!
//! Every state transition and its public event are committed in one
//! `BEGIN IMMEDIATE` transaction.  The store never infers progress from logs:
//! callers can close the process, reopen the database, and resume from the
//! exact persisted snapshot.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use orchester_protokoll::{
    ActionId, AgentAction, CallId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
    StopReason, TurnId, HARNESS_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use thiserror::Error;

const MIGRATION: &str = include_str!("../../migrations/0001_state.sql");

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        transition: Transition,
    ) -> Result<HarnessEvent, StoreError>;
    fn record_action(&self, action: ActionRecord) -> Result<HarnessEvent, StoreError>;
    fn load_run(&self, run_id: &RunId) -> Result<RunSnapshot, StoreError>;
    fn load_run_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<RunSnapshot, StoreError>;
    fn events(&self, run_id: &RunId) -> Result<Vec<HarnessEvent>, StoreError>;
    fn events_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<Vec<HarnessEvent>, StoreError>;
    fn mark_audit_checkpoint(
        &self,
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
            std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
        }
        let connection = Connection::open(path)?;
        Self::initialize(connection)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(connection: Connection) -> Result<Self, StoreError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL; PRAGMA journal_mode = WAL;",
        )?;
        connection.execute_batch(MIGRATION)?;
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
        self.load_run(&input.run_id)
    }

    fn append_transition(
        &self,
        run_id: &RunId,
        transition: Transition,
    ) -> Result<HarnessEvent, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, None)?;
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

    fn record_action(&self, action: ActionRecord) -> Result<HarnessEvent, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, &action.run_id, None)?;
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

    fn load_run(&self, run_id: &RunId) -> Result<RunSnapshot, StoreError> {
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, None)
    }

    fn load_run_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<RunSnapshot, StoreError> {
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, Some(owner_actor_id))
    }

    fn events(&self, run_id: &RunId) -> Result<Vec<HarnessEvent>, StoreError> {
        let connection = self.connection()?;
        load_events(&connection, run_id)
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
        event_id: &EventId,
        checkpoint: AuditCheckpoint,
    ) -> Result<(), StoreError> {
        let connection = self.connection()?;
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
