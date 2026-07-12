use async_trait::async_trait;
use orchester_protokoll::CallId;
use serde_json::Value;
use std::fmt;
use std::time::Duration;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// A provider-neutral role in a model conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    System,
    User,
    Assistant,
    Tool,
}

/// One provider-neutral item inside a model message.
///
/// Model DTOs are intentionally not Serde wire types. Provider adapters must
/// explicitly translate their wire format so provider changes cannot silently
/// alter the harness boundary.
#[derive(Clone, PartialEq)]
pub enum ModelItem {
    Text(String),
    ToolCall(ToolCall),
    ToolResult { call_id: CallId, output: String },
    Opaque(Value),
}

impl fmt::Debug for ModelItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter
                .debug_struct("Text")
                .field("bytes", &text.len())
                .finish(),
            Self::ToolCall(call) => formatter.debug_tuple("ToolCall").field(call).finish(),
            Self::ToolResult { output, .. } => formatter
                .debug_struct("ToolResult")
                .field("call_id", &Redacted)
                .field("output_bytes", &output.len())
                .finish(),
            Self::Opaque(_) => formatter.write_str("Opaque(<redacted>)"),
        }
    }
}

/// A message sent to a language model.
#[derive(Clone, PartialEq)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub items: Vec<ModelItem>,
}

impl fmt::Debug for ModelMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelMessage")
            .field("role", &self.role)
            .field("item_count", &self.items.len())
            .finish()
    }
}

/// A tool declaration supplied to a model provider.
#[derive(Clone, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl fmt::Debug for ToolDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolDefinition")
            .field("name", &Redacted)
            .field("description_bytes", &self.description.len())
            .field("parameters", &Redacted)
            .finish()
    }
}

/// A provider-neutral model request.
#[derive(Clone, PartialEq)]
pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<ToolDefinition>,
    pub store: bool,
}

impl fmt::Debug for ModelRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelRequest")
            .field("model", &Redacted)
            .field("message_count", &self.messages.len())
            .field("tool_count", &self.tools.len())
            .field("store", &self.store)
            .finish()
    }
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// A provider-neutral tool call.
#[derive(Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub call_id: CallId,
    pub name: String,
    pub arguments_json: String,
}

impl ToolCall {
    pub fn new(
        call_id: impl Into<CallId>,
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

impl fmt::Debug for ToolCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolCall")
            .field("call_id", &Redacted)
            .field("name", &Redacted)
            .field("arguments_json_bytes", &self.arguments_json.len())
            .finish()
    }
}

/// A single response from a language model.
#[derive(Clone, PartialEq)]
pub struct ModelResponse {
    pub assistant_text: String,
    pub tool_call: Option<ToolCall>,
    pub usage: ModelUsage,
    pub opaque_items: Vec<Value>,
}

impl ModelResponse {
    /// Build a response containing one tool call and default usage metadata.
    pub fn tool(
        call_id: impl Into<CallId>,
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

impl fmt::Debug for ModelResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelResponse")
            .field("assistant_text_bytes", &self.assistant_text.len())
            .field("tool_call", &self.tool_call)
            .field("usage", &self.usage)
            .field("opaque_item_count", &self.opaque_items.len())
            .finish()
    }
}

/// A provider supplied retry delay after applying Orchester's safety cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RetryAfter(Duration);

impl RetryAfter {
    /// No provider can make the harness sleep for more than five minutes.
    pub const MAX: Self = Self(Duration::from_secs(5 * 60));

    pub fn new(duration: Duration) -> Self {
        Self(duration.min(Self::MAX.0))
    }

    pub const fn as_duration(self) -> Duration {
        self.0
    }
}

/// Safe retry guidance for the harness. It contains no provider body, header,
/// request identifier, or other arbitrary provider-controlled text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryMetadata {
    retryable: bool,
    retry_after: Option<Duration>,
}

impl RetryMetadata {
    const fn never() -> Self {
        Self {
            retryable: false,
            retry_after: None,
        }
    }

    const fn after(retry_after: Option<Duration>) -> Self {
        Self {
            retryable: true,
            retry_after,
        }
    }

    pub const fn retryable(self) -> bool {
        self.retryable
    }

    pub const fn retry_after(self) -> Option<Duration> {
        self.retry_after
    }
}

/// Errors crossing the model boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelError {
    #[error("model request cancelled")]
    Cancelled,
    #[error("scripted model responses exhausted")]
    ScriptExhausted,
    #[error("model authentication failed")]
    Authentication,
    #[error("model rate limited")]
    RateLimited { retry_after: Option<RetryAfter> },
    #[error("model transport failed")]
    Transport,
    #[error("model protocol failed")]
    Protocol,
}

impl ModelError {
    pub fn rate_limited(retry_after: Option<Duration>) -> Self {
        Self::RateLimited {
            retry_after: retry_after.map(RetryAfter::new),
        }
    }

    pub const fn retry_metadata(&self) -> RetryMetadata {
        match self {
            Self::RateLimited { retry_after } => RetryMetadata::after(match retry_after {
                Some(delay) => Some(delay.as_duration()),
                None => None,
            }),
            Self::Cancelled
            | Self::ScriptExhausted
            | Self::Authentication
            | Self::Transport
            | Self::Protocol => RetryMetadata::never(),
        }
    }
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

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}
