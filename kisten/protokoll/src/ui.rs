//! Versioned, redaction-safe events intended for browser clients.
//!
//! HarnessEvent remains the durable internal journal. This module is a
//! deliberately smaller public projection: it contains display data and stable
//! correlation ids, never provider payloads, workspace identities or tool
//! observations.

use serde::{Deserialize, Serialize};

use crate::harness::{ApprovalId, CallId, EventId, RunId, StopReason, TurnId};
use crate::{ChangeKind, TodoItem};

/// The first version of the browser envelope.
pub const UI_SCHEMA_VERSION: u16 = 1;

/// An explicit version marker for clients that still receive legacy flat
/// Event values rather than envelopes.
pub const LEGACY_EVENT_SCHEMA_VERSION: u16 = 0;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiApprovalRequest {
    pub approval_id: ApprovalId,
    pub row_version: u64,
    pub risk: String,
    pub action: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
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
        approval_id: ApprovalId,
        row_version: u64,
        decision: UiApprovalDecision,
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

/// The stream envelope consumed by browser clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiEventEnvelope {
    pub schema_version: u16,
    pub event_id: EventId,
    pub run_id: RunId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<TurnId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<CallId>,
    pub sequence: u64,
    pub occurred_at: String,
    pub kind: UiEventKind,
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
        let event = envelope(UiEventKind::ToolCall {
            call_id: CallId::from("call-7"),
            name: "read_file".into(),
            state: UiToolState::Running,
            detail: None,
        });
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["kind"]["type"], "tool_call");
        assert_eq!(json["kind"]["call_id"], "call-7");
        assert_eq!(json["kind"]["state"], "running");
        assert!(json["kind"].get("detail").is_none());
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
}
