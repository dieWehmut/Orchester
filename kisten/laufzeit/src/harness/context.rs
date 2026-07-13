//! Bounded, secret-aware context assembly for one model call.

use std::fmt;

use orchester_modell::{ModelItem, ModelMessage, ModelRequest, ModelRole, ToolDefinition};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use super::transcript::TranscriptRecord as TranscriptEntry;

const SYSTEM_PROMPT: &str = "You are the Orchester self-owned coding agent. Inspect the workspace with the provided structured tools. Produce at most one tool call per step. Never invent tool results. Use request_approval for a human checkpoint and finish only after required validation succeeds.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLimits {
    pub max_bytes: usize,
    pub max_history_entries: usize,
}

impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            max_bytes: 128 * 1024,
            max_history_entries: 64,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ContextInput {
    pub model: String,
    pub prompt: String,
    pub history: Vec<TranscriptEntry>,
    pub store: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ContinuationInput {
    pub model: String,
    pub history: Vec<TranscriptEntry>,
    pub store: bool,
}

impl fmt::Debug for ContinuationInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContinuationInput")
            .field("model", &"<redacted>")
            .field("history_entries", &self.history.len())
            .field("store", &self.store)
            .finish()
    }
}

impl fmt::Debug for ContextInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextInput")
            .field("model", &"<redacted>")
            .field("prompt_bytes", &self.prompt.len())
            .field("history_entries", &self.history.len())
            .field("store", &self.store)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssembledContext {
    pub request: ModelRequest,
    pub omitted_entries: usize,
    pub omitted_prefix_hash: String,
    pub estimated_bytes: usize,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ContextError {
    #[error("task prompt is empty")]
    EmptyPrompt,
    #[error("model context exceeds its configured budget")]
    BudgetExceeded,
    #[error("model context contains credential material")]
    SecretDetected,
    #[error("model context limits are invalid")]
    InvalidLimits,
    #[error("model continuation is missing a matching tool call and result")]
    InvalidContinuation,
}

pub struct ContextAssembler {
    limits: ContextLimits,
    secrets: Vec<SecretString>,
}

impl ContextAssembler {
    pub fn new(limits: ContextLimits, secrets: Vec<SecretString>) -> Self {
        Self { limits, secrets }
    }

    pub fn assemble(&self, input: ContextInput) -> Result<AssembledContext, ContextError> {
        if self.limits.max_bytes == 0 || self.limits.max_history_entries == 0 {
            return Err(ContextError::InvalidLimits);
        }
        if input.prompt.trim().is_empty() {
            return Err(ContextError::EmptyPrompt);
        }
        self.assemble_inner(
            input.model,
            Some(input.prompt),
            input.history,
            input.store,
            0,
        )
    }

    pub fn assemble_continuation(
        &self,
        input: ContinuationInput,
    ) -> Result<AssembledContext, ContextError> {
        if self.limits.max_bytes == 0 || self.limits.max_history_entries < 2 {
            return Err(ContextError::InvalidLimits);
        }
        if !has_matching_tool_tail(&input.history) {
            return Err(ContextError::InvalidContinuation);
        }
        self.assemble_inner(input.model, None, input.history, input.store, 2)
    }

    fn assemble_inner(
        &self,
        model: String,
        prompt: Option<String>,
        history: Vec<TranscriptEntry>,
        store: bool,
        required_tail_entries: usize,
    ) -> Result<AssembledContext, ContextError> {
        self.reject_secret(&model)?;
        if let Some(prompt) = prompt.as_deref() {
            self.reject_secret(prompt)?;
        }
        for entry in &history {
            for value in entry.strings() {
                self.reject_secret(value)?;
            }
        }

        let tools = tool_definitions();
        let tools_bytes = tools.iter().map(tool_bytes).sum::<usize>();
        let fixed_bytes =
            SYSTEM_PROMPT.len() + model.len() + prompt.as_deref().map_or(0, str::len) + tools_bytes;
        if fixed_bytes > self.limits.max_bytes {
            return Err(ContextError::BudgetExceeded);
        }

        let max_start = history
            .len()
            .saturating_sub(self.limits.max_history_entries);
        let mut start = max_start;
        let mut history_bytes = history[start..]
            .iter()
            .map(TranscriptEntry::byte_len)
            .sum::<usize>();
        let required_tail_start = history.len().saturating_sub(required_tail_entries);
        while start < history.len()
            && fixed_bytes.saturating_add(history_bytes) > self.limits.max_bytes
        {
            if required_tail_entries > 0 && start >= required_tail_start {
                return Err(ContextError::BudgetExceeded);
            }
            history_bytes = history_bytes.saturating_sub(history[start].byte_len());
            start += 1;
        }

        let omitted = &history[..start];
        let retained = &history[start..];
        let mut messages = Vec::with_capacity(retained.len() + 2);
        messages.push(text_message(ModelRole::System, SYSTEM_PROMPT.into()));
        messages.extend(retained.iter().map(TranscriptEntry::to_message));
        if let Some(prompt) = prompt {
            messages.push(text_message(ModelRole::User, prompt));
        }

        Ok(AssembledContext {
            request: ModelRequest {
                model,
                messages,
                tools,
                store,
            },
            omitted_entries: omitted.len(),
            omitted_prefix_hash: transcript_hash(omitted),
            estimated_bytes: fixed_bytes + history_bytes,
        })
    }

    fn reject_secret(&self, value: &str) -> Result<(), ContextError> {
        if self.secrets.iter().any(|secret| {
            let secret = secret.expose_secret();
            !secret.is_empty() && value.contains(secret)
        }) || looks_like_secret(value)
        {
            Err(ContextError::SecretDetected)
        } else {
            Ok(())
        }
    }
}

fn has_matching_tool_tail(history: &[TranscriptEntry]) -> bool {
    let Some((result, preceding)) = history.split_last() else {
        return false;
    };
    let Some(call) = preceding.last() else {
        return false;
    };
    matches!(
        (call, result),
        (
            TranscriptEntry::ToolCall { call_id: call_id_a, .. },
            TranscriptEntry::ToolResult { call_id: call_id_b, .. }
        ) if call_id_a == call_id_b
    )
}

fn text_message(role: ModelRole, text: String) -> ModelMessage {
    ModelMessage {
        role,
        items: vec![ModelItem::Text(text)],
    }
}

fn transcript_hash(entries: &[TranscriptEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-context-prefix-v1");
    for entry in entries {
        let kind = match entry {
            TranscriptEntry::System(_) => 0u8,
            TranscriptEntry::User(_) => 1u8,
            TranscriptEntry::Assistant(_) => 2,
            TranscriptEntry::ToolCall { .. } => 3,
            TranscriptEntry::ToolResult { .. } => 4,
            TranscriptEntry::Opaque { .. } => 5,
        };
        hasher.update([kind]);
        for value in entry.strings() {
            hasher.update((value.len() as u64).to_le_bytes());
            hasher.update(value.as_bytes());
        }
        if let TranscriptEntry::Opaque { byte_len, .. } = entry {
            hasher.update((*byte_len as u64).to_le_bytes());
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn tool_bytes(tool: &ToolDefinition) -> usize {
    tool.name.len() + tool.description.len() + tool.parameters.to_string().len()
}

fn looks_like_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "sk-",
        "sk_",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "authorization: bearer ",
        "-----begin private key-----",
    ]
    .iter()
    .any(|prefix| lower.contains(prefix))
}

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            "list_files",
            "List files below a workspace-relative path.",
            object_schema(
                &["path", "depth"],
                json!({"path":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":16}}),
            ),
        ),
        tool(
            "search_text",
            "Search text below a workspace-relative path.",
            object_schema(
                &["path", "query"],
                json!({"path":{"type":"string"},"query":{"type":"string"}}),
            ),
        ),
        tool(
            "read_file",
            "Read a bounded line range from a workspace file.",
            object_schema(
                &["path", "start_line", "end_line"],
                json!({
                    "path":{"type":"string"},
                    "start_line":{"type":["integer","null"],"minimum":1},
                    "end_line":{"type":["integer","null"],"minimum":1}
                }),
            ),
        ),
        tool(
            "write_file",
            "Write complete content to a workspace file.",
            object_schema(
                &["path", "content"],
                json!({"path":{"type":"string"},"content":{"type":"string"}}),
            ),
        ),
        tool(
            "apply_patch",
            "Apply one unified patch inside the workspace.",
            object_schema(&["patch"], json!({"patch":{"type":"string"}})),
        ),
        tool(
            "run_command",
            "Run one structured executable and argument vector without a shell.",
            object_schema(
                &["program", "args", "cwd"],
                json!({
                    "program":{"type":"string"},
                    "args":{"type":"array","items":{"type":"string"},"maxItems":128},
                    "cwd":{"type":["string","null"]}
                }),
            ),
        ),
        tool(
            "run_checks",
            "Run configured validator identifiers.",
            object_schema(
                &["ids"],
                json!({"ids":{"type":"array","items":{"type":"string"},"maxItems":128}}),
            ),
        ),
        tool(
            "remember",
            "Propose a project memory item for later review.",
            object_schema(
                &["kind", "content"],
                json!({
                    "kind":{"type":"string","enum":["convention","architecture_decision","lesson"]},
                    "content":{"type":"string"}
                }),
            ),
        ),
        tool(
            "recall",
            "Recall accepted project memory.",
            object_schema(
                &["query", "limit"],
                json!({"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":100}}),
            ),
        ),
        tool(
            "request_approval",
            "Pause at a no-side-effect human checkpoint.",
            object_schema(&["reason"], json!({"reason":{"type":"string"}})),
        ),
        tool(
            "finish",
            "Finish the run after required validation.",
            object_schema(&["summary"], json!({"summary":{"type":"string"}})),
        ),
    ]
}

fn tool(name: &str, description: &str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: description.into(),
        parameters,
    }
}

fn object_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}
