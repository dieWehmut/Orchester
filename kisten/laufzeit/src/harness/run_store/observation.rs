use std::collections::HashSet;

use orchester_protokoll::{
    CallId, FeedbackReport, HarnessEventKind, Observation, ObservationId, RunId,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{EventAppend, StoreError};
use crate::harness::feedback::{FeedbackClass, FeedbackEngine, FeedbackInput};

const MAX_PAYLOAD_BYTES: usize = 65_536;
const MAX_DEPTH: usize = 24;
const MAX_NODES: usize = 2_048;
const MAX_AGGREGATE_TEXT_BYTES: usize = 24 * 1024;
const MAX_KEY_BYTES: usize = 512;
const MAX_STRING_BYTES: usize = 16 * 1024;
const MAX_KIND_BYTES: usize = 128;
const MAX_SUMMARY_BYTES: usize = 8 * 1024;
const MAX_OBSERVATION_ID_BYTES: usize = 512;

pub(super) struct DurableObservation {
    pub(super) observation_id: ObservationId,
    pub(super) call_id: CallId,
    pub(super) kind: &'static str,
    pub(super) payload: String,
    pub(super) fingerprint: String,
    pub(super) outcome: &'static str,
}

pub(super) fn prepare_terminal_input(
    run_id: &RunId,
    mut input: EventAppend,
    sanitizer: Option<&FeedbackEngine>,
) -> Result<(EventAppend, Option<DurableObservation>), StoreError> {
    let prepared = match &input.kind {
        HarnessEventKind::ToolCompleted { observation } => {
            let sanitizer = sanitizer.ok_or_else(|| {
                StoreError::Invariant(
                    "terminal events require explicit secret-aware sanitization".into(),
                )
            })?;
            let call_id = input.call_id.as_ref().ok_or_else(|| {
                StoreError::Invariant("tool completion requires a call identifier".into())
            })?;
            if observation.call_id != *call_id {
                return Err(StoreError::Invariant(
                    "tool completion observation call does not match its event".into(),
                ));
            }
            validate_call_id(call_id)?;
            let sanitized = sanitize_observation(observation, sanitizer)?;
            let payload = bounded_json(&sanitized)?;
            let durable = DurableObservation {
                observation_id: sanitized.observation_id.clone(),
                call_id: sanitized.call_id.clone(),
                kind: "tool.completed",
                fingerprint: fingerprint(&payload),
                payload,
                outcome: "completed",
            };
            Some((
                HarnessEventKind::ToolCompleted {
                    observation: sanitized,
                },
                durable,
            ))
        }
        HarnessEventKind::ToolFailed { feedback } => {
            let sanitizer = sanitizer.ok_or_else(|| {
                StoreError::Invariant(
                    "terminal events require explicit secret-aware sanitization".into(),
                )
            })?;
            let call_id = input.call_id.as_ref().ok_or_else(|| {
                StoreError::Invariant("tool failure requires a call identifier".into())
            })?;
            validate_call_id(call_id)?;
            let (sanitized, payload) = sanitize_feedback(feedback, sanitizer)?;
            let durable = DurableObservation {
                observation_id: failed_observation_id(run_id, call_id),
                call_id: call_id.clone(),
                kind: "tool.failed",
                fingerprint: sanitized.fingerprint.clone(),
                payload,
                outcome: "failed",
            };
            Some((
                HarnessEventKind::ToolFailed {
                    feedback: sanitized,
                },
                durable,
            ))
        }
        _ => None,
    };

    if let Some((kind, durable)) = prepared {
        input.kind = kind;
        Ok((input, Some(durable)))
    } else {
        Ok((input, None))
    }
}

fn sanitize_observation(
    observation: &Observation,
    sanitizer: &FeedbackEngine,
) -> Result<Observation, StoreError> {
    validate_observation_id(&observation.observation_id)?;
    let kind = sanitized_or_fallback(&observation.kind, sanitizer, MAX_KIND_BYTES, "unknown");
    let summary = sanitized_or_fallback(
        &observation.summary,
        sanitizer,
        MAX_SUMMARY_BYTES,
        "[truncated: observation summary exceeded limit]",
    );
    let mut budget = DataBudget::default();
    let data = sanitize_value(&observation.data, sanitizer, 0, &mut budget)
        .unwrap_or_else(|_| truncation_sentinel());
    Ok(Observation {
        observation_id: observation.observation_id.clone(),
        call_id: observation.call_id.clone(),
        kind,
        summary,
        data,
    })
}

fn sanitize_feedback(
    feedback: &FeedbackReport,
    sanitizer: &FeedbackEngine,
) -> Result<(FeedbackReport, String), StoreError> {
    let raw_bytes = [
        feedback.source.len(),
        feedback.validator_id.as_deref().map_or(0, str::len),
        feedback.summary.len(),
        feedback.stdout_tail.len(),
        feedback.stderr_tail.len(),
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add);
    let input = if raw_bytes.is_some_and(|bytes| bytes <= MAX_PAYLOAD_BYTES) {
        FeedbackInput {
            source: feedback.source.clone(),
            validator_id: feedback.validator_id.clone(),
            exit_code: feedback.exit_code,
            class: FeedbackClass::ToolFailed,
            summary: feedback.summary.clone(),
            stdout: feedback.stdout_tail.clone(),
            stderr: feedback.stderr_tail.clone(),
            retryable: feedback.retryable,
        }
    } else {
        FeedbackInput {
            source: "tool".into(),
            validator_id: None,
            exit_code: feedback.exit_code,
            class: FeedbackClass::ToolFailed,
            summary: "tool feedback exceeded durable limits".into(),
            stdout: String::new(),
            stderr: String::new(),
            retryable: feedback.retryable,
        }
    };
    let sanitized = sanitizer.build(input).report;
    match bounded_json(&sanitized) {
        Ok(payload) => Ok((sanitized, payload)),
        Err(_) => {
            let fallback = sanitizer
                .build(FeedbackInput {
                    source: "tool".into(),
                    validator_id: None,
                    exit_code: feedback.exit_code,
                    class: FeedbackClass::ToolFailed,
                    summary: "tool feedback serialization exceeded durable limits".into(),
                    stdout: String::new(),
                    stderr: String::new(),
                    retryable: feedback.retryable,
                })
                .report;
            let payload = bounded_json(&fallback)?;
            Ok((fallback, payload))
        }
    }
}

#[derive(Default)]
struct DataBudget {
    nodes: usize,
    text_bytes: usize,
}

impl DataBudget {
    fn admit_node(&mut self) -> Result<(), ()> {
        self.nodes = self.nodes.checked_add(1).ok_or(())?;
        (self.nodes <= MAX_NODES).then_some(()).ok_or(())
    }

    fn admit_text(&mut self, bytes: usize) -> Result<(), ()> {
        self.text_bytes = self.text_bytes.checked_add(bytes).ok_or(())?;
        (self.text_bytes <= MAX_AGGREGATE_TEXT_BYTES)
            .then_some(())
            .ok_or(())
    }
}

fn sanitize_value(
    value: &Value,
    sanitizer: &FeedbackEngine,
    depth: usize,
    budget: &mut DataBudget,
) -> Result<Value, ()> {
    if depth > MAX_DEPTH {
        return Err(());
    }
    budget.admit_node()?;

    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value.clone()),
        Value::String(text) => {
            budget.admit_text(text.len())?;
            if text.len() > MAX_STRING_BYTES {
                return Err(());
            }
            let sanitized = sanitizer.sanitize_text(text);
            if sanitized.len() > MAX_STRING_BYTES {
                return Err(());
            }
            Ok(Value::String(sanitized))
        }
        Value::Array(values) => values
            .iter()
            .map(|value| sanitize_value(value, sanitizer, depth + 1, budget))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(values) => {
            let mut sanitized = Map::new();
            let mut keys = HashSet::new();
            for (key, value) in values {
                budget.admit_text(key.len())?;
                if key.len() > MAX_KEY_BYTES {
                    return Err(());
                }
                let key = sanitizer.sanitize_text(key);
                if key.len() > MAX_KEY_BYTES {
                    return Err(());
                }
                if !keys.insert(key.clone()) {
                    return Err(());
                }
                sanitized.insert(key, sanitize_value(value, sanitizer, depth + 1, budget)?);
            }
            Ok(Value::Object(sanitized))
        }
    }
}

