use orchester_protokoll::{AgentAction, HarnessEventKind};
use serde_json::Value;

use super::{EventAppend, StoreError};
use crate::harness::feedback::FeedbackEngine;

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
        kind => kind,
    };
    Ok(EventAppend { kind, ..input })
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
