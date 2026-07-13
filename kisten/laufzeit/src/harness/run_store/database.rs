use orchester_protokoll::{
    ActionId, AgentAction, CallId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
    StopReason, TurnId, HARNESS_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Value};

use crate::harness::approval::{ApprovalBinding, ApprovalSnapshot, ApprovalState};

use super::{hash_canonical_action, ApprovalRow, RunSnapshot, RunStatus, StoreError};

pub(super) fn load_snapshot(
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

pub(super) fn load_approval_row(
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

pub(super) fn advance_run(
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

pub(super) fn append_approval_event(
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

pub(super) fn update_approval_state(
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

pub(super) fn capability_hash_matches(
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

pub(super) fn approval_context_matches(
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
             JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
             WHERE a.run_id = ?1 AND a.action_id = ?2
               AND r.owner_actor_id = ?3 AND a.action_hash = ?4
               AND p.workspace_identity = ?5
               AND r.policy_snapshot_hash = ?6
               AND r.config_snapshot_hash = ?7
               AND a.policy_decision = 'ask' AND a.policy_rule_id = ?8
               AND a.state = ?9 AND r.status = ?10
               AND a.origin_model_call_id IS NOT NULL
               AND s.model_phase = 'completed'
               AND a.origin_model_call_id = s.model_call_id",
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

pub(super) fn current_step_id(
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

pub(super) fn close_waiting_approval(
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

pub(super) fn close_ready_approval(
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

pub(super) fn snapshot_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<Result<RunSnapshot, StoreError>> {
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

pub(super) fn load_events(
    connection: &Connection,
    run_id: &RunId,
) -> Result<Vec<HarnessEvent>, StoreError> {
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
        let payload: Value = serde_json::from_str(&payload).map_err(|_| StoreError::Corrupt)?;
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
        let event = serde_json::from_value(value).map_err(|_| StoreError::Corrupt)?;
        events.push(normalize_action_event(connection, run_id, event)?);
    }
    Ok(events)
}

fn normalize_action_event(
    connection: &Connection,
    run_id: &RunId,
    mut event: HarnessEvent,
) -> Result<HarnessEvent, StoreError> {
    let (action_id, action_summary, action_hash, event_origin) = match &event.kind {
        HarnessEventKind::ActionRecorded {
            action_id,
            action_summary,
            action_hash,
            origin_model_call_id,
        } => (
            action_id,
            action_summary.as_str(),
            action_hash.as_str(),
            origin_model_call_id.as_ref(),
        ),
        _ => return Ok(event),
    };
    let step_id = event.step_id.as_ref().ok_or(StoreError::Corrupt)?;
    let call_id = event.call_id.as_ref().ok_or(StoreError::Corrupt)?;
    type DurableActionRow = (String, String, Option<String>, String, String);
    let durable: Option<DurableActionRow> = connection
        .query_row(
            "SELECT step_id, call_id, origin_model_call_id, canonical_json, action_hash
             FROM actions WHERE run_id = ?1 AND action_id = ?2",
            params![run_id.0, action_id.0],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((durable_step, durable_call, durable_origin, canonical_json, durable_hash)) = durable
    else {
        return Err(StoreError::Corrupt);
    };
    let action: AgentAction =
        serde_json::from_str(&canonical_json).map_err(|_| StoreError::Corrupt)?;
    let canonical = serde_json::to_string(&action).map_err(|_| StoreError::Corrupt)?;
    let derived_summary = action.action_summary();
    if durable_step != step_id.0
        || durable_call != call_id.0
        || canonical != canonical_json
        || hash_canonical_action(&canonical_json) != durable_hash
    {
        return Err(StoreError::Corrupt);
    }
    if action_summary != derived_summary
        || action_hash != durable_hash
        || event_origin.map(|origin| origin.0.as_str()) != durable_origin.as_deref()
    {
        return Err(StoreError::Corrupt);
    }
    event.kind = HarnessEventKind::ActionRecorded {
        action_id: action_id.clone(),
        action_summary: derived_summary,
        action_hash: durable_hash,
        origin_model_call_id: durable_origin.map(CallId::from),
    };
    Ok(event)
}

pub(super) fn persist_event(
    transaction: &Transaction<'_>,
    event: &HarnessEvent,
) -> Result<(), StoreError> {
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

pub(super) fn event_id(run_id: &RunId, sequence: u64) -> EventId {
    EventId::from(format!("event:{}:{sequence}", run_id.0))
}

pub(super) fn ensure_single_update(updated: usize) -> Result<(), StoreError> {
    if updated == 1 {
        Ok(())
    } else {
        Err(StoreError::Invariant(
            "concurrent state transition detected".into(),
        ))
    }
}

pub(super) fn map_constraint(error: rusqlite::Error, message: &str) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Invariant(message.into())
        }
        _ => StoreError::Database(error),
    }
}

pub(super) fn status_for_stop(reason: &StopReason) -> RunStatus {
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

pub(super) fn stop_reason_name(reason: &StopReason) -> &'static str {
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

pub(super) fn action_kind(action: &AgentAction) -> &'static str {
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

pub(super) fn policy_decision_name(decision: orchester_protokoll::PolicyDecision) -> &'static str {
    match decision {
        orchester_protokoll::PolicyDecision::Allow => "allow",
        orchester_protokoll::PolicyDecision::Ask => "ask",
        orchester_protokoll::PolicyDecision::Deny => "deny",
    }
}

pub(super) fn current_step_status(
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
