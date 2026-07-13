use orchester_protokoll::{normalize_action_summary, AgentAction, HarnessEventKind};
use serde_json::Value;

use super::{EventAppend, StoreError};
use crate::harness::feedback::{FeedbackClass, FeedbackEngine, FeedbackInput};

const MAX_MODEL_TEXT_BYTES: usize = 65_536;
const MAX_ENVELOPE_FIELD_BYTES: usize = 512;

pub(super) fn canonicalize_input(
    input: EventAppend,
    sanitizer: &FeedbackEngine,
) -> Result<EventAppend, StoreError> {
    if let Some(turn_id) = &input.turn_id {
        ensure_durable_field("turn identifier", &turn_id.0, sanitizer)?;
    }
    if let Some(step_id) = &input.step_id {
        ensure_durable_field("step identifier", &step_id.0, sanitizer)?;
    }
    if let Some(call_id) = &input.call_id {
        ensure_durable_field("call identifier", &call_id.0, sanitizer)?;
    }
    ensure_durable_field("event timestamp", &input.occurred_at, sanitizer)?;
    let kind = canonicalize_kind(input.kind, sanitizer)?;
    Ok(EventAppend { kind, ..input })
}

pub(super) fn ensure_durable_field(
    field: &str,
    value: &str,
    sanitizer: &FeedbackEngine,
) -> Result<(), StoreError> {
    if value.is_empty()
        || value.len() > MAX_ENVELOPE_FIELD_BYTES
        || sanitizer.sanitize_text(value) != value
    {
        Err(StoreError::Invariant(format!(
            "{field} is not eligible for durable persistence"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn canonicalize_kind(
    kind: HarnessEventKind,
    sanitizer: &FeedbackEngine,
) -> Result<HarnessEventKind, StoreError> {
    Ok(match kind {
        HarnessEventKind::ModelCompleted { assistant_text } => {
            let assistant_text = sanitizer.sanitize_text(&assistant_text);
            if assistant_text.len() > MAX_MODEL_TEXT_BYTES {
                return Err(StoreError::Invariant(
                    "model completion text exceeds the durable limit".into(),
                ));
            }
            HarnessEventKind::ModelCompleted { assistant_text }
        }
        HarnessEventKind::ValidatorCompleted { feedback } => HarnessEventKind::ValidatorCompleted {
            feedback: sanitize_validator_feedback(&feedback, sanitizer),
        },
        HarnessEventKind::RunCompleted { reason, summary } => {
            let summary = canonicalize_summary(&summary, sanitizer);
            if summary.len() > MAX_MODEL_TEXT_BYTES {
                return Err(StoreError::Invariant(
                    "run completion summary exceeds the durable limit".into(),
                ));
            }
            HarnessEventKind::RunCompleted { reason, summary }
        }
        kind => kind,
    })
}

pub(super) fn canonicalize_summary(input: &str, sanitizer: &FeedbackEngine) -> String {
    normalize_action_summary(&sanitizer.sanitize_text(input))
}

fn sanitize_validator_feedback(
    feedback: &orchester_protokoll::FeedbackReport,
    sanitizer: &FeedbackEngine,
) -> orchester_protokoll::FeedbackReport {
    sanitizer
        .build(FeedbackInput {
            source: feedback.source.clone(),
            validator_id: feedback.validator_id.clone(),
            exit_code: feedback.exit_code,
            class: validator_feedback_class(&feedback.classification),
            summary: feedback.summary.clone(),
            stdout: feedback.stdout_tail.clone(),
            stderr: feedback.stderr_tail.clone(),
            retryable: feedback.retryable,
        })
        .report
}

fn validator_feedback_class(classification: &str) -> FeedbackClass {
    match classification {
        "validator_passed" => FeedbackClass::ValidatorPassed,
        "validator_mutated_sources" => FeedbackClass::ValidatorMutatedSources,
        "validator_output_truncated" => FeedbackClass::ValidatorOutputTruncated,
        "snapshot_limit_exceeded" => FeedbackClass::SnapshotLimitExceeded,
        "process_cancelled" => FeedbackClass::ProcessCancelled,
        "process_timed_out" => FeedbackClass::ProcessTimedOut,
        "process_spawn_failed" => FeedbackClass::ProcessSpawnFailed,
        _ => FeedbackClass::ValidatorFailed,
    }
}

pub(super) fn durable_action_json(
    action: &AgentAction,
    sanitizer: &FeedbackEngine,
) -> Result<String, StoreError> {
    let canonical_json = serde_json::to_string(action)?;
    let value = serde_json::to_value(action)?;
    let sanitized = sanitize_value(value.clone(), sanitizer);
    if sanitized != value {
        return Err(StoreError::Invariant(
            "action contains data that is not eligible for durable persistence".into(),
        ));
    }
    Ok(canonical_json)
}

fn sanitize_value(value: Value, sanitizer: &FeedbackEngine) -> Value {
    match value {
        Value::String(text) => Value::String(sanitizer.sanitize_text(&text)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| sanitize_value(value, sanitizer))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, sanitize_value(value, sanitizer)))
                .collect(),
        ),
        other => other,
    }
}
