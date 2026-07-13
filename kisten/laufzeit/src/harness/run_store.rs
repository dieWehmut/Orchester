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
    ActionId, AgentAction, EventId, HarnessEvent, HarnessEventKind, RunId, StopReason,
    HARNESS_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use secrecy::SecretString;
use sha2::{Digest, Sha256};

use crate::harness::approval::{
    ApprovalBinding, ApprovalRequestInput, ApprovalSnapshot, ApprovalState,
};
use crate::harness::audit::{AuditInput, AuditReceipt};
use crate::harness::barrier::{ExecutionPermit, StartedTool};
use crate::harness::feedback::FeedbackEngine;

mod database;
mod observation;
mod sanitized;
mod schema;
mod storage;
mod transcript;
mod transition;
mod types;

use database::{
    action_kind, advance_run, append_approval_event, approval_context_matches,
    capability_hash_matches, close_ready_approval, close_waiting_approval, current_step_id,
    current_step_status, ensure_single_update, event_id, load_approval_row, load_events,
    load_snapshot, map_constraint, persist_event, policy_decision_name, status_for_stop,
    stop_reason_name, update_approval_state,
};
pub use transcript::StoredTranscriptRecord;
pub use types::{
    ActionRecord, AuditCheckpoint, EffectClass, EventAppend, NewRun, RunSnapshot, RunStatus,
    RunStore, StoreError, Transition,
};

fn terminal_sanitizer(secrets: Vec<SecretString>) -> FeedbackEngine {
    secrets
        .into_iter()
        .fold(FeedbackEngine::default(), FeedbackEngine::with_secret)
}

pub struct SqliteRunStore {
    connection: Mutex<Connection>,
    event_sanitizer: FeedbackEngine,
    terminal_sanitizer: Option<FeedbackEngine>,
}

