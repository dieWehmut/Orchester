use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A unit of work handed to an adapter.
///
/// This is deliberately minimal: Orchester never re-implements planning, tools, or
/// memory — it only tells an existing agent *what* to do and *where*, plus optional
/// resume/model hints that the adapter translates into vendor-specific flags.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    /// The natural-language instruction handed to the agent.
    pub prompt: String,
    /// Working directory the agent runs in.
    pub cwd: PathBuf,
    /// Session id to resume, if continuing a prior run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
    /// Optional model override (vendor-specific string, e.g. `gpt-5`, `claude-opus-4-6`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Task {
    /// Construct a fresh task with the given prompt, rooted at `cwd`.
    pub fn new(prompt: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            prompt: prompt.into(),
            cwd: cwd.into(),
            resume: None,
            model: None,
        }
    }

    /// Builder-style: resume an existing session.
    pub fn with_resume(mut self, session_id: impl Into<String>) -> Self {
        self.resume = Some(session_id.into());
        self
    }

    /// Builder-style: override the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}
