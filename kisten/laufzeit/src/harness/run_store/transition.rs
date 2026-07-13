use orchester_protokoll::{CallId, EventId, HarnessEventKind, StepId};
use rusqlite::{params, OptionalExtension, Transaction};

use super::observation::DurableObservation;
use super::{
    current_step_status, ensure_single_update, map_constraint, policy_decision_name,
    system_unix_now, EventAppend, RunSnapshot, RunStatus, StoreError,
};

pub(super) fn apply_event_transition(
    transaction: &Transaction<'_>,
    snapshot: &RunSnapshot,
    input: &EventAppend,
    permit_event_id: Option<&EventId>,
    terminal_observation: Option<&DurableObservation>,
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
            let updated = transaction.execute(
                "UPDATE steps
                 SET status = 'model_running', model_call_id = ?1,
                     model_phase = 'running'
                 WHERE run_id = ?2 AND step_id = ?3 AND status = 'created'
                   AND model_phase = 'not_started' AND model_call_id IS NULL",
                params![
                    input.call_id.as_ref().map(|id| id.0.as_str()),
                    snapshot.run_id.0,
                    step_id.0,
                ],
            )?;
            ensure_single_update(updated)?;
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
            let updated = transaction.execute(
                "UPDATE steps SET model_phase = 'completed'
                 WHERE run_id = ?1 AND step_id = ?2 AND status = 'model_running'
                   AND model_phase = 'running' AND model_call_id = ?3",
                params![
                    snapshot.run_id.0,
                    step_id.0,
                    input.call_id.as_ref().map(|id| id.0.as_str()),
                ],
            )?;
            ensure_single_update(updated)?;
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
                 WHERE run_id = ?4 AND step_id = ?5 AND action_id = ?6
                   AND state = 'recorded' AND origin_model_call_id IS NOT NULL
                   AND EXISTS(
                     SELECT 1 FROM steps s
                     WHERE s.run_id = actions.run_id AND s.step_id = actions.step_id
                       AND s.model_phase = 'completed'
                       AND actions.origin_model_call_id = s.model_call_id
                   )",
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
                    let step_updated = transaction.execute(
                        "UPDATE steps SET status = 'awaiting_approval'
                         WHERE run_id = ?1 AND step_id = ?2 AND status = 'action_recorded'",
                        params![snapshot.run_id.0, step_id.0],
                    )?;
                    ensure_single_update(step_updated)?;
                    let run_updated = transaction.execute(
                        "UPDATE runs SET status = 'awaiting_approval'
                         WHERE run_id = ?1 AND status = 'running' AND current_step_id = ?2",
                        params![snapshot.run_id.0, step_id.0],
                    )?;
                    ensure_single_update(run_updated)?;
                }
                orchester_protokoll::PolicyDecision::Deny => {
                    let step_updated = transaction.execute(
                        "UPDATE steps SET status = 'observed'
                         WHERE run_id = ?1 AND step_id = ?2 AND status = 'action_recorded'",
                        params![snapshot.run_id.0, step_id.0],
                    )?;
                    ensure_single_update(step_updated)?;
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
                     AND action_id = ?3 AND state = 'awaiting_approval'
                     AND origin_model_call_id IS NOT NULL
                     AND EXISTS(
                       SELECT 1 FROM steps s
                       WHERE s.run_id = actions.run_id AND s.step_id = actions.step_id
                         AND s.model_phase = 'completed'
                         AND actions.origin_model_call_id = s.model_call_id
                     )",
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
                     JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                     WHERE a.run_id = ?1 AND a.action_id = ?2 AND a.state = 'ready'
                       AND a.audit_event_id = ?3
                       AND a.call_id = ?4
                       AND a.origin_model_call_id IS NOT NULL
                       AND s.model_phase = 'completed'
                       AND a.origin_model_call_id = s.model_call_id
                       AND (a.policy_decision = 'allow' OR EXISTS(
                         SELECT 1 FROM approvals ap
                         WHERE ap.action_id = a.action_id AND ap.state = 'executing'
                           AND ap.expires_at_unix > ?5
                       ))",
                    params![
                        snapshot.run_id.0,
                        action_id.0,
                        permit_event_id.0,
                        call_id.0,
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
                "UPDATE steps SET status = 'tool_running'
                 WHERE run_id = ?1 AND step_id = ?2 AND status = 'action_recorded'",
                params![snapshot.run_id.0, step_id.0],
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
            let durable = terminal_observation.ok_or_else(|| {
                StoreError::Invariant("tool completion requires a durable observation".into())
            })?;
            finish_tool_attempt(
                transaction,
                snapshot,
                step_id,
                call_id,
                "completed",
                durable,
                &input.occurred_at,
            )?;
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
            let durable = terminal_observation.ok_or_else(|| {
                StoreError::Invariant("tool failure requires a durable observation".into())
            })?;
            finish_tool_attempt(
                transaction,
                snapshot,
                step_id,
                call_id,
                "failed",
                durable,
                &input.occurred_at,
            )?;
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
    observation: &DurableObservation,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let action_id = transaction
        .query_row(
            "SELECT a.action_id
             FROM steps s
             JOIN actions a ON a.run_id = s.run_id AND a.step_id = s.step_id
                           AND a.action_id = s.action_id
             JOIN tool_attempts ta ON ta.action_id = a.action_id AND ta.call_id = a.call_id
             WHERE s.run_id = ?1 AND s.step_id = ?2 AND s.status = 'tool_running'
               AND a.state = 'executing' AND a.call_id = ?3 AND ta.state = 'started'",
            params![snapshot.run_id.0, step_id.0, call_id.0],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| {
            StoreError::Invariant(
                "tool attempt is not bound to the current step action and call".into(),
            )
        })?;
    if observation.call_id != *call_id || observation.outcome != terminal {
        return Err(StoreError::Invariant(
            "terminal observation does not match the tool attempt".into(),
        ));
    }
    transaction
        .execute(
            "INSERT INTO observations(
               observation_id, run_id, step_id, call_id, kind,
               sanitized_payload, fingerprint, created_at, outcome
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                observation.observation_id.0,
                snapshot.run_id.0,
                step_id.0,
                observation.call_id.0,
                observation.kind,
                observation.payload,
                observation.fingerprint,
                occurred_at,
                observation.outcome,
            ],
        )
        .map_err(|error| map_constraint(error, "terminal observation is already linked"))?;
    let attempt = transaction.execute(
        "UPDATE tool_attempts
         SET state = ?1, terminal_at = CURRENT_TIMESTAMP, observation_id = ?4
         WHERE call_id = ?2 AND action_id = ?3 AND state = 'started'
           AND observation_id IS NULL",
        params![terminal, call_id.0, action_id, observation.observation_id.0],
    )?;
    ensure_single_update(attempt)?;
    let action = transaction.execute(
        "UPDATE actions SET state = ?1, terminal_at = CURRENT_TIMESTAMP
         WHERE run_id = ?2 AND step_id = ?3 AND action_id = ?4
           AND call_id = ?5 AND state = 'executing'",
        params![terminal, snapshot.run_id.0, step_id.0, action_id, call_id.0],
    )?;
    ensure_single_update(action)?;
    let step = transaction.execute(
        "UPDATE steps SET status = 'observed', finished_at = CURRENT_TIMESTAMP
         WHERE run_id = ?1 AND step_id = ?2 AND action_id = ?3 AND status = 'tool_running'",
        params![snapshot.run_id.0, step_id.0, action_id],
    )?;
    ensure_single_update(step)?;
    Ok(())
}
