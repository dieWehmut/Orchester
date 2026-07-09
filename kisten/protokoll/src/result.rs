use serde::{Deserialize, Serialize};

/// Token accounting for a turn. Shared by [`crate::Event::Usage`] and [`RunResult`].
///
/// Field names mirror Codex's `Usage` for painless mapping, but the type is
/// vendor-neutral. All counts default to zero so adapters can emit partial data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub reasoning_output_tokens: u64,
}

impl Usage {
    /// Accumulate another usage sample into this one (adapters may report per-turn).
    pub fn add(&mut self, other: &Usage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cached_input_tokens += other.cached_input_tokens;
        self.reasoning_output_tokens += other.reasoning_output_tokens;
    }
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failed,
    Cancelled,
}

/// Summary produced by the runtime when a run finishes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunResult {
    /// The resumable session id, if the agent reported one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The final assistant text (from the last [`crate::Event::Result`]).
    pub final_text: String,
    /// Total token usage across the run.
    pub usage: Usage,
    /// Terminal outcome.
    pub outcome: Outcome,
}