fn sanitized_or_fallback(
    input: &str,
    sanitizer: &FeedbackEngine,
    max_bytes: usize,
    fallback: &str,
) -> String {
    if input.len() > max_bytes {
        fallback.to_owned()
    } else {
        let sanitized = sanitizer.sanitize_text(input);
        if sanitized.len() <= max_bytes {
            sanitized
        } else {
            fallback.to_owned()
        }
    }
}

fn truncation_sentinel() -> Value {
    serde_json::json!({"reason": "limit_exceeded", "truncated": true})
}

fn bounded_json<T: serde::Serialize>(value: &T) -> Result<String, StoreError> {
    let payload = serde_json::to_string(value)?;
    if payload.len() <= MAX_PAYLOAD_BYTES {
        Ok(payload)
    } else {
        Err(StoreError::Invariant(
            "terminal observation exceeds the durable payload limit".into(),
        ))
    }
}

fn validate_observation_id(observation_id: &ObservationId) -> Result<(), StoreError> {
    let value = &observation_id.0;
    if !value.is_empty()
        && value.len() <= MAX_OBSERVATION_ID_BYTES
        && !value.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(StoreError::Invariant(
            "observation identifier is malformed".into(),
        ))
    }
}

fn validate_call_id(call_id: &CallId) -> Result<(), StoreError> {
    if !call_id.0.is_empty()
        && call_id.0.len() <= MAX_OBSERVATION_ID_BYTES
        && !call_id.0.chars().any(char::is_control)
    {
        Ok(())
    } else {
        Err(StoreError::Invariant(
            "terminal call identifier is malformed".into(),
        ))
    }
}

fn failed_observation_id(run_id: &RunId, call_id: &CallId) -> ObservationId {
    let mut hasher = Sha256::new();
    for value in [&run_id.0, &call_id.0] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    ObservationId::from(format!("observation:failed:{}", hex(&hasher.finalize())))
}

fn fingerprint(payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
