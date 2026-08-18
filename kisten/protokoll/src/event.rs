use serde::{Deserialize, Serialize};

use crate::harness::{ApprovalId, StopReason};
use crate::result::Usage;

/// A single normalized event in an agent run.
///
/// This mirrors Codex's `ThreadEvent` (see
/// `agent-research/codex/codex-rs/exec/src/exec_events.rs`) but is **vendor-neutral**:
/// every adapter maps its own stream into this one flat enum. Because it is
/// `#[serde(tag = "type")]`, serializing the stream yields clean JSONL — Orchester's
/// own wire protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The agent reported a resumable session/thread id. Emitted exactly once,
    /// the first time an id is seen.
    SessionStarted { session_id: String },
    /// A turn (one prompt→response cycle) has begun.
    TurnStarted,
    /// Assistant natural-language output.
    Message { text: String },
    /// The agent's reasoning summary (chain-of-thought digest).
    Reasoning { text: String },
    /// A tool invocation: a shell command execution or an MCP tool call.
    ToolCall {
        name: String,
        status: ToolStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A file was added, updated, or deleted by the agent.
    FileChange { path: String, kind: ChangeKind },
    /// The agent's running to-do list changed.
    TodoList { items: Vec<TodoItem> },
    /// Token usage accounting for the turn.
    Usage(Usage),
    /// The current turn completed.
    TurnCompleted,
    /// A governed action needs a human decision before the run can continue.
    ///
    /// `action` is the redacted one-line summary produced by
    /// [`crate::AgentAction::action_summary`], never raw model output: this event
    /// leaves the process and reaches a browser, and the approval prompt is
    /// exactly where a leaked credential would be read.
    ApprovalRequired {
        approval_id: ApprovalId,
        action: String,
        reason: String,
    },
    /// The final assistant message / run result text.
    Result { text: String },
    /// The run stopped, and why.
    ///
    /// Distinct from [`Event::Result`] and [`Event::Error`] because a governed run
    /// has outcomes that are neither: it can stop waiting for an approval or
    /// because it ran out of budget, and a frontend has to offer a different next
    /// step for each.
    Stopped { reason: StopReason },
    /// A fatal error surfaced by the agent or the adapter.
    Error { message: String },
}

/// Lifecycle status of a [`Event::ToolCall`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    InProgress,
    Completed,
    Failed,
}

/// The kind of a [`Event::FileChange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Add,
    Update,
    Delete,
}

/// A single entry in a [`Event::TodoList`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}
