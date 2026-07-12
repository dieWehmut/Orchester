//! Provider-neutral, single-call language-model boundary for the harness.
//!
//! This crate intentionally owns no loop, history mutation, provider adapter,
//! tool callback, or execution policy.  It only turns a request into one
//! response and provides deterministic test doubles for that boundary.
//!
//! The model DTOs are an internal provider-neutral contract, not a persisted
//! wire format. They intentionally do not implement `Serialize` or
//! `Deserialize`; each provider adapter must map its own versioned wire shape
//! explicitly.

mod decoder;
mod scripted;
mod types;

pub use decoder::{
    ActionDecoder, ArgumentField, DecodeError, ToolKind, MAX_ARGUMENTS_JSON_BYTES,
    MAX_CALL_ID_BYTES, MAX_COMMAND_PART_BYTES, MAX_CONTENT_BYTES, MAX_LIST_DEPTH, MAX_LIST_ITEMS,
    MAX_PATH_BYTES, MAX_QUERY_BYTES, MAX_RECALL_LIMIT,
};
pub use scripted::{RequestSummary, ScriptedLlm};
pub use types::{
    LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest, ModelResponse, ModelRole,
    ModelUsage, RetryAfter, RetryMetadata, ToolCall, ToolDefinition,
};
