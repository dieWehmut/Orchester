use orchester_protokoll::{
    ActionId, AgentAction, HarnessEvent, HarnessEventKind, RunId, HARNESS_SCHEMA_VERSION,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use crate::harness::governance::{PolicyEngine, PolicyResult};

use super::{
    action_hash, database, ensure_single_update, event_id, persist_event, sanitized, transition,
    EventAppend, RunStatus, SqliteRunStore, StoreError,
};

struct PolicyActionRow {
    canonical_json: String,
    stored_hash: String,
    stored_effect: String,
    state: String,
    existing_decision: Option<String>,
    existing_rule: Option<String>,
    policy_event_id: Option<String>,
    audit_event_id: Option<String>,
    audit_sequence: Option<i64>,
    approval_id: Option<String>,
    origin_model_call_id: Option<String>,
    model_phase: String,
    model_call_id: Option<String>,
}

impl SqliteRunStore {
    /// Calculate and persist the policy decision for a recorded action.
    ///
    /// The action is loaded from the same transaction that writes the policy
    /// event. Callers provide only identity and a boundary timestamp; the
    /// decision and rule are never accepted from caller-controlled payloads.
    pub fn decide_policy(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        occurred_at: impl Into<String>,
    ) -> Result<(HarnessEvent, PolicyResult), StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = decide_in_transaction(
            self,
            &transaction,
            owner_actor_id,
            run_id,
            action_id,
            occurred_at.into(),
        )?;
        transaction.commit()?;
        Ok(result)
    }
}

fn decide_in_transaction(
    store: &SqliteRunStore,
    transaction: &Transaction<'_>,
    owner_actor_id: &str,
    run_id: &RunId,
    action_id: &ActionId,
    occurred_at: String,
) -> Result<(HarnessEvent, PolicyResult), StoreError> {
    let snapshot = database::load_snapshot(transaction, run_id, Some(owner_actor_id))?;
    if snapshot.status != RunStatus::Running {
        return Err(StoreError::Invariant(
            "policy decision requires a running run".into(),
        ));
    }
    if snapshot.policy_snapshot_hash != PolicyEngine::snapshot_hash() {
        return Err(StoreError::Invariant(
            "run policy snapshot does not match the active policy".into(),
        ));
    }
    let step_id = snapshot
        .current_step_id
        .as_ref()
        .ok_or_else(|| StoreError::Invariant("policy decision requires a current step".into()))?;
    if database::current_step_status(transaction, &snapshot)?.as_deref() != Some("action_recorded")
    {
        return Err(StoreError::Invariant(
            "policy decision requires one recorded action".into(),
        ));
    }

    let row: Option<PolicyActionRow> = transaction
        .query_row(
            "SELECT a.canonical_json, a.action_hash, a.effect_class, a.state,
                    a.policy_decision, a.policy_rule_id, a.policy_event_id,
                    a.audit_event_id, a.audit_sequence, ap.approval_id,
                    a.origin_model_call_id, s.model_phase, s.model_call_id
             FROM actions a
             JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
             LEFT JOIN approvals ap ON ap.action_id = a.action_id
             WHERE a.run_id = ?1 AND a.step_id = ?2 AND a.action_id = ?3",
            params![run_id.0, step_id.0, action_id.0],
            |row| {
                Ok(PolicyActionRow {
                    canonical_json: row.get(0)?,
                    stored_hash: row.get(1)?,
                    stored_effect: row.get(2)?,
                    state: row.get(3)?,
                    existing_decision: row.get(4)?,
                    existing_rule: row.get(5)?,
                    policy_event_id: row.get(6)?,
                    audit_event_id: row.get(7)?,
                    audit_sequence: row.get(8)?,
                    approval_id: row.get(9)?,
                    origin_model_call_id: row.get(10)?,
                    model_phase: row.get(11)?,
                    model_call_id: row.get(12)?,
                })
            },
        )
        .optional()?;
    let Some(PolicyActionRow {
        canonical_json,
        stored_hash,
        stored_effect,
        state,
        existing_decision,
        existing_rule,
        policy_event_id,
        audit_event_id,
        audit_sequence,
        approval_id,
        origin_model_call_id,
        model_phase,
        model_call_id,
    }) = row
    else {
        return Err(StoreError::NotFound);
    };
    if state != "recorded"
        || existing_decision.is_some()
        || existing_rule.is_some()
        || policy_event_id.is_some()
        || audit_event_id.is_some()
        || audit_sequence.is_some()
        || approval_id.is_some()
    {
        return Err(StoreError::Invariant(
            "action already has a policy decision".into(),
        ));
    }
    if model_phase != "completed"
        || origin_model_call_id.is_none()
        || model_call_id.is_none()
        || origin_model_call_id.as_deref() != model_call_id.as_deref()
    {
        return Err(StoreError::Invariant(
            "policy action is not bound to a completed model call".into(),
        ));
    }

    let action: AgentAction =
        serde_json::from_str(&canonical_json).map_err(|_| StoreError::Corrupt)?;
    if serde_json::to_string(&action).map_err(|_| StoreError::Corrupt)? != canonical_json
        || action_hash(&action)? != stored_hash
    {
        return Err(StoreError::Corrupt);
    }
    let result = PolicyEngine::new()
        .evaluate(&action)
        .map_err(|_| StoreError::Invariant("action policy classification failed".into()))?;
    if stored_effect != result.effect.as_db() {
        return Err(StoreError::Corrupt);
    }
    sanitized::ensure_durable_field(
        "policy decision timestamp",
        &occurred_at,
        &store.event_sanitizer,
    )?;

    let input = sanitized::canonicalize_input(
        EventAppend {
            turn_id: snapshot.current_turn_id.clone(),
            step_id: Some(step_id.clone()),
            call_id: None,
            occurred_at,
            kind: HarnessEventKind::PolicyDecided {
                action_id: action_id.clone(),
                decision: result.decision,
                rule_id: result.rule_id.clone(),
            },
        },
        &store.event_sanitizer,
    )?;
    transition::apply_event_transition(transaction, &snapshot, &input, None, None)?;
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
    persist_event(transaction, &event)?;
    let updated = transaction.execute(
        "UPDATE actions SET policy_event_id = ?1
         WHERE run_id = ?2 AND action_id = ?3 AND policy_event_id IS NULL",
        params![event.event_id.0, run_id.0, action_id.0],
    )?;
    ensure_single_update(updated)?;
    let advanced = transaction.execute(
        "UPDATE runs SET next_sequence = ?1, row_version = row_version + 1,
           updated_at = ?2 WHERE run_id = ?3 AND row_version = ?4",
        params![
            snapshot.next_sequence + 1,
            event.occurred_at,
            run_id.0,
            snapshot.row_version,
        ],
    )?;
    ensure_single_update(advanced)?;
    Ok((event, result))
}
