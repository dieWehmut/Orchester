//! Versioned, redaction-safe events intended for browser clients.
//!
//! HarnessEvent remains the durable internal journal. This module is a
//! deliberately smaller public projection: it contains display data and stable
//! correlation ids, never provider payloads, workspace identities or tool
//! observations.

use std::fmt;

use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::harness::{
    normalize_action_summary, ApprovalId, CallId, EventId, RunId, StopReason, TurnId,
};
use crate::{ChangeKind, TodoItem};

/// The first version of the browser envelope.
pub const UI_SCHEMA_VERSION: u16 = 1;

/// An explicit version marker for clients that still receive legacy flat
/// Event values rather than envelopes.
pub const LEGACY_EVENT_SCHEMA_VERSION: u16 = 0;

/// A browser protocol invariant violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiProtocolValidationError {
    UnsupportedSchemaVersion {
        found: u16,
        expected: u16,
    },
    InvalidSequence {
        found: u64,
    },
    EmptyEventId,
    EmptyRunId,
    EmptyCallId,
    EmptyApprovalId,
    InvalidApprovalRowVersion,
    EmptyOccurredAt,
    ApprovalRunIdMismatch {
        outer: RunId,
        request: RunId,
    },
    ToolCallIdMismatch {
        envelope: Option<CallId>,
        payload: CallId,
    },
}

impl fmt::Display for UiProtocolValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, expected } => write!(
                f,
                "unsupported UI schema version {found}; expected {expected}"
            ),
            Self::InvalidSequence { found } => {
                write!(f, "UI sequence must be greater than zero (got {found})")
            }
            Self::EmptyEventId => f.write_str("UI event id must not be empty"),
            Self::EmptyRunId => f.write_str("UI run id must not be empty"),
            Self::EmptyCallId => f.write_str("UI tool call id must not be empty"),
            Self::EmptyApprovalId => f.write_str("UI approval id must not be empty"),
            Self::InvalidApprovalRowVersion => {
                f.write_str("UI approval row version must be greater than zero")
            }
            Self::EmptyOccurredAt => f.write_str("UI event timestamp must not be empty"),
            Self::ApprovalRunIdMismatch { outer, request } => write!(
                f,
                "UI approval request run binding does not match event run ({}/{})",
                outer.0, request.0
            ),
            Self::ToolCallIdMismatch { envelope, payload } => write!(
                f,
                "UI tool call id does not match envelope ({:?}/{})",
                envelope, payload.0
            ),
        }
    }
}

impl std::error::Error for UiProtocolValidationError {}

/// The lifecycle states a browser can render for one tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiToolState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// A redacted decision state for an approval row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiApprovalDecision {
    Approved,
    Denied,
    Expired,
    Stale,
}

/// Token totals displayed in the run footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_output_tokens: u64,
}

/// A browser-safe approval request. Durable hashes and workspace identities
/// stay in the harness journal; the UI receives only the decision context.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiApprovalRequest {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    pub row_version: u64,
    pub risk: String,
    pub action: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

impl UiApprovalRequest {
    fn sanitized(&self) -> Self {
        Self {
            approval_id: self.approval_id.clone(),
            run_id: self.run_id.clone(),
            row_version: self.row_version,
            risk: redact_ui_text(&self.risk),
            action: redact_ui_text(&self.action),
            reason: redact_ui_text(&self.reason),
            expires_at: self.expires_at.clone(),
        }
    }

    fn validate(&self) -> Result<(), UiProtocolValidationError> {
        if self.approval_id.0.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyApprovalId);
        }
        if self.run_id.0.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyRunId);
        }
        if self.row_version == 0 {
            return Err(UiProtocolValidationError::InvalidApprovalRowVersion);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiApprovalRequestWire {
    approval_id: ApprovalId,
    run_id: RunId,
    row_version: u64,
    risk: String,
    action: String,
    reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

impl From<&UiApprovalRequest> for UiApprovalRequestWire {
    fn from(value: &UiApprovalRequest) -> Self {
        let sanitized = value.sanitized();
        Self {
            approval_id: sanitized.approval_id,
            run_id: sanitized.run_id,
            row_version: sanitized.row_version,
            risk: sanitized.risk,
            action: sanitized.action,
            reason: sanitized.reason,
            expires_at: sanitized.expires_at,
        }
    }
}

impl Serialize for UiApprovalRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        UiApprovalRequestWire::from(self).serialize(serializer)
    }
}

