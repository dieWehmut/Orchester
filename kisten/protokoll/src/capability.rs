use serde::{Deserialize, Serialize};

/// What an adapter advertises about itself.
///
/// This feeds the future Planner (v1.0): given a task, pick the adapter whose
/// capabilities best match. In v0.1 it is surfaced by `orchester list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Adapter name (matches the CLI `--agent <name>`).
    pub name: String,
    /// Task kinds this agent handles.
    pub kinds: Vec<TaskKind>,
    /// Whether the agent supports resuming a prior session.
    pub supports_resume: bool,
    /// Whether the agent streams incremental events (vs. one final blob).
    pub streaming: bool,
}

/// A category of work an agent can perform. Feeds capability-based routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Code,
    Review,
    Chat,
    Browser,
    /// Escape hatch for vendor-specific kinds not yet modeled.
    Custom(String),
}
