use orchester_protokoll::{HarnessEvent, HarnessEventKind, RunId, HARNESS_SCHEMA_VERSION};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use crate::harness::transcript::TranscriptRecord;

use super::{
    action_kind, database::load_snapshot, ensure_single_update, event_id, hash_canonical_action,
    observation, persist_event, sanitized, transcript, transition, ActionRecord, EventAppend,
    RunSnapshot, RunStatus, SqliteRunStore, StoreError,
};

impl SqliteRunStore {
    pub fn append_model_completed_with_action(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        action: ActionRecord,
    ) -> Result<(HarnessEvent, HarnessEvent), StoreError> {
        if !matches!(input.kind, HarnessEventKind::ModelCompleted { .. }) {
            return Err(StoreError::Invariant(
                "combined model action requires a model-completion event".into(),
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        if snapshot.status.is_terminal() {
            return Err(StoreError::Invariant(
                "terminal run cannot complete a model action".into(),
            ));
        }
        let (input, terminal_observation) =
            observation::prepare_terminal_input(run_id, input, self.terminal_sanitizer.as_ref())?;
        if terminal_observation.is_some() {
            return Err(StoreError::Invariant(
                "model completion cannot contain a terminal observation".into(),
            ));
        }
        let input = sanitized::canonicalize_input(input, &self.event_sanitizer)?;
        transition::apply_event_transition(&transaction, &snapshot, &input, None, None)?;
        let model_event = HarnessEvent {
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
        let HarnessEventKind::ModelCompleted { assistant_text } = &model_event.kind else {
            return Err(StoreError::Invariant(
                "combined model action requires a model-completion event".into(),
            ));
        };
        if !assistant_text.is_empty() {
            transcript::append_records_in_transaction(
                &transaction,
                run_id,
                &[TranscriptRecord::assistant(assistant_text.clone())],
                &model_event.occurred_at,
                &self.event_sanitizer,
            )?;
        }
        persist_event(&transaction, &model_event)?;
        let advanced = transaction.execute(
            "UPDATE runs SET next_sequence = ?1, row_version = row_version + 1,
               updated_at = ?2 WHERE run_id = ?3 AND row_version = ?4",
            params![
                snapshot.next_sequence + 1,
                model_event.occurred_at,
                run_id.0,
                snapshot.row_version,
            ],
        )?;
        ensure_single_update(advanced)?;
        let action_snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        let action_event = self.record_action_in_transaction(
            &transaction,
            owner_actor_id,
            action,
            &action_snapshot,
        )?;
        transaction.commit()?;
        Ok((model_event, action_event))
    }

    pub(super) fn record_action_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        owner_actor_id: &str,
        action: ActionRecord,
        snapshot: &RunSnapshot,
    ) -> Result<HarnessEvent, StoreError> {
        if snapshot.owner_actor_id != owner_actor_id || snapshot.run_id != action.run_id {
            return Err(StoreError::NotFound);
        }
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
        let transcript_record = transcript::action_tool_call(&action.call_id, &action.action)?;
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
        transcript::append_records_in_transaction(
            transaction,
            &action.run_id,
            &[transcript_record],
            &action.occurred_at,
            &self.event_sanitizer,
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
        persist_event(transaction, &event)?;
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
        Ok(event)
    }
}