impl SqliteRunStore {
    /// Open a store for non-terminal state operations. Tool terminal events
    /// fail closed until the caller explicitly supplies its secret set.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::open_configured(path.as_ref(), None)
    }

    /// Open a store whose terminal-event sanitizer knows every configured
    /// credential that could appear in tool output.
    pub fn open_with_terminal_secrets(
        path: impl AsRef<Path>,
        secrets: Vec<SecretString>,
    ) -> Result<Self, StoreError> {
        Self::open_configured(path.as_ref(), Some(terminal_sanitizer(secrets)))
    }

    fn open_configured(
        path: &Path,
        terminal_sanitizer: Option<FeedbackEngine>,
    ) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            let created = !parent.exists();
            std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
            storage::ensure_private_state_dir(parent, created)?;
        }
        let file_existed = path.exists();
        let wal_existed = storage::state_sidecar(path, "-wal").exists();
        let shm_existed = storage::state_sidecar(path, "-shm").exists();
        let connection = Connection::open(path)?;
        storage::ensure_private_state_file(path, !file_existed)?;
        let store = Self::initialize(connection, true, terminal_sanitizer)?;
        storage::ensure_private_state_sidecars(path, wal_existed, shm_existed)?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Self::initialize(Connection::open_in_memory()?, false, None)
    }

    fn initialize(
        mut connection: Connection,
        enable_wal: bool,
        terminal_sanitizer: Option<FeedbackEngine>,
    ) -> Result<Self, StoreError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")?;
        schema::apply_migrations(&mut connection)?;
        if enable_wal {
            storage::enable_wal_mode(&connection)?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
            event_sanitizer: terminal_sanitizer.clone().unwrap_or_default(),
            terminal_sanitizer,
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
        sanitized::ensure_durable_field(
            "approval owner identifier",
            &input.owner_actor_id,
            &self.event_sanitizer,
        )?;
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
                 JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                 WHERE a.action_id = ?1 AND a.run_id = ?2 AND r.owner_actor_id = ?3
                   AND a.origin_model_call_id IS NOT NULL
                   AND s.model_phase = 'completed'
                   AND a.origin_model_call_id = s.model_call_id",
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
        let request = sanitized::canonicalize_approval_request(
            input.protocol_request(),
            &self.event_sanitizer,
        )?;
        let action_summary = request.action_summary.clone();
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
            kind: HarnessEventKind::ApprovalRequested {
                request: request.clone(),
            },
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
                    request.approval_id.0,
                    request.run_id.0,
                    request.action_id.0,
                    input.owner_actor_id,
                    request.action_hash,
                    action_summary,
                    request.workspace_identity,
                    request.policy_snapshot_hash,
                    request.config_snapshot_hash,
                    request.risk,
                    request.rule_id,
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
                 JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                 LEFT JOIN approvals ap ON ap.action_id = a.action_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2 AND r.owner_actor_id = ?3
                   AND a.origin_model_call_id IS NOT NULL
                   AND s.model_phase = 'completed'
                   AND a.origin_model_call_id = s.model_call_id",
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
                 JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2 AND r.owner_actor_id = ?3
                   AND a.origin_model_call_id IS NOT NULL
                   AND s.model_phase = 'completed'
                   AND a.origin_model_call_id = s.model_call_id",
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
                 JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                 LEFT JOIN approvals ap ON ap.action_id = a.action_id
                 WHERE a.run_id = ?1 AND a.action_id = ?2
                   AND a.origin_model_call_id IS NOT NULL
                   AND s.model_phase = 'completed'
                   AND a.origin_model_call_id = s.model_call_id
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
        let (input, terminal_observation) =
            observation::prepare_terminal_input(run_id, input, self.terminal_sanitizer.as_ref())?;
        let input = sanitized::canonicalize_input(input, &self.event_sanitizer)?;
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
        transition::apply_event_transition(
            &transaction,
            &snapshot,
            &input,
            permit_event_id,
            terminal_observation.as_ref(),
        )?;
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
                let kind = sanitized::canonicalize_kind(kind, &self.event_sanitizer)?;
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
        sanitized::ensure_durable_field(
            "action identifier",
            &action.action_id.0,
            &self.event_sanitizer,
        )?;
        sanitized::ensure_durable_field("run identifier", &action.run_id.0, &self.event_sanitizer)?;
        sanitized::ensure_durable_field(
            "step identifier",
            &action.step_id.0,
            &self.event_sanitizer,
        )?;
        sanitized::ensure_durable_field(
            "provider call identifier",
            &action.call_id.0,
            &self.event_sanitizer,
        )?;
        sanitized::ensure_durable_field(
            "origin model call identifier",
            &action.origin_model_call_id.0,
            &self.event_sanitizer,
        )?;
        sanitized::ensure_durable_field(
            "action timestamp",
            &action.occurred_at,
            &self.event_sanitizer,
        )?;

        let canonical_json = sanitized::durable_action_json(&action.action, &self.event_sanitizer)?;
        let expected_hash = hash_canonical_action(&canonical_json);
        if action.action_hash != expected_hash {
            return Err(StoreError::Invariant(
                "action hash does not match canonical action".into(),
            ));
        }
        let model_binding: Option<(String, String, Option<String>)> = transaction
            .query_row(
                "SELECT status, model_phase, model_call_id
                 FROM steps WHERE run_id = ?1 AND step_id = ?2",
                params![action.run_id.0, action.step_id.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((step_status, model_phase, model_call_id)) = model_binding else {
            return Err(StoreError::Invariant("action step is missing".into()));
        };
        if step_status != "model_running"
            || model_phase != "completed"
            || model_call_id.as_deref() != Some(action.origin_model_call_id.0.as_str())
        {
            return Err(StoreError::Invariant(
                "action origin does not match a completed model call".into(),
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
                action_id, run_id, step_id, call_id, origin_model_call_id,
                kind, canonical_json, action_hash, effect_class, state, created_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'recorded', ?10)",
            params![
                action.action_id.0,
                action.run_id.0,
                action.step_id.0,
                action.call_id.0,
                action.origin_model_call_id.0,
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
                origin_model_call_id: Some(action.origin_model_call_id.clone()),
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