/// The outcome returned after an approval row is changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiApprovalResolution {
    pub approval_id: ApprovalId,
    pub row_version: u64,
    pub decision: UiApprovalDecision,
}

impl UiApprovalResolution {
    fn validate(&self) -> Result<(), UiProtocolValidationError> {
        if self.approval_id.0.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyApprovalId);
        }
        if self.row_version == 0 {
            return Err(UiProtocolValidationError::InvalidApprovalRowVersion);
        }
        Ok(())
    }
}

/// A validation result that is safe to show in a transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiValidation {
    pub ok: bool,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Display events emitted by the runtime adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiEventKind {
    RunStarted {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    TurnStarted,
    Message {
        text: String,
    },
    MessageDelta {
        text: String,
        #[serde(rename = "final", default)]
        final_chunk: bool,
    },
    Reasoning {
        text: String,
    },
    ToolCall {
        call_id: CallId,
        name: String,
        state: UiToolState,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    FileChange {
        path: String,
        kind: ChangeKind,
    },
    TodoList {
        items: Vec<TodoItem>,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cached_input_tokens: u64,
        reasoning_output_tokens: u64,
    },
    ApprovalRequested {
        approval: UiApprovalRequest,
    },
    ApprovalResolved {
        resolution: UiApprovalResolution,
    },
    Validation {
        validation: UiValidation,
    },
    RunStopped {
        reason: StopReason,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Remove host-specific roots while keeping a useful relative tail for the
/// file tree. The root marker is intentionally not a username or workspace
/// name.
pub fn redact_ui_path(input: &str) -> String {
    let normalized = input.trim().replace('\\', "/");
    let absolute = normalized.starts_with('/')
        || normalized.starts_with("~/")
        || (normalized.len() >= 3
            && normalized.as_bytes()[1] == b':'
            && normalized.as_bytes()[2] == b'/');
    let mut parts = normalized
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return "[REDACTED_PATH]".into();
    }

    if absolute {
        let keep_from = parts.len().saturating_sub(3);
        parts = parts.split_off(keep_from);
        return format!("[ROOT]/{}", parts.join("/"));
    }

    if parts.iter().any(|part| *part == "..") {
        return parts
            .into_iter()
            .map(|part| if part == ".." { "[PARENT]" } else { part })
            .collect::<Vec<_>>()
            .join("/");
    }

    parts.join("/")
}

/// Normalize and redact display text before it can cross the browser boundary.
///
/// Action summaries already have conservative credential handling in the
/// harness. This wrapper additionally removes absolute path roots from tokens
/// such as path=/Users/name/project/file.rs.
pub fn redact_ui_text(input: &str) -> String {
    let normalized = normalize_action_summary(input);
    normalized
        .split_whitespace()
        .map(|token| {
            if token.contains("://") {
                return "[REDACTED_URL]".to_owned();
            }
            if let Some(separator) = token.find(['=', ':']) {
                let key = &token[..separator];
                let value = &token[separator + 1..];
                if value.starts_with('/')
                    || value.starts_with("~/")
                    || (value.len() >= 3
                        && value.as_bytes()[1] == b':'
                        && value.as_bytes()[2] == b'/')
                {
                    return format!("{key}={}", redact_ui_path(value));
                }
            }
            if token.starts_with('/')
                || token.starts_with("~/")
                || (token.len() >= 3 && token.as_bytes()[1] == b':' && token.as_bytes()[2] == b'/')
            {
                return redact_ui_path(token);
            }
            token.to_owned()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl UiEventKind {
    fn sanitized(&self) -> Self {
        match self {
            Self::RunStarted { title } => Self::RunStarted {
                title: title.as_deref().map(redact_ui_text),
            },
            Self::TurnStarted => Self::TurnStarted,
            Self::Message { text } => Self::Message {
                text: redact_ui_text(text),
            },
            Self::MessageDelta { text, final_chunk } => Self::MessageDelta {
                text: redact_ui_text(text),
                final_chunk: *final_chunk,
            },
            Self::Reasoning { text } => Self::Reasoning {
                text: redact_ui_text(text),
            },
            Self::ToolCall {
                call_id,
                name,
                state,
                detail,
            } => Self::ToolCall {
                call_id: call_id.clone(),
                name: redact_ui_text(name),
                state: *state,
                detail: detail.as_deref().map(redact_ui_text),
            },
            Self::FileChange { path, kind } => Self::FileChange {
                path: redact_ui_path(path),
                kind: *kind,
            },
            Self::TodoList { items } => Self::TodoList {
                items: items
                    .iter()
                    .map(|item| TodoItem {
                        text: redact_ui_text(&item.text),
                        completed: item.completed,
                    })
                    .collect(),
            },
            Self::Usage {
                input_tokens,
                output_tokens,
                cached_input_tokens,
                reasoning_output_tokens,
            } => Self::Usage {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                cached_input_tokens: *cached_input_tokens,
                reasoning_output_tokens: *reasoning_output_tokens,
            },
            Self::ApprovalRequested { approval } => Self::ApprovalRequested {
                approval: approval.sanitized(),
            },
            Self::ApprovalResolved { resolution } => Self::ApprovalResolved {
                resolution: resolution.clone(),
            },
            Self::Validation { validation } => Self::Validation {
                validation: UiValidation {
                    ok: validation.ok,
                    summary: redact_ui_text(&validation.summary),
                    details: validation.details.as_deref().map(redact_ui_text),
                },
            },
            Self::RunStopped { reason } => Self::RunStopped {
                reason: reason.clone(),
            },
            Self::Error { code, message } => Self::Error {
                code: redact_ui_text(code),
                message: redact_ui_text(message),
            },
        }
    }
}

/// The stream envelope consumed by browser clients.
#[derive(Debug, Clone, PartialEq)]
pub struct UiEventEnvelope {
    pub schema_version: u16,
    pub event_id: EventId,
    pub run_id: RunId,
    pub turn_id: Option<TurnId>,
    pub call_id: Option<CallId>,
    pub sequence: u64,
    pub occurred_at: String,
    pub kind: UiEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiEventEnvelopeWire {
    schema_version: u16,
    event_id: EventId,
    run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<TurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    call_id: Option<CallId>,
    sequence: u64,
    occurred_at: String,
    kind: UiEventKind,
}

impl From<&UiEventEnvelope> for UiEventEnvelopeWire {
    fn from(value: &UiEventEnvelope) -> Self {
        Self {
            schema_version: value.schema_version,
            event_id: value.event_id.clone(),
            run_id: value.run_id.clone(),
            turn_id: value.turn_id.clone(),
            call_id: value.call_id.clone(),
            sequence: value.sequence,
            occurred_at: value.occurred_at.clone(),
            kind: value.kind.sanitized(),
        }
    }
}

impl UiEventEnvelope {
    /// Validate invariants which serde field types cannot express.
    pub fn validate(&self) -> Result<(), UiProtocolValidationError> {
        if self.schema_version != UI_SCHEMA_VERSION {
            return Err(UiProtocolValidationError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: UI_SCHEMA_VERSION,
            });
        }
        if self.sequence == 0 {
            return Err(UiProtocolValidationError::InvalidSequence {
                found: self.sequence,
            });
        }
        if self.event_id.0.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyEventId);
        }
        if self.run_id.0.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyRunId);
        }
        if self.occurred_at.trim().is_empty() {
            return Err(UiProtocolValidationError::EmptyOccurredAt);
        }
        if let UiEventKind::ApprovalRequested { approval } = &self.kind {
            approval.validate()?;
            if approval.run_id != self.run_id {
                return Err(UiProtocolValidationError::ApprovalRunIdMismatch {
                    outer: self.run_id.clone(),
                    request: approval.run_id.clone(),
                });
            }
        }
        if let UiEventKind::ApprovalResolved { resolution } = &self.kind {
            resolution.validate()?;
        }
        if let UiEventKind::ToolCall { call_id, .. } = &self.kind {
            if call_id.0.trim().is_empty() {
                return Err(UiProtocolValidationError::EmptyCallId);
            }
            if self.call_id.as_ref() != Some(call_id) {
                return Err(UiProtocolValidationError::ToolCallIdMismatch {
                    envelope: self.call_id.clone(),
                    payload: call_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn sanitized(&self) -> Self {
        Self {
            kind: self.kind.sanitized(),
            ..self.clone()
        }
    }
}

impl Serialize for UiEventEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let sanitized = self.sanitized();
        sanitized.validate().map_err(S::Error::custom)?;
        UiEventEnvelopeWire::from(&sanitized).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UiEventEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = UiEventEnvelopeWire::deserialize(deserializer)?;
        let event = Self {
            schema_version: wire.schema_version,
            event_id: wire.event_id,
            run_id: wire.run_id,
            turn_id: wire.turn_id,
            call_id: wire.call_id,
            sequence: wire.sequence,
            occurred_at: wire.occurred_at,
            kind: wire.kind,
        };
        event.validate().map_err(D::Error::custom)?;
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(kind: UiEventKind) -> UiEventEnvelope {
        UiEventEnvelope {
            schema_version: UI_SCHEMA_VERSION,
            event_id: EventId::from("event-1"),
            run_id: RunId::from("run-1"),
            turn_id: Some(TurnId::from("turn-1")),
            call_id: None,
            sequence: 1,
            occurred_at: "2026-08-19T00:00:00Z".into(),
            kind,
        }
    }

    #[test]
    fn envelope_keeps_metadata_and_nested_discriminator() {
        let json = serde_json::to_value(envelope(UiEventKind::TurnStarted)).unwrap();
        assert_eq!(json["schema_version"], UI_SCHEMA_VERSION);
        assert_eq!(json["event_id"], "event-1");
        assert_eq!(json["run_id"], "run-1");
        assert_eq!(json["kind"]["type"], "turn_started");
        assert!(json["call_id"].is_null());
    }

    #[test]
    fn tool_state_and_call_id_are_stable_wire_values() {
        let mut event = envelope(UiEventKind::ToolCall {
            call_id: CallId::from("call-7"),
            name: "read_file".into(),
            state: UiToolState::Running,
            detail: None,
        });
        event.call_id = Some(CallId::from("call-7"));
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["kind"]["type"], "tool_call");
        assert_eq!(json["kind"]["call_id"], "call-7");
        assert_eq!(json["kind"]["state"], "running");
        assert!(json["kind"].get("detail").is_none());
    }

    #[test]
    fn tool_frames_reject_missing_or_mismatched_envelope_call_ids() {
        let event = envelope(UiEventKind::ToolCall {
            call_id: CallId::from("call-7"),
            name: "read_file".into(),
            state: UiToolState::Queued,
            detail: None,
        });
        assert!(matches!(
            event.validate(),
            Err(UiProtocolValidationError::ToolCallIdMismatch { .. })
        ));

        let mut mismatched = event;
        mismatched.call_id = Some(CallId::from("call-other"));
        assert!(matches!(
            mismatched.validate(),
            Err(UiProtocolValidationError::ToolCallIdMismatch { .. })
        ));
    }

    #[test]
    fn same_named_tools_keep_distinct_call_ids() {
        let mut first = envelope(UiEventKind::ToolCall {
            call_id: CallId::from("call-a"),
            name: "run_command".into(),
            state: UiToolState::Running,
            detail: None,
        });
        first.call_id = Some(CallId::from("call-a"));
        let mut second = first.clone();
        second.event_id = EventId::from("event-2");
        second.call_id = Some(CallId::from("call-b"));
        second.kind = UiEventKind::ToolCall {
            call_id: CallId::from("call-b"),
            name: "run_command".into(),
            state: UiToolState::Succeeded,
            detail: None,
        };

        assert_eq!(first.validate(), Ok(()));
        assert_eq!(second.validate(), Ok(()));
        assert_ne!(first.call_id, second.call_id);
    }

    #[test]
    fn all_tool_states_have_explicit_wire_names() {
        let states = [
            (UiToolState::Queued, "queued"),
            (UiToolState::Running, "running"),
            (UiToolState::Succeeded, "succeeded"),
            (UiToolState::Failed, "failed"),
            (UiToolState::Cancelled, "cancelled"),
        ];
        for (state, expected) in states {
            assert_eq!(
                serde_json::to_string(&state).unwrap(),
                format!("\"{expected}\"")
            );
        }
    }

    #[test]
    fn approval_request_and_resolution_require_row_versions() {
        let mut request = UiApprovalRequest {
            approval_id: ApprovalId::from("approval-1"),
            run_id: RunId::from("run-1"),
            row_version: 0,
            risk: "high".into(),
            action: "write_file".into(),
            reason: "policy".into(),
            expires_at: None,
        };
        assert_eq!(
            request.validate(),
            Err(UiProtocolValidationError::InvalidApprovalRowVersion)
        );
        request.row_version = 1;

        let resolution = UiApprovalResolution {
            approval_id: request.approval_id,
            row_version: 1,
            decision: UiApprovalDecision::Approved,
        };
        let mut event = envelope(UiEventKind::ApprovalResolved {
            resolution: resolution.clone(),
        });
        assert_eq!(event.validate(), Ok(()));
        assert_eq!(
            serde_json::to_value(&event).unwrap()["kind"]["resolution"]["decision"],
            "approved"
        );
        event.kind = UiEventKind::ApprovalResolved {
            resolution: UiApprovalResolution {
                row_version: 0,
                ..resolution
            },
        };
        assert_eq!(
            event.validate(),
            Err(UiProtocolValidationError::InvalidApprovalRowVersion)
        );
    }

    fn all_event_variants() -> Vec<UiEventEnvelope> {
        let run_id = RunId::from("run-roundtrip");
        let approval = UiApprovalRequest {
            approval_id: ApprovalId::from("approval-roundtrip"),
            run_id: run_id.clone(),
            row_version: 2,
            risk: "high".into(),
            action: "write_file path=src/main.rs".into(),
            reason: "workspace policy".into(),
            expires_at: Some("2026-08-19T01:00:00Z".into()),
        };
        let kinds = vec![
            UiEventKind::RunStarted {
                title: Some("Roundtrip".into()),
            },
            UiEventKind::TurnStarted,
            UiEventKind::Message {
                text: "hello".into(),
            },
            UiEventKind::MessageDelta {
                text: "world".into(),
                final_chunk: false,
            },
            UiEventKind::Reasoning {
                text: "digest".into(),
            },
            UiEventKind::ToolCall {
                call_id: CallId::from("call-roundtrip"),
                name: "read_file".into(),
                state: UiToolState::Succeeded,
                detail: Some("src/main.rs".into()),
            },
            UiEventKind::FileChange {
                path: "src/main.rs".into(),
                kind: ChangeKind::Update,
            },
            UiEventKind::TodoList {
                items: vec![TodoItem {
                    text: "verify".into(),
                    completed: false,
                }],
            },
            UiEventKind::Usage {
                input_tokens: 1,
                output_tokens: 2,
                cached_input_tokens: 3,
                reasoning_output_tokens: 4,
            },
            UiEventKind::ApprovalRequested { approval },
            UiEventKind::ApprovalResolved {
                resolution: UiApprovalResolution {
                    approval_id: ApprovalId::from("approval-roundtrip"),
                    row_version: 3,
                    decision: UiApprovalDecision::Approved,
                },
            },
            UiEventKind::Validation {
                validation: UiValidation {
                    ok: true,
                    summary: "checks passed".into(),
                    details: None,
                },
            },
            UiEventKind::RunStopped {
                reason: StopReason::Succeeded,
            },
            UiEventKind::Error {
                code: "runtime_error".into(),
                message: "safe message".into(),
            },
        ];

        kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| {
                let call_id = if matches!(kind, UiEventKind::ToolCall { .. }) {
                    Some(CallId::from("call-roundtrip"))
                } else {
                    None
                };
                UiEventEnvelope {
                    schema_version: UI_SCHEMA_VERSION,
                    event_id: EventId::from(format!("event-{index}")),
                    run_id: run_id.clone(),
                    turn_id: Some(TurnId::from("turn-roundtrip")),
                    call_id,
                    sequence: index as u64 + 1,
                    occurred_at: "2026-08-19T00:00:00Z".into(),
                    kind,
                }
            })
            .collect()
    }

    #[test]
    fn every_ui_event_variant_roundtrips() {
        for event in all_event_variants() {
            let value = serde_json::to_value(&event).unwrap();
            let decoded: UiEventEnvelope = serde_json::from_value(value).unwrap();
            assert_eq!(decoded, event, "roundtrip failed for {:?}", event.kind);
        }
    }

    #[test]
    fn unknown_envelope_fields_are_rejected() {
        let raw = r#"{
            "schema_version": 1,
            "event_id": "event-1",
            "run_id": "run-1",
            "sequence": 1,
            "occurred_at": "2026-08-19T00:00:00Z",
            "kind": {"type": "turn_started"},
            "provider_secret": "do-not-accept"
        }"#;
        assert!(serde_json::from_str::<UiEventEnvelope>(raw).is_err());
    }

    #[test]
    fn unknown_kind_fields_are_rejected() {
        let raw = r#"{
            "schema_version": 1,
            "event_id": "event-1",
            "run_id": "run-1",
            "sequence": 1,
            "occurred_at": "2026-08-19T00:00:00Z",
            "kind": {"type": "turn_started", "unexpected": true}
        }"#;
        assert!(serde_json::from_str::<UiEventEnvelope>(raw).is_err());
    }

    #[test]
    fn invalid_sequence_and_schema_are_rejected() {
        let mut event = envelope(UiEventKind::TurnStarted);
        event.sequence = 0;
        assert_eq!(
            event.validate(),
            Err(UiProtocolValidationError::InvalidSequence { found: 0 })
        );
        assert!(serde_json::to_string(&event).is_err());

        event.sequence = 1;
        event.schema_version = UI_SCHEMA_VERSION + 1;
        assert!(matches!(
            event.validate(),
            Err(UiProtocolValidationError::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn approval_requests_are_bound_to_the_envelope_run() {
        let event = envelope(UiEventKind::ApprovalRequested {
            approval: UiApprovalRequest {
                approval_id: ApprovalId::from("approval-1"),
                run_id: RunId::from("another-run"),
                row_version: 1,
                risk: "high".into(),
                action: "write_file path=src/main.rs".into(),
                reason: "workspace write".into(),
                expires_at: None,
            },
        });

        assert_eq!(
            event.validate(),
            Err(UiProtocolValidationError::ApprovalRunIdMismatch {
                outer: RunId::from("run-1"),
                request: RunId::from("another-run"),
            })
        );
        assert!(serde_json::to_string(&event).is_err());
    }

    #[test]
    fn empty_run_and_event_ids_are_rejected() {
        let mut event = envelope(UiEventKind::TurnStarted);
        event.run_id = RunId::from(" ");
        assert_eq!(event.validate(), Err(UiProtocolValidationError::EmptyRunId));

        event.run_id = RunId::from("run-1");
        event.event_id = EventId::from("");
        assert_eq!(
            event.validate(),
            Err(UiProtocolValidationError::EmptyEventId)
        );
    }

    #[test]
    fn redacts_absolute_paths_and_credential_tokens() {
        assert_eq!(
            redact_ui_path(r"C:\Users\alice\project\src\main.rs"),
            "[ROOT]/project/src/main.rs"
        );
        assert_eq!(
            redact_ui_text("api_key=sk-live-secret path=/Users/alice/project/src/main.rs"),
            "api_key=[REDACTED] path=[ROOT]/project/src/main.rs"
        );
    }

    #[test]
    fn serialization_sanitizes_messages_tools_and_approvals() {
        let event = envelope(UiEventKind::ToolCall {
            call_id: CallId::from("call-redact"),
            name: "run_command".into(),
            state: UiToolState::Running,
            detail: Some("Authorization: Bearer sk-live-secret".into()),
        });
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("sk-live-secret"));
        assert!(json.contains("[REDACTED]"));

        let approval = envelope(UiEventKind::ApprovalRequested {
            approval: UiApprovalRequest {
                approval_id: ApprovalId::from("approval-redact"),
                run_id: RunId::from("run-1"),
                row_version: 1,
                risk: "provider api_key=sk-provider-secret".into(),
                action: "write_file path=/Users/alice/project/file.rs".into(),
                reason: "credential token=secret-value".into(),
                expires_at: None,
            },
        });
        let json = serde_json::to_string(&approval).unwrap();
        assert!(!json.contains("sk-provider-secret"));
        assert!(!json.contains("secret-value"));
        assert!(!json.contains("/Users/alice"));
        assert!(json.contains("[ROOT]/project/file.rs"));

        let direct_json = serde_json::to_string(match approval.kind {
            UiEventKind::ApprovalRequested { approval } => approval,
            _ => unreachable!(),
        })
        .unwrap();
        assert!(!direct_json.contains("sk-provider-secret"));
    }

    #[test]
    fn provider_payloads_are_not_part_of_the_ui_schema() {
        let raw = r#"{
            "schema_version": 1,
            "event_id": "event-1",
            "run_id": "run-1",
            "sequence": 1,
            "occurred_at": "2026-08-19T00:00:00Z",
            "kind": {
                "type": "error",
                "code": "provider_error",
                "message": "provider failed",
                "provider": {"base_url": "https://secret.example"}
            }
        }"#;
        assert!(serde_json::from_str::<UiEventEnvelope>(raw).is_err());
        assert_eq!(
            redact_ui_text("provider response https://secret.example/api"),
            "provider response [REDACTED_URL]"
        );
    }
}
