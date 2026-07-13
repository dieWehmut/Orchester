use orchester_protokoll::{AgentAction, CallId, HarnessEvent, HarnessEventKind, RunId};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::harness::feedback::FeedbackEngine;
use crate::harness::transcript::{
    TranscriptCodec, TranscriptError, TranscriptLimits, TranscriptRecord,
};

use super::{database::load_snapshot, map_constraint, EventAppend, SqliteRunStore, StoreError};

mod binding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTranscriptRecord {
    pub ordinal: u64,
    pub record: TranscriptRecord,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptAppendRange {
    pub first_ordinal: u64,
    pub last_ordinal: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptBindingPhase {
    ModelRequest,
    ModelResponse,
    Action,
    ToolResult,
}

impl TranscriptBindingPhase {
    fn as_db(self) -> &'static str {
        match self {
            Self::ModelRequest => "model_request",
            Self::ModelResponse => "model_response",
            Self::Action => "action",
            Self::ToolResult => "tool_result",
        }
    }

    fn expected_event(self, kind: &str) -> bool {
        match self {
            Self::ModelRequest => kind == "model.started",
            Self::ModelResponse => kind == "model.completed",
            Self::Action => kind == "action.recorded",
            Self::ToolResult => matches!(kind, "tool.completed" | "tool.failed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptBinding {
    pub event_sequence: u64,
    pub phase: TranscriptBindingPhase,
    pub first_ordinal: Option<u64>,
    pub last_ordinal: Option<u64>,
    pub record_count: u64,
}

struct CanonicalRecord {
    kind: &'static str,
    call_id: Option<String>,
    wire: String,
    record_hash: String,
}

impl SqliteRunStore {
    pub fn transcript_binding_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        event_sequence: u64,
        phase: TranscriptBindingPhase,
    ) -> Result<Option<TranscriptBinding>, StoreError> {
        ensure_field(owner_actor_id, &self.event_sanitizer)?;
        let sequence = i64::try_from(event_sequence).map_err(|_| StoreError::Corrupt)?;
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, Some(owner_actor_id))?;
        load_binding(&connection, run_id, sequence, phase, &self.codec())
    }

    pub fn append_model_started_with_transcript(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        input: EventAppend,
        records: Vec<TranscriptRecord>,
    ) -> Result<HarnessEvent, StoreError> {
        if !matches!(input.kind, HarnessEventKind::ModelStarted) {
            return Err(StoreError::Invariant(
                "request transcript requires a model-start event".into(),
            ));
        }
        self.append_event_internal(owner_actor_id, run_id, input, None, None, Some(&records))
            .map(|(event, _)| event)
    }

    pub fn append_transcript_record(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        record: TranscriptRecord,
        created_at: impl Into<String>,
    ) -> Result<u64, StoreError> {
        self.append_transcript_records(owner_actor_id, run_id, vec![record], created_at)
            .map(|range| range.first_ordinal)
    }

    pub fn append_transcript_records(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        records: Vec<TranscriptRecord>,
        created_at: impl Into<String>,
    ) -> Result<TranscriptAppendRange, StoreError> {
        let created_at = created_at.into();
        ensure_field(owner_actor_id, &self.event_sanitizer)?;

        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        if snapshot.status.is_terminal() {
            return Err(StoreError::Invariant(
                "terminal run cannot append a transcript record".into(),
            ));
        }
        let appended = append_records_in_transaction(
            &transaction,
            run_id,
            &records,
            &created_at,
            &self.event_sanitizer,
        )?;
        transaction.commit()?;
        Ok(appended)
    }

    pub fn transcript_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
    ) -> Result<Vec<StoredTranscriptRecord>, StoreError> {
        ensure_field(owner_actor_id, &self.event_sanitizer)?;
        let connection = self.connection()?;
        load_snapshot(&connection, run_id, Some(owner_actor_id))?;
        let codec = self.codec();
        let mut statement = connection.prepare(
            "SELECT ordinal, kind, call_id, wire_json, record_hash, created_at
             FROM transcript_records WHERE run_id = ?1 ORDER BY ordinal",
        )?;
        let rows = statement.query_map(params![run_id.0], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let mut expected = 1_u64;
        let mut records = Vec::new();
        for row in rows {
            let (ordinal, kind, call_id, wire, record_hash, created_at) = row?;
            let ordinal = u64::try_from(ordinal).map_err(|_| StoreError::Corrupt)?;
            if ordinal != expected {
                return Err(StoreError::Corrupt);
            }
            let record = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
            if transcript_hash(&wire) != record_hash {
                return Err(StoreError::Corrupt);
            }
            let (actual_kind, actual_call_id) = record_metadata(&record);
            if kind != actual_kind || call_id.as_deref() != actual_call_id {
                return Err(StoreError::Corrupt);
            }
            records.push(StoredTranscriptRecord {
                ordinal,
                record,
                created_at,
            });
            expected = expected.checked_add(1).ok_or(StoreError::Corrupt)?;
        }
        let sequence = records
            .iter()
            .map(|stored| stored.record.clone())
            .collect::<Vec<_>>();
        codec
            .validate_sequence(&sequence)
            .map_err(|_| StoreError::Corrupt)?;
        Ok(records)
    }

    fn codec(&self) -> TranscriptCodec {
        TranscriptCodec::with_sanitizer(TranscriptLimits::default(), self.event_sanitizer.clone())
    }
}

pub(super) fn bind_transcript_range_in_transaction(
    transaction: &Transaction<'_>,
    run_id: &RunId,
    event_sequence: u64,
    phase: TranscriptBindingPhase,
    range: Option<TranscriptAppendRange>,
) -> Result<TranscriptBinding, StoreError> {
    let event_sequence = i64::try_from(event_sequence).map_err(|_| StoreError::Corrupt)?;
    let event_kind: Option<String> = transaction
        .query_row(
            "SELECT kind FROM events WHERE run_id = ?1 AND sequence = ?2",
            params![run_id.0, event_sequence],
            |row| row.get(0),
        )
        .optional()?;
    let event_kind = event_kind.ok_or(StoreError::Corrupt)?;
    if !phase.expected_event(&event_kind) {
        return Err(StoreError::Invariant(
            "transcript binding phase does not match lifecycle event".into(),
        ));
    }

    let (first_ordinal, last_ordinal, record_count) = if let Some(range) = range {
        if range.first_ordinal == 0 || range.last_ordinal < range.first_ordinal {
            return Err(StoreError::Corrupt);
        }
        let count = range
            .last_ordinal
            .checked_sub(range.first_ordinal)
            .and_then(|span| span.checked_add(1))
            .ok_or(StoreError::Corrupt)?;
        let first = i64::try_from(range.first_ordinal).map_err(|_| StoreError::Corrupt)?;
        let last = i64::try_from(range.last_ordinal).map_err(|_| StoreError::Corrupt)?;
        let actual: (i64, Option<i64>, Option<i64>) = transaction.query_row(
            "SELECT COUNT(*), MIN(ordinal), MAX(ordinal)
             FROM transcript_records
             WHERE run_id = ?1 AND ordinal BETWEEN ?2 AND ?3",
            params![run_id.0, first, last],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if actual.0 != i64::try_from(count).map_err(|_| StoreError::Corrupt)?
            || actual.1 != Some(first)
            || actual.2 != Some(last)
        {
            return Err(StoreError::Corrupt);
        }
        (Some(first), Some(last), count)
    } else {
        (None, None, 0)
    };

    transaction
        .execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id.0,
                event_sequence,
                phase.as_db(),
                first_ordinal,
                last_ordinal,
                i64::try_from(record_count).map_err(|_| StoreError::Corrupt)?,
            ],
        )
        .map_err(|error| map_constraint(error, "transcript binding already exists"))?;

