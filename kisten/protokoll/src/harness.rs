//! Versioned wire types for the self-owned harness.
//!
//! These types deliberately contain no execution or provider logic.  They are
//! the durable boundary shared by the harness loop, CLI, audit stream, and
//! later WebUI adapters.

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident) => {
        /// A strongly typed identifier used by the harness protocol.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

string_id!(EventId);
string_id!(RunId);
string_id!(TurnId);
string_id!(StepId);
string_id!(CallId);
string_id!(ActionId);
string_id!(ApprovalId);
string_id!(ObservationId);

/// An action candidate produced by the model and subsequently checked by the
/// governance layer before it can reach a tool runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentAction {
    ListFiles {
        path: String,
        depth: u16,
    },
    SearchText {
        path: String,
        query: String,
    },
    ReadFile {
        path: String,
        start_line: Option<u32>,
        end_line: Option<u32>,
    },
    WriteFile {
        path: String,
        content: String,
    },
    ApplyPatch {
        patch: String,
    },
    RunCommand {
        program: String,
        args: Vec<String>,
        cwd: Option<String>,
    },
    RunChecks {
        ids: Vec<String>,
    },
    Remember {
        kind: MemoryKind,
        content: String,
    },
    Recall {
        query: String,
        limit: u16,
    },
    RequestApproval {
        reason: String,
    },
    Finish {
        summary: String,
    },
}

/// The bounded categories of durable memory a harness action may propose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Convention,
    ArchitectureDecision,
    Lesson,
}

/// Governance outcome for a decoded action, ordered from least to most
/// restrictive so a stricter policy can safely win a merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Ask,
    Deny,
}

/// A normalized result from a tool or validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackReport {
    pub source: String,
    pub validator_id: Option<String>,
    pub exit_code: Option<i32>,
    pub classification: String,
    pub summary: String,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub fingerprint: String,
    pub retryable: bool,
}

/// Typed terminal or pause reasons for a harness run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Succeeded,
    Failed,
    Cancelled,
    AwaitingApproval,
    BudgetExceeded,
    RepeatedFailure,
    InterruptedUnknownOutcome,
}

/// Structured output from a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: ObservationId,
    pub call_id: CallId,
    pub kind: String,
    pub summary: String,
    pub data: serde_json::Value,
}

/// A durable, hash-bindable request for human approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    pub action_id: ActionId,
    pub action_hash: String,
    pub workspace_identity: String,
    pub policy_snapshot_hash: String,
    pub config_snapshot_hash: String,
    pub risk: String,
    pub rule_id: String,
    pub created_at: String,
    pub expires_at: String,
}

/// The event payload variants emitted by the self-harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum HarnessEventKind {
    RunCreated,
    StepStarted,
    ModelStarted,
    ModelCompleted {
        assistant_text: String,
    },
    ActionRecorded {
        action_id: ActionId,
        action: AgentAction,
    },
    PolicyDecided {
        action_id: ActionId,
        decision: PolicyDecision,
        rule_id: String,
    },
    ApprovalRequested {
        request: ApprovalRequest,
    },
    ApprovalResolved {
        approval_id: ApprovalId,
        decision: String,
    },
    ToolStarted {
        action_id: ActionId,
    },
    ToolCompleted {
        observation: Observation,
    },
    ToolFailed {
        feedback: FeedbackReport,
    },
    ValidatorCompleted {
        feedback: FeedbackReport,
    },
    RunPaused {
        reason: StopReason,
    },
    RunCompleted {
        reason: StopReason,
        summary: String,
    },
}

/// Versioned envelope shared by persisted and streamed harness events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessEvent {
    pub schema_version: u16,
    pub event_id: EventId,
    pub run_id: RunId,
    pub turn_id: Option<TurnId>,
    pub step_id: Option<StepId>,
    pub call_id: Option<CallId>,
    pub sequence: u64,
    pub occurred_at: String,
    pub kind: HarnessEventKind,
}

impl HarnessEvent {
    /// Build a deterministic envelope for protocol tests and fixtures.
    ///
    /// Runtime code will supply real timestamps and identifiers when it
    /// creates events; this helper intentionally has no clock dependency.
    pub fn new_for_test(
        event_id: EventId,
        run_id: RunId,
        step_id: StepId,
        sequence: u64,
        kind: HarnessEventKind,
    ) -> Self {
        Self {
            schema_version: 1,
            event_id,
            run_id,
            turn_id: None,
            step_id: Some(step_id),
            call_id: None,
            sequence,
            occurred_at: "2026-07-10T00:00:00Z".into(),
            kind,
        }
    }
}
