use orchester_protokoll::{AgentAction, CallId, RunId};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::harness::transcript::{TranscriptCodec, TranscriptRecord};

use super::{
    action_tool_call, record_metadata, transcript_hash, TranscriptBinding, TranscriptBindingPhase,
};
use super::super::{hash_canonical_action, StoreError};

pub(in crate::harness::run_store) fn load_binding(
    connection: &Connection,
    run_id: &RunId,
    event_sequence: i64,
    phase: TranscriptBindingPhase,
    codec: &TranscriptCodec,
) -> Result<Option<TranscriptBinding>, StoreError> {
    type BindingRow = (Option<i64>, Option<i64>, i64, String, Option<String>, String);
    let row: Option<BindingRow> = connection
        .query_row(
            "SELECT b.first_ordinal, b.last_ordinal, b.record_count,
                    e.kind, e.call_id, e.sanitized_payload
             FROM transcript_bindings b
             JOIN events e ON e.run_id = b.run_id AND e.sequence = b.event_sequence
             WHERE b.run_id = ?1 AND b.event_sequence = ?2 AND b.phase = ?3",
            params![run_id.0, event_sequence, phase.as_db()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((first, last, count, event_kind, event_call_id, event_payload)) = row else {
        return Ok(None);
    };
    if !phase.expected_event(&event_kind) {
        return Err(StoreError::Corrupt);
    }
    let event_sequence = u64::try_from(event_sequence).map_err(|_| StoreError::Corrupt)?;
    let record_count = u64::try_from(count).map_err(|_| StoreError::Corrupt)?;
    let first_ordinal = first
        .map(|ordinal| u64::try_from(ordinal).map_err(|_| StoreError::Corrupt))
        .transpose()?;
    let last_ordinal = last
        .map(|ordinal| u64::try_from(ordinal).map_err(|_| StoreError::Corrupt))
        .transpose()?;
    if (record_count == 0) != first_ordinal.is_none()
        || (record_count == 0) != last_ordinal.is_none()
        || (record_count > 0
            && last_ordinal != first_ordinal.and_then(|first| first.checked_add(record_count - 1)))
    {
        return Err(StoreError::Corrupt);
    }
    let binding = TranscriptBinding {
        event_sequence,
        phase,
        first_ordinal,
        last_ordinal,
        record_count,
    };
    validate_binding_records(
        connection,
        run_id,
        &binding,
        &event_kind,
        event_call_id.as_deref(),
        &event_payload,
        codec,
    )?;
    Ok(Some(binding))
}

fn validate_binding_records(
    connection: &Connection,
    run_id: &RunId,
    binding: &TranscriptBinding,
    event_kind: &str,
    event_call_id: Option<&str>,
    event_payload: &str,
    codec: &TranscriptCodec,
) -> Result<(), StoreError> {
    let event_payload: Value =
        serde_json::from_str(event_payload).map_err(|_| StoreError::Corrupt)?;
    if binding.record_count == 0 {
        return if binding.phase == TranscriptBindingPhase::ModelResponse
            && event_payload.get("assistant_text") == Some(&Value::String(String::new()))
        {
            Ok(())
        } else {
            Err(StoreError::Corrupt)
        };
    }
    let first = i64::try_from(binding.first_ordinal.ok_or(StoreError::Corrupt)?)
        .map_err(|_| StoreError::Corrupt)?;
    let last = i64::try_from(binding.last_ordinal.ok_or(StoreError::Corrupt)?)
        .map_err(|_| StoreError::Corrupt)?;
    let records = load_range_records(connection, run_id, first, last, codec)?;
    if records.len() != usize::try_from(binding.record_count).map_err(|_| StoreError::Corrupt)? {
        return Err(StoreError::Corrupt);
    }
    let full_records = load_full_records(connection, run_id, codec)?;
    match binding.phase {
        TranscriptBindingPhase::ModelRequest => {
            if binding.first_ordinal != Some(1) {
                return Err(StoreError::Corrupt);
            }
            codec
                .validate_sequence(&full_records)
                .map_err(|_| StoreError::Corrupt)?;
            codec
                .validate_provider_sequence(&records)
                .map_err(|_| StoreError::Corrupt)
        }
        TranscriptBindingPhase::ModelResponse => match (
            records.as_slice(),
            event_payload.get("assistant_text"),
        ) {
            ([TranscriptRecord::Assistant(text)], Some(Value::String(expected)))
                if text == expected => Ok(()),
            _ => Err(StoreError::Corrupt),
        },
        TranscriptBindingPhase::Action => validate_action_binding(
            connection,
            run_id,
            &records,
            event_kind,
            event_call_id,
            &event_payload,
        ),
        TranscriptBindingPhase::ToolResult => {
            validate_tool_result_binding(&records, event_kind, event_call_id, &event_payload)
        }
    }
}

fn validate_action_binding(
    connection: &Connection,
    run_id: &RunId,
    records: &[TranscriptRecord],
    event_kind: &str,
    event_call_id: Option<&str>,
    event_payload: &Value,
) -> Result<(), StoreError> {
    let [TranscriptRecord::ToolCall { call_id, .. }] = records else {
        return Err(StoreError::Corrupt);
    };
    if event_call_id != Some(call_id.0.as_str()) || event_kind != "action.recorded" {
        return Err(StoreError::Corrupt);
    }
    let action_id = event_payload
        .get("action_id")
        .and_then(Value::as_str)
        .ok_or(StoreError::Corrupt)?;
    let action_value = event_payload
        .get("action")
        .cloned()
        .ok_or(StoreError::Corrupt)?;
    let action: AgentAction =
        serde_json::from_value(action_value).map_err(|_| StoreError::Corrupt)?;
    let expected = action_tool_call(&CallId::from(call_id.0.clone()), &action)?;
    let origin_model_call_id = event_payload
        .get("origin_model_call_id")
        .and_then(Value::as_str)
        .ok_or(StoreError::Corrupt)?;
    let durable: Option<(String, Option<String>, String, String)> = connection
        .query_row(
            "SELECT call_id, origin_model_call_id, canonical_json, action_hash
             FROM actions WHERE run_id = ?1 AND action_id = ?2",
            params![run_id.0, action_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((durable_call_id, durable_origin, canonical_json, action_hash)) = durable else {
        return Err(StoreError::Corrupt);
    };
    let expected_json = serde_json::to_string(&action).map_err(|_| StoreError::Corrupt)?;
    if durable_call_id != call_id.0
        || durable_origin.as_deref() != Some(origin_model_call_id)
        || canonical_json != expected_json
        || hash_canonical_action(&canonical_json) != action_hash
        || expected != records[0]
    {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

fn validate_tool_result_binding(
    records: &[TranscriptRecord],
    event_kind: &str,
    event_call_id: Option<&str>,
    event_payload: &Value,
) -> Result<(), StoreError> {
    let [TranscriptRecord::ToolResultJson { call_id, payload }] = records else {
        return Err(StoreError::Corrupt);
    };
    if event_call_id != Some(call_id.0.as_str())
        || !matches!(event_kind, "tool.completed" | "tool.failed")
    {
        return Err(StoreError::Corrupt);
    }
    let expected = event_payload
        .get(if event_kind == "tool.completed" {
            "observation"
        } else {
            "feedback"
        })
        .ok_or(StoreError::Corrupt)?;
    if payload != expected {
        return Err(StoreError::Corrupt);
    }
    Ok(())
}

fn load_range_records(
    connection: &Connection,
    run_id: &RunId,
    first: i64,
    last: i64,
    codec: &TranscriptCodec,
) -> Result<Vec<TranscriptRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT ordinal, kind, call_id, wire_json, record_hash
         FROM transcript_records
         WHERE run_id = ?1 AND ordinal BETWEEN ?2 AND ?3
         ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![run_id.0, first, last], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut expected = first;
    let mut records = Vec::new();
    for row in rows {
        let (ordinal, kind, call_id, wire, record_hash) = row?;
        let record = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
        let (actual_kind, actual_call_id) = record_metadata(&record);
        if ordinal != expected
            || transcript_hash(&wire) != record_hash
            || kind != actual_kind
            || call_id.as_deref() != actual_call_id
        {
            return Err(StoreError::Corrupt);
        }
        records.push(record);
        expected = expected.checked_add(1).ok_or(StoreError::Corrupt)?;
    }
    Ok(records)
}

fn load_full_records(
    connection: &Connection,
    run_id: &RunId,
    codec: &TranscriptCodec,
) -> Result<Vec<TranscriptRecord>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT ordinal, kind, call_id, wire_json, record_hash
         FROM transcript_records WHERE run_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![run_id.0], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut expected = 1_i64;
    let mut records = Vec::new();
    for row in rows {
        let (ordinal, kind, call_id, wire, record_hash) = row?;
        let record = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
        let (actual_kind, actual_call_id) = record_metadata(&record);
        if ordinal != expected
            || transcript_hash(&wire) != record_hash
            || kind != actual_kind
            || call_id.as_deref() != actual_call_id
        {
            return Err(StoreError::Corrupt);
        }
        records.push(record);
        expected = expected.checked_add(1).ok_or(StoreError::Corrupt)?;
    }
    codec
        .validate_sequence(&records)
        .map_err(|_| StoreError::Corrupt)?;
    Ok(records)
}