    Ok(TranscriptBinding {
        event_sequence: u64::try_from(event_sequence).map_err(|_| StoreError::Corrupt)?,
        phase,
        first_ordinal: first_ordinal
            .map(|ordinal| u64::try_from(ordinal).map_err(|_| StoreError::Corrupt))
            .transpose()?,
        last_ordinal: last_ordinal
            .map(|ordinal| u64::try_from(ordinal).map_err(|_| StoreError::Corrupt))
            .transpose()?,
        record_count,
    })
}

pub(super) fn current_transcript_range_in_transaction(
    transaction: &Transaction<'_>,
    run_id: &RunId,
) -> Result<Option<TranscriptAppendRange>, StoreError> {
    let (count, first, last): (i64, Option<i64>, Option<i64>) = transaction.query_row(
        "SELECT COUNT(*), MIN(ordinal), MAX(ordinal)
         FROM transcript_records WHERE run_id = ?1",
        params![run_id.0],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if count == 0 {
        if first.is_some() || last.is_some() {
            return Err(StoreError::Corrupt);
        }
        return Ok(None);
    }
    let count = u64::try_from(count).map_err(|_| StoreError::Corrupt)?;
    let first = u64::try_from(first.ok_or(StoreError::Corrupt)?).map_err(|_| StoreError::Corrupt)?;
    let last = u64::try_from(last.ok_or(StoreError::Corrupt)?).map_err(|_| StoreError::Corrupt)?;
    if last.checked_sub(first).and_then(|span| span.checked_add(1)) != Some(count) {
        return Err(StoreError::Corrupt);
    }
    Ok(Some(TranscriptAppendRange {
        first_ordinal: first,
        last_ordinal: last,
    }))
}

pub(super) use binding::load_binding;

pub(super) fn append_records_in_transaction(
    transaction: &Transaction<'_>,
    run_id: &RunId,
    records: &[TranscriptRecord],
    created_at: &str,
    sanitizer: &FeedbackEngine,
) -> Result<TranscriptAppendRange, StoreError> {
    if records.is_empty() {
        return Err(StoreError::Invariant(
            "transcript batch must contain at least one record".into(),
        ));
    }
    ensure_field(&run_id.0, sanitizer)?;
    ensure_field(created_at, sanitizer)?;
    let codec = TranscriptCodec::with_sanitizer(TranscriptLimits::default(), sanitizer.clone());
    let canonical = records
        .iter()
        .map(|record| canonical_record(&codec, record))
        .collect::<Result<Vec<_>, _>>()?;
    let prior_wires = load_prior_wires(transaction, run_id, &codec)?;
    codec
        .decode_all(&prior_wires)
        .map_err(|_| StoreError::Corrupt)?;
    let mut sequence_wires = prior_wires;
    sequence_wires.extend(canonical.iter().map(|record| record.wire.clone()));
    codec.decode_all(&sequence_wires).map_err(map_input_error)?;
    let first_ordinal = u64::try_from(sequence_wires.len() - canonical.len() + 1)
        .map_err(|_| StoreError::Corrupt)?;
    for (offset, record) in canonical.iter().enumerate() {
        let ordinal = first_ordinal
            .checked_add(u64::try_from(offset).map_err(|_| StoreError::Corrupt)?)
            .ok_or(StoreError::Corrupt)?;
        transaction
            .execute(
                "INSERT INTO transcript_records(
                   run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run_id.0,
                    ordinal,
                    record.kind,
                    record.call_id,
                    record.wire,
                    record.record_hash,
                    created_at,
                ],
            )
            .map_err(|error| {
                map_constraint(error, "transcript record conflicts with durable state")
            })?;
    }
    let last_ordinal = first_ordinal
        .checked_add(u64::try_from(canonical.len() - 1).map_err(|_| StoreError::Corrupt)?)
        .ok_or(StoreError::Corrupt)?;
    Ok(TranscriptAppendRange {
        first_ordinal,
        last_ordinal,
    })
}

pub(super) fn validate_provider_records_in_transaction(
    transaction: &Transaction<'_>,
    run_id: &RunId,
    sanitizer: &FeedbackEngine,
) -> Result<usize, StoreError> {
    let codec = TranscriptCodec::with_sanitizer(TranscriptLimits::default(), sanitizer.clone());
    let wires = load_prior_wires(transaction, run_id, &codec)?;
    let records = codec
        .decode_all(&wires)
        .map_err(|_| StoreError::Corrupt)?;
    codec
        .validate_provider_sequence(&records)
        .map_err(|_| {
            StoreError::Invariant("model start requires a closed request transcript".into())
        })?;
    Ok(records.len())
}

pub(super) fn action_tool_call(
    call_id: &CallId,
    action: &AgentAction,
) -> Result<TranscriptRecord, StoreError> {
    let mut value = serde_json::to_value(action)?;
    let Value::Object(fields) = &mut value else {
        return Err(StoreError::Invariant(
            "durable action is not an object".into(),
        ));
    };
    fields.remove("tool");
    let arguments_json = serde_json::to_string(&value)?;
    Ok(TranscriptRecord::tool_call(
        call_id.clone(),
        super::action_kind(action),
        arguments_json,
    ))
}

fn canonical_record(
    codec: &TranscriptCodec,
    record: &TranscriptRecord,
) -> Result<CanonicalRecord, StoreError> {
    let wire = codec.encode(record).map_err(map_input_error)?;
    let canonical = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
    let (kind, call_id) = record_metadata(&canonical);
    Ok(CanonicalRecord {
        kind,
        call_id: call_id.map(str::to_owned),
        record_hash: transcript_hash(&wire),
        wire,
    })
}

fn load_prior_wires(
    transaction: &Transaction<'_>,
    run_id: &RunId,
    codec: &TranscriptCodec,
) -> Result<Vec<String>, StoreError> {
    let mut statement = transaction.prepare(
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
    let mut wires = Vec::new();
    for (position, row) in rows.enumerate() {
        let (ordinal, kind, call_id, wire, record_hash) = row?;
        let expected = i64::try_from(position + 1).map_err(|_| StoreError::Corrupt)?;
        let record = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
        let (actual_kind, actual_call_id) = record_metadata(&record);
        if ordinal != expected
            || transcript_hash(&wire) != record_hash
            || kind != actual_kind
            || call_id.as_deref() != actual_call_id
        {
            return Err(StoreError::Corrupt);
        }
        wires.push(wire);
    }
    Ok(wires)
}

fn record_metadata(record: &TranscriptRecord) -> (&'static str, Option<&str>) {
    match record {
        TranscriptRecord::System(_) => ("system", None),
        TranscriptRecord::User(_) => ("user", None),
        TranscriptRecord::Assistant(_) => ("assistant", None),
        TranscriptRecord::ToolCall { call_id, .. } => ("tool_call", Some(call_id.0.as_str())),
        TranscriptRecord::ToolResult { call_id, .. }
        | TranscriptRecord::ToolResultJson { call_id, .. } => {
            ("tool_result", Some(call_id.0.as_str()))
        }
        TranscriptRecord::Opaque { .. } => ("opaque", None),
    }
}

fn transcript_hash(wire: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-transcript-record-v1");
    hasher.update((wire.len() as u64).to_be_bytes());
    hasher.update(wire.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn ensure_field(value: &str, sanitizer: &FeedbackEngine) -> Result<(), StoreError> {
    if value.is_empty() || value.len() > 512 || sanitizer.sanitize_text(value) != value {
        Err(StoreError::Invariant(
            "transcript metadata is not eligible for durable persistence".into(),
        ))
    } else {
        Ok(())
    }
}

fn map_input_error(error: TranscriptError) -> StoreError {
    match error {
        TranscriptError::InvalidWire
        | TranscriptError::InvalidIdentifier
        | TranscriptError::NonCanonical
        | TranscriptError::TextTooLarge
        | TranscriptError::RecordTooLarge
        | TranscriptError::OpaqueTooLarge
        | TranscriptError::InvalidLimits
        | TranscriptError::UnpairedToolResult
        | TranscriptError::UnpairedToolCall => {
            StoreError::Invariant("transcript record is invalid or exceeds durable limits".into())
        }
    }
}
