use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A declarative description of a subprocess agent.
///
/// This is the "data by default" half of Orchester's hybrid adapter model: most
/// agents can be added by shipping one of these TOML files under `manifeste/` —
/// no new Rust required. The [`crate::ManifestAdapter`] interprets it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterManifest {
    /// Adapter name (matches CLI `--agent <name>` and overrides built-ins by name).
    pub name: String,
    /// Executable to spawn (looked up on `PATH`).
    pub command: String,
    /// Argument template for a fresh run. Placeholders: `{prompt}`, `{model}`.
    pub args: Vec<String>,
    /// Full argument template used **instead of** `args` when resuming a session.
    /// Placeholders additionally include `{session_id}`. This lets irregular vendors
    /// (e.g. Codex's `exec resume <id>` *subcommand*) stay fully declarative.
    #[serde(default)]
    pub resume_args: Option<Vec<String>>,
    /// Task kinds this agent advertises (`code`, `review`, `chat`, `browser`, or custom).
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Whether the agent can resume a prior session.
    #[serde(default)]
    pub supports_resume: bool,
    /// Whether the agent streams incremental events.
    #[serde(default = "default_true")]
    pub streaming: bool,
    /// How to turn each stdout JSON line into events.
    pub parse: ParseSpec,
}

fn default_true() -> bool {
    true
}

/// How to interpret each JSON line from the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseSpec {
    /// Top-level field selecting a branch in [`Self::map`] (e.g. `type`).
    pub discriminator: String,
    /// Optional second-level discriminator for nested-tagged schemas (e.g. Codex's
    /// `item.type`). When set, the engine first tries a composite `"<disc>/<sub>"`
    /// map key, then falls back to the plain `<disc>` key.
    #[serde(default)]
    pub sub_discriminator: Option<String>,
    /// Dotted path to the resumable session id. Emits [`orchester_protokoll::Event::SessionStarted`]
    /// the first time a value is seen on any line.
    #[serde(default)]
    pub session_id: Option<String>,
    /// discriminator value -> how to build an Orchester event.
    #[serde(default)]
    pub map: HashMap<String, EventMapping>,
}

/// One branch of [`ParseSpec::map`]: which Orchester event to emit and where its
/// fields come from. All field values are either a JSON path or a `=literal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMapping {
    /// Target event kind: `message`, `reasoning`, `result`, `tool_call`, `file_change`,
    /// `usage`, `todo_list`, `session_started`, `turn_started`, `turn_completed`,
    /// `error`, or `ignore`.
    pub event: String,

    // --- text-bearing events (message / reasoning / result) ---
    #[serde(default)]
    pub text: Option<String>,

    // --- tool_call ---
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,

    // --- file_change ---
    /// Path to an array to iterate, emitting one FileChange per element.
    #[serde(default)]
    pub each: Option<String>,
    /// File path (relative to each element when `each` is set, else absolute path).
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,

    // --- error ---
    #[serde(default)]
    pub message: Option<String>,

    // --- usage ---
    #[serde(default)]
    pub input_tokens: Option<String>,
    #[serde(default)]
    pub output_tokens: Option<String>,
    #[serde(default)]
    pub cached_input_tokens: Option<String>,
    #[serde(default)]
    pub reasoning_output_tokens: Option<String>,

    // --- todo_list ---
    #[serde(default)]
    pub items: Option<String>,
    #[serde(default)]
    pub item_text: Option<String>,
    #[serde(default)]
    pub item_completed: Option<String>,
}
