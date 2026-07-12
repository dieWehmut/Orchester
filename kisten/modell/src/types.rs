use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// A provider-neutral role in a model conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRole {
    System,
    User,
    Assistant,
    Tool,
}

/// One provider-neutral item inside a model message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelItem {
    Text(String),
    ToolCall(ToolCall),
    ToolResult { call_id: String, output: String },
    Opaque(Value),
}

/// A message sent to a language model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub items: Vec<ModelItem>,
}

/// A tool declaration supplied to a model provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// A provider-neutral model request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<ToolDefinition>,
    pub store: bool,
}

impl ModelRequest {
    /// Build a minimal request used by deterministic boundary tests.
    pub fn test() -> Self {
        Self {
            model: "test-model".to_owned(),
            messages: Vec::new(),
            tools: Vec::new(),
            store: false,
        }
    }
}

/// Token accounting returned by a model provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// A provider-neutral tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments_json: String,
}

impl ToolCall {
    pub fn new(
        call_id: impl Into<String>,
        name: impl Into<String>,
        arguments_json: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            name: name.into(),
            arguments_json: arguments_json.into(),
        }
    }
}

/// A single response from a language model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub assistant_text: String,
    pub tool_call: Option<ToolCall>,
    pub usage: ModelUsage,
    pub opaque_items: Vec<Value>,
}

impl ModelResponse {
    /// Build a response containing one tool call and default usage metadata.
    pub fn tool(
        call_id: impl Into<String>,
        name: impl Into<String>,
        arguments_json: impl Into<String>,
    ) -> Self {
        Self {
            assistant_text: String::new(),
            tool_call: Some(ToolCall::new(call_id, name, arguments_json)),
            usage: ModelUsage::default(),
            opaque_items: Vec::new(),
        }
    }

    pub fn tool_call(&self) -> Option<&ToolCall> {
        self.tool_call.as_ref()
    }
}

/// Errors crossing the model boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelError {
    #[error("model request cancelled")]
    Cancelled,
    #[error("scripted model responses exhausted")]
    ScriptExhausted,
    #[error("model transport error: {0}")]
    Transport(String),
    #[error("model protocol error: {0}")]
    Protocol(String),
}

/// Provider-neutral, loop-free single-call model interface.
#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError>;
}
