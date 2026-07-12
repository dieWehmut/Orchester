//! Provider-neutral, single-call language-model boundary for the harness.
//!
//! This crate intentionally owns no loop, history mutation, provider adapter,
//! tool callback, or execution policy.  It only turns a request into one
//! response and provides deterministic test doubles for that boundary.

mod decoder;
mod scripted;
mod types;

pub use decoder::{ActionDecoder, DecodeError, MAX_LIST_DEPTH, MAX_RECALL_LIMIT};
pub use scripted::{RequestSummary, ScriptedLlm};
pub use types::{
    LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelUsage, ToolCall, ToolDefinition,
};
