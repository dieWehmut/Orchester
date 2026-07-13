use orchester_protokoll::{ActionId, AgentAction};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::harness::transcript::TranscriptCodec;

use super::super::{
    hash_canonical_action, transcript, RunSnapshot, StoreError, TranscriptBinding,
    TranscriptBindingPhase,
};
use super::StepRow;

pub(super) struct ActionRow {
    pub(super) call_id: String,
    pub(super) state: String,
    pub(super) policy_decision: Option<String>,
    policy_rule_id: Option<String>,
    policy_event_id: Option<String>,
    audit_event_id: Option<String>,
    pub(super) approval_id: Option<String>,
    pub(super) approval_state: Option<String>,
    approval_action_hash: Option<String>,
    approval_workspace_identity: Option<String>,
    approval_policy_snapshot_hash: Option<String>,
    approval_config_snapshot_hash: Option<String>,
    approval_owner_actor_id: Option<String>,
    approval_rule_id: Option<String>,
    approval_event_id: Option<String>,
    origin_model_call_id: Option<String>,
    canonical_json: String,
    action_hash: String,
}

pub(super) fn load_action(
    connection: &Connection,
    run: &RunSnapshot,
    step: &StepRow,
    action_id: &ActionId,
    codec: &TranscriptCodec,
) -> Result<ActionRow, StoreError> {
    let row = connection
        .query_row(
            "SELECT a.call_id, a.state, a.policy_decision, a.policy_rule_id,
                    a.policy_event_id, a.audit_event_id,
                    ap.approval_id, ap.state, ap.action_hash,
                    ap.workspace_identity, ap.policy_snapshot_hash,
                    ap.config_snapshot_hash, ap.owner_actor_id, ap.rule_id,
                    ap.approval_event_id,
                    a.origin_model_call_id, a.canonical_json, a.action_hash
             FROM actions a
             LEFT JOIN approvals ap
               ON ap.run_id = a.run_id AND ap.action_id = a.action_id
              AND ap.owner_actor_id = ?4
             WHERE a.run_id = ?1 AND a.step_id = ?2 AND a.action_id = ?3",
            params![run.run_id.0, step.step_id, action_id.0, run.owner_actor_id],
            |row| {
                Ok(ActionRow {
                    call_id: row.get(0)?,
                    state: row.get(1)?,
                    policy_decision: row.get(2)?,
                    policy_rule_id: row.get(3)?,
                    policy_event_id: row.get(4)?,
                    audit_event_id: row.get(5)?,
                    approval_id: row.get(6)?,
                    approval_state: row.get(7)?,
                    approval_action_hash: row.get(8)?,
                    approval_workspace_identity: row.get(9)?,
                    approval_policy_snapshot_hash: row.get(10)?,
                    approval_config_snapshot_hash: row.get(11)?,
                    approval_owner_actor_id: row.get(12)?,
                    approval_rule_id: row.get(13)?,
                    approval_event_id: row.get(14)?,
                    origin_model_call_id: row.get(15)?,
                    canonical_json: row.get(16)?,
                    action_hash: row.get(17)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let origin_model_call_id = row
        .origin_model_call_id
        .as_deref()
        .ok_or(StoreError::Corrupt)?;
    if step.model_phase != "completed"
        || step.model_call_id.as_deref() != Some(origin_model_call_id)
    {
        return Err(StoreError::Corrupt);
    }
    let action: AgentAction =
        serde_json::from_str(&row.canonical_json).map_err(|_| StoreError::Corrupt)?;
    let canonical = serde_json::to_string(&action).map_err(|_| StoreError::Corrupt)?;
    if canonical != row.canonical_json || hash_canonical_action(&canonical) != row.action_hash {
        return Err(StoreError::Corrupt);
    }
    load_model_response_binding(connection, run, step, codec)?;
    let action_sequence: i64 = connection
        .query_row(
            "SELECT sequence FROM events
             WHERE run_id = ?1 AND step_id = ?2 AND call_id = ?3
               AND kind = 'action.recorded'
             ORDER BY sequence DESC LIMIT 1",
            params![run.run_id.0, step.step_id, row.call_id],
            |query| query.get(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let action_binding = transcript::load_binding(
        connection,
        &run.run_id,
        action_sequence,
        TranscriptBindingPhase::Action,
        codec,
    )?
    .ok_or(StoreError::Corrupt)?;
    if action_binding.record_count != 1 {
        return Err(StoreError::Corrupt);
    }
    Ok(row)
}

fn load_model_response_binding(
    connection: &Connection,
    run: &RunSnapshot,
    step: &StepRow,
    codec: &TranscriptCodec,
) -> Result<TranscriptBinding, StoreError> {
    let call_id = step.model_call_id.as_deref().ok_or(StoreError::Corrupt)?;
    let sequence: i64 = connection
        .query_row(
            "SELECT sequence FROM events
             WHERE run_id = ?1 AND step_id = ?2 AND call_id = ?3
               AND kind = 'model.completed'
             ORDER BY sequence DESC LIMIT 1",
            params![run.run_id.0, step.step_id, call_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    transcript::load_binding(
        connection,
        &run.run_id,
        sequence,
        TranscriptBindingPhase::ModelResponse,
        codec,
    )?
    .ok_or(StoreError::Corrupt)
}

pub(super) fn require_unprocessed_policy(action: &ActionRow) -> Result<(), StoreError> {
    if action.policy_decision.is_none()
        && action.policy_rule_id.is_none()
        && action.policy_event_id.is_none()
        && action.audit_event_id.is_none()
        && action.approval_id.is_none()
    {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

pub(super) fn require_policy_event(
    connection: &Connection,
    run: &RunSnapshot,
    step: &StepRow,
    action_id: &ActionId,
    action: &ActionRow,
    expected_decision: &str,
) -> Result<(), StoreError> {
    let event_id = action
        .policy_event_id
        .as_deref()
        .ok_or(StoreError::Corrupt)?;
    let (event_step, event_call, kind, payload): (String, Option<String>, String, String) =
        connection
            .query_row(
                "SELECT step_id, call_id, kind, sanitized_payload
                 FROM events WHERE run_id = ?1 AND event_id = ?2",
                params![run.run_id.0, event_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or(StoreError::Corrupt)?;
    if event_step != step.step_id || event_call.is_some() || kind != "policy.decided" {
        return Err(StoreError::Corrupt);
    }
    let payload: Value = serde_json::from_str(&payload).map_err(|_| StoreError::Corrupt)?;
    if payload.get("action_id").and_then(Value::as_str) != Some(action_id.0.as_str())
        || payload.get("decision").and_then(Value::as_str) != Some(expected_decision)
        || payload.get("rule_id").and_then(Value::as_str) != action.policy_rule_id.as_deref()
    {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

pub(super) fn validate_optional_audit_checkpoint(
    connection: &Connection,
    run: &RunSnapshot,
    action: &ActionRow,
) -> Result<(), StoreError> {
    let Some(event_id) = action.audit_event_id.as_deref() else {
        return Ok(());
    };
    if action.policy_event_id.as_deref() != Some(event_id)
        && action.approval_event_id.as_deref() != Some(event_id)
    {
        return Err(StoreError::Corrupt);
    }
    let event_exists = connection
        .query_row(
            "SELECT 1 FROM events WHERE run_id = ?1 AND event_id = ?2",
            params![run.run_id.0, event_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !event_exists {
        return Err(StoreError::Corrupt);
    }
    let checkpoint: Option<(i64, String)> = connection
        .query_row(
            "SELECT audit_sequence, head_hash FROM audit_checkpoints WHERE event_id = ?1",
            params![event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((sequence, head_hash)) = checkpoint else {
        return Err(StoreError::Corrupt);
    };
    if sequence <= 0
        || head_hash.len() != 64
        || !head_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

pub(super) fn validate_approval_binding(
    connection: &Connection,
    run: &RunSnapshot,
    action_id: &ActionId,
    action: &ActionRow,
) -> Result<(), StoreError> {
    let approval_id = action.approval_id.as_deref().ok_or(StoreError::Corrupt)?;
    if action.approval_action_hash.as_deref() != Some(action.action_hash.as_str())
        || action.approval_workspace_identity.is_none()
        || action.approval_policy_snapshot_hash.as_deref()
            != Some(run.policy_snapshot_hash.as_str())
        || action.approval_config_snapshot_hash.as_deref()
            != Some(run.config_snapshot_hash.as_str())
        || action.approval_owner_actor_id.as_deref() != Some(run.owner_actor_id.as_str())
        || action.approval_rule_id.as_deref() != action.policy_rule_id.as_deref()
    {
        return Err(StoreError::Corrupt);
    }
    let workspace: String = connection
        .query_row(
            "SELECT workspace_identity FROM projects WHERE project_id = ?1",
            params![run.project_id],
            |row| row.get(0),
        )
        .map_err(|_| StoreError::Corrupt)?;
    if action.approval_workspace_identity.as_deref() != Some(workspace.as_str()) {
        return Err(StoreError::Corrupt);
    }
    let event_id = action
        .approval_event_id
        .as_deref()
        .ok_or(StoreError::Corrupt)?;
    let (kind, payload): (String, String) = connection
        .query_row(
            "SELECT kind, sanitized_payload FROM events
             WHERE run_id = ?1 AND event_id = ?2",
            params![run.run_id.0, event_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(StoreError::Corrupt)?;
    let payload: Value = serde_json::from_str(&payload).map_err(|_| StoreError::Corrupt)?;
    let linked_action_id: Option<String> = connection
        .query_row(
            "SELECT action_id FROM approvals WHERE approval_id = ?1 AND run_id = ?2",
            params![approval_id, run.run_id.0],
            |row| row.get(0),
        )
        .optional()?;
    if linked_action_id.as_deref() != Some(action_id.0.as_str()) {
        return Err(StoreError::Corrupt);
    }
    if approval_event_matches(&payload, &kind, approval_id, action_id, action, run) {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}

fn approval_event_matches(
    payload: &Value,
    kind: &str,
    approval_id: &str,
    action_id: &ActionId,
    action: &ActionRow,
    run: &RunSnapshot,
) -> bool {
    match action.approval_state.as_deref() {
        Some("awaiting") => {
            let request = payload.get("request");
            kind == "approval.requested"
                && request
                    .and_then(|value| value.get("approval_id"))
                    .and_then(Value::as_str)
                    == Some(approval_id)
                && request
                    .and_then(|value| value.get("action_id"))
                    .and_then(Value::as_str)
                    == Some(action_id.0.as_str())
                && request
                    .and_then(|value| value.get("run_id"))
                    .and_then(Value::as_str)
                    == Some(run.run_id.0.as_str())
                && request
                    .and_then(|value| value.get("action_hash"))
                    .and_then(Value::as_str)
                    == Some(action.action_hash.as_str())
                && request
                    .and_then(|value| value.get("workspace_identity"))
                    .and_then(Value::as_str)
                    == action.approval_workspace_identity.as_deref()
                && request
                    .and_then(|value| value.get("policy_snapshot_hash"))
                    .and_then(Value::as_str)
                    == Some(run.policy_snapshot_hash.as_str())
                && request
                    .and_then(|value| value.get("config_snapshot_hash"))
                    .and_then(Value::as_str)
                    == Some(run.config_snapshot_hash.as_str())
                && request
                    .and_then(|value| value.get("rule_id"))
                    .and_then(Value::as_str)
                    == action.policy_rule_id.as_deref()
        }
        Some("approved") => {
            resolved_event_matches(payload, kind, approval_id, &["approved", "reissued"])
        }
        Some("executing") => {
            resolved_event_matches(payload, kind, approval_id, &["executing", "reissued"])
        }
        Some("consumed") => resolved_event_matches(payload, kind, approval_id, &["consumed"]),
        _ => false,
    }
}

fn resolved_event_matches(
    payload: &Value,
    kind: &str,
    approval_id: &str,
    decisions: &[&str],
) -> bool {
    kind == "approval.resolved"
        && payload.get("approval_id").and_then(Value::as_str) == Some(approval_id)
        && payload
            .get("decision")
            .and_then(Value::as_str)
            .is_some_and(|decision| decisions.contains(&decision))
}

pub(super) fn validate_execution_evidence(
    connection: &Connection,
    run: &RunSnapshot,
    step: &StepRow,
    action_id: &ActionId,
    action: &ActionRow,
) -> Result<(), StoreError> {
    match action.policy_decision.as_deref() {
        Some("allow") => {
            require_policy_event(connection, run, step, action_id, action, "allow")?;
            if action.approval_id.is_some() || action.approval_state.is_some() {
                return Err(StoreError::Corrupt);
            }
        }
        Some("ask") => {
            require_policy_event(connection, run, step, action_id, action, "ask")?;
            validate_approval_binding(connection, run, action_id, action)?;
        }
        _ => return Err(StoreError::Corrupt),
    }
    if action.audit_event_id.is_none() {
        return Err(StoreError::Corrupt);
    }
    validate_optional_audit_checkpoint(connection, run, action)
}

pub(super) fn require_completed_transcript(
    connection: &Connection,
    run: &RunSnapshot,
    step: &StepRow,
    codec: &TranscriptCodec,
) -> Result<(), StoreError> {
    let binding = load_model_response_binding(connection, run, step, codec)?;
    if binding.record_count > 0 {
        Ok(())
    } else {
        Err(StoreError::Corrupt)
    }
}
