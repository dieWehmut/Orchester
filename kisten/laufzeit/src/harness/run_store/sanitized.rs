use orchester_protokoll::{AgentAction, HarnessEventKind};
use serde_json::Value;

use super::{EventAppend, StoreError};
use crate::harness::feedback::{FeedbackClass, FeedbackEngine, FeedbackInput};

const MAX_MODEL_TEXT_BYTES: usize = 65_536;

pub(super) fn canonicalize_input(
    input: EventAppend,
    sanitizer: &FeedbackEngine,
) -> Result<EventAppend, StoreError> {
    let kind = match input.kind {
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
        kind => kind,
    };
    Ok(EventAppend { kind, ..input })
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
