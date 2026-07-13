use orchester_protokoll::RunId;
use rusqlite::{params, TransactionBehavior};
use sha2::{Digest, Sha256};

use crate::harness::feedback::FeedbackEngine;
use crate::harness::transcript::{
    TranscriptCodec, TranscriptError, TranscriptLimits, TranscriptRecord,
};

use super::{database::load_snapshot, map_constraint, SqliteRunStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTranscriptRecord {
    pub ordinal: u64,
    pub record: TranscriptRecord,
    pub created_at: String,
}

impl SqliteRunStore {
    pub fn append_transcript_record(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        record: TranscriptRecord,
        created_at: impl Into<String>,
    ) -> Result<u64, StoreError> {
        let created_at = created_at.into();
        ensure_field(owner_actor_id, &self.event_sanitizer)?;
        ensure_field(&run_id.0, &self.event_sanitizer)?;
        ensure_field(&created_at, &self.event_sanitizer)?;

        let codec = self.codec();
        let wire = codec.encode(&record).map_err(map_input_error)?;
        let canonical = codec.decode(&wire).map_err(|_| StoreError::Corrupt)?;
        let (kind, call_id) = record_metadata(&canonical);
        let record_hash = transcript_hash(&wire);

        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot = load_snapshot(&transaction, run_id, Some(owner_actor_id))?;
        if snapshot.status.is_terminal() {
            return Err(StoreError::Invariant(
                "terminal run cannot append a transcript record".into(),
            ));
        }
        let mut prior_wires = transaction
            .prepare(
                "SELECT ordinal, wire_json FROM transcript_records
                 WHERE run_id = ?1 ORDER BY ordinal",
            )?
            .query_map(params![run_id.0], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (position, (ordinal, _)) in prior_wires.iter().enumerate() {
            let expected = i64::try_from(position + 1).map_err(|_| StoreError::Corrupt)?;
            if *ordinal != expected {
                return Err(StoreError::Corrupt);
            }
        }
        let mut sequence_wires = prior_wires
            .drain(..)
            .map(|(_, wire)| wire)
            .collect::<Vec<_>>();
        sequence_wires.push(wire.clone());
        codec.decode_all(&sequence_wires).map_err(map_input_error)?;
        let ordinal: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(ordinal), 0) + 1
             FROM transcript_records WHERE run_id = ?1",
            params![run_id.0],
            |row| row.get(0),
        )?;
        let ordinal = u64::try_from(ordinal).map_err(|_| StoreError::Corrupt)?;
        transaction
            .execute(
                "INSERT INTO transcript_records(
                   run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    run_id.0,
                    ordinal,
                    kind,
                    call_id,
                    wire,
                    record_hash,
                    created_at,
                ],
            )
            .map_err(|error| {
                map_constraint(error, "transcript record conflicts with durable state")
            })?;
        transaction.commit()?;
        Ok(ordinal)
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
        Ok(records)
    }

    fn codec(&self) -> TranscriptCodec {
        TranscriptCodec::with_sanitizer(TranscriptLimits::default(), self.event_sanitizer.clone())
    }
}

fn record_metadata(record: &TranscriptRecord) -> (&'static str, Option<&str>) {
    match record {
        TranscriptRecord::System(_) => ("system", None),
        TranscriptRecord::User(_) => ("user", None),
        TranscriptRecord::Assistant(_) => ("assistant", None),
        TranscriptRecord::ToolCall { call_id, .. } => ("tool_call", Some(call_id.0.as_str())),
        TranscriptRecord::ToolResult { call_id, .. } => ("tool_result", Some(call_id.0.as_str())),
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
