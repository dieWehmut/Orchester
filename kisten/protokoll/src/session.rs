use serde::{Deserialize, Serialize};

/// Lifecycle state of a session, owned by the runtime's `Session`.
///
/// State machine:
/// `Starting` → `Running` (on first [`crate::Event::SessionStarted`])
/// → `Completed` (on [`crate::Event::Result`]) | `Failed` (on [`crate::Event::Error`])
/// | `Cancelled` (on Ctrl-C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}
