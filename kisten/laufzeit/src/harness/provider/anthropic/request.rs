use orchester_modell::{
    ModelItem, ModelMessage, ModelRequest, ModelRole, ToolCall, MAX_ARGUMENTS_JSON_BYTES,
    MAX_CALL_ID_BYTES, MAX_CONTENT_BYTES,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::harness::provider::json::{self, BoundedJsonError};
use crate::harness::provider::MAX_HTTP_REQUEST_BYTES;

const MAX_MODEL_BYTES: usize = 4 * 1024;
const MAX_MESSAGES: usize = 512;
const MAX_CONTENT_BLOCKS: usize = 1_024;
const MAX_TOOLS: usize = 128;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 16 * 1024;
const MAX_OPTION_BYTES: usize = 64;

/// The reply budget sent when the configuration names none. Anthropic rejects a
/// Messages request that omits `max_tokens`, so the wire always carries one.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 32_000;

/// Optional Messages fields supplied by the effective provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicRequestOptions {
    pub max_output_tokens: u32,
    pub service_tier: Option<String>,
}

impl Default for AnthropicRequestOptions {
    fn default() -> Self {
        Self {
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
            service_tier: None,
        }
    }
}

/// A request rejected before it crosses the model HTTP boundary.
///
/// Variants intentionally carry no provider, prompt, tool, or credential text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AnthropicRequestError {
    #[error("Messages model name is invalid")]
    InvalidModel,
    #[error("Messages history is empty")]
    EmptyMessages,
    #[error("Messages history contains too many turns")]
    TooManyMessages,
    #[error("Messages history is invalid")]
    InvalidMessage,
    #[error("Messages content exceeds its limit")]
    ContentTooLarge,
    #[error("opaque model input cannot be sent to Messages")]
    OpaqueItem,
    #[error("Messages request contains too many tools")]
    TooManyTools,
    #[error("Messages tool definition is invalid")]
    InvalidTool,
    #[error("Messages tool arguments are invalid")]
    InvalidToolArguments,
    #[error("Messages request option is invalid")]
    InvalidOption,
    #[error("Messages request exceeds the HTTP request limit")]
    RequestTooLarge,
    #[error("Messages request serialization failed")]
    Serialization,
}

/// Encode the provider-neutral request into a bounded Messages JSON body.
pub fn encode_anthropic_request(
    request: &ModelRequest,
    options: &AnthropicRequestOptions,
) -> Result<Vec<u8>, AnthropicRequestError> {
    encode_with_stream(request, options, false)
}

/// Encode a Messages request that asks the provider for Server-Sent Events.
pub fn encode_anthropic_stream_request(
    request: &ModelRequest,
    options: &AnthropicRequestOptions,
) -> Result<Vec<u8>, AnthropicRequestError> {
    encode_with_stream(request, options, true)
}

fn encode_with_stream(
    request: &ModelRequest,
    options: &AnthropicRequestOptions,
    stream: bool,
) -> Result<Vec<u8>, AnthropicRequestError> {
    validate_plain_value(&request.model, MAX_MODEL_BYTES)
        .then_some(())
        .ok_or(AnthropicRequestError::InvalidModel)?;
    validate_options(options)?;

    let tools = encode_tools(request)?;
    let (system, messages) = encode_messages(&request.messages)?;
    if messages.is_empty() {
        return Err(AnthropicRequestError::EmptyMessages);
    }

    let wire = WireRequest {
        model: &request.model,
        max_tokens: options.max_output_tokens,
        system: (!system.is_empty()).then_some(system.as_str()),
        messages,
        // Messages rejects a tool choice that names no tools, so the field only
        // appears alongside a declared catalog.
        tool_choice: tools.is_some().then_some(WireToolChoice {
            kind: "auto",
            disable_parallel_tool_use: true,
        }),
        tools,
        stream,
        service_tier: options.service_tier.as_deref(),
    };

    json::to_bounded_vec(&wire, MAX_HTTP_REQUEST_BYTES).map_err(|error| match error {
        BoundedJsonError::LimitExceeded => AnthropicRequestError::RequestTooLarge,
        BoundedJsonError::Serialization => AnthropicRequestError::Serialization,
    })
}

fn validate_options(options: &AnthropicRequestOptions) -> Result<(), AnthropicRequestError> {
    if options.max_output_tokens == 0 {
        return Err(AnthropicRequestError::InvalidOption);
    }
    match options.service_tier.as_deref() {
        Some(tier) if !validate_token(tier, MAX_OPTION_BYTES) => {
            Err(AnthropicRequestError::InvalidOption)
        }
        _ => Ok(()),
    }
}

fn encode_tools(
    request: &ModelRequest,
) -> Result<Option<Vec<WireTool<'_>>>, AnthropicRequestError> {
    if request.tools.len() > MAX_TOOLS {
        return Err(AnthropicRequestError::TooManyTools);
    }
    let mut tools = Vec::with_capacity(request.tools.len());
    for tool in &request.tools {
        if !validate_token(&tool.name, MAX_TOOL_NAME_BYTES)
            || tool.description.len() > MAX_TOOL_DESCRIPTION_BYTES
            || !tool.parameters.is_object()
            || !json::fits(&tool.parameters, MAX_ARGUMENTS_JSON_BYTES)
        {
            return Err(AnthropicRequestError::InvalidTool);
        }
        tools.push(WireTool {
            name: &tool.name,
            description: &tool.description,
            input_schema: &tool.parameters,
        });
    }
    Ok((!tools.is_empty()).then_some(tools))
}

fn encode_messages(
    messages: &[ModelMessage],
) -> Result<(String, Vec<WireMessage<'_>>), AnthropicRequestError> {
    let mut system = String::new();
    let mut wire: Vec<WireMessage<'_>> = Vec::new();
    let mut blocks = 0usize;
    let mut input_started = false;

    for message in messages {
        if message.items.is_empty() {
            return Err(AnthropicRequestError::InvalidMessage);
        }
        match message.role {
            ModelRole::System if !input_started => append_system(&mut system, message)?,
            ModelRole::System => return Err(AnthropicRequestError::InvalidMessage),
            ModelRole::User | ModelRole::Tool => {
                input_started = true;
                let content = match message.role {
                    ModelRole::Tool => encode_tool_results(message)?,
                    _ => encode_user_items(message)?,
                };
                push_message(&mut wire, &mut blocks, "user", content)?;
            }
            ModelRole::Assistant => {
                input_started = true;
                let content = encode_assistant_items(message)?;
                push_message(&mut wire, &mut blocks, "assistant", content)?;
            }
        }
    }
    Ok((system, wire))
}

fn append_system(system: &mut String, message: &ModelMessage) -> Result<(), AnthropicRequestError> {
    for item in &message.items {
        let ModelItem::Text(text) = item else {
            return Err(match item {
                ModelItem::Opaque(_) => AnthropicRequestError::OpaqueItem,
                _ => AnthropicRequestError::InvalidMessage,
            });
        };
        validate_content(text)?;
        let separator_bytes = usize::from(!system.is_empty()) * 2;
        let length = system
            .len()
            .checked_add(separator_bytes)
            .and_then(|length| length.checked_add(text.len()))
            .ok_or(AnthropicRequestError::ContentTooLarge)?;
        if length > MAX_CONTENT_BYTES {
            return Err(AnthropicRequestError::ContentTooLarge);
        }
        if separator_bytes != 0 {
            system.push_str("\n\n");
        }
        system.push_str(text);
    }
    Ok(())
}

fn encode_user_items(
    message: &ModelMessage,
) -> Result<Vec<WireContent<'_>>, AnthropicRequestError> {
    let mut content = Vec::with_capacity(message.items.len());
    for item in &message.items {
        match item {
            ModelItem::Text(text) => {
                validate_content(text)?;
                content.push(WireContent::Text { text });
            }
            ModelItem::Opaque(_) => return Err(AnthropicRequestError::OpaqueItem),
            ModelItem::ToolCall(_) | ModelItem::ToolResult { .. } => {
                return Err(AnthropicRequestError::InvalidMessage);
            }
        }
    }
    Ok(content)
}

fn encode_assistant_items(
    message: &ModelMessage,
) -> Result<Vec<WireContent<'_>>, AnthropicRequestError> {
    let mut content = Vec::with_capacity(message.items.len());
    for item in &message.items {
        match item {
            ModelItem::Text(text) => {
                validate_content(text)?;
                content.push(WireContent::Text { text });
            }
            ModelItem::ToolCall(call) => content.push(encode_tool_call(call)?),
            ModelItem::Opaque(_) => return Err(AnthropicRequestError::OpaqueItem),
            ModelItem::ToolResult { .. } => return Err(AnthropicRequestError::InvalidMessage),
        }
    }
    Ok(content)
}

fn encode_tool_call(call: &ToolCall) -> Result<WireContent<'_>, AnthropicRequestError> {
    if !validate_plain_value(&call.call_id.0, MAX_CALL_ID_BYTES)
        || !validate_token(&call.name, MAX_TOOL_NAME_BYTES)
        || call.arguments_json.len() > MAX_ARGUMENTS_JSON_BYTES
    {
        return Err(AnthropicRequestError::InvalidToolArguments);
    }
    // Messages carries tool arguments as a JSON object rather than as the
    // encoded string the Responses wire uses, so the harness value is parsed
    // here and rejected when it is not an object.
    let Ok(input @ Value::Object(_)) = serde_json::from_str::<Value>(&call.arguments_json) else {
        return Err(AnthropicRequestError::InvalidToolArguments);
    };
    Ok(WireContent::ToolUse {
        id: &call.call_id.0,
        name: &call.name,
        input,
    })
}

fn encode_tool_results(
    message: &ModelMessage,
) -> Result<Vec<WireContent<'_>>, AnthropicRequestError> {
    let mut content = Vec::with_capacity(message.items.len());
    for item in &message.items {
        match item {
            ModelItem::ToolResult { call_id, output } => {
                if !validate_plain_value(&call_id.0, MAX_CALL_ID_BYTES) {
                    return Err(AnthropicRequestError::InvalidMessage);
                }
                validate_content(output)?;
                content.push(WireContent::ToolResult {
                    tool_use_id: &call_id.0,
                    content: output,
                });
            }
            ModelItem::Opaque(_) => return Err(AnthropicRequestError::OpaqueItem),
            ModelItem::Text(_) | ModelItem::ToolCall(_) => {
                return Err(AnthropicRequestError::InvalidMessage);
            }
        }
    }
    Ok(content)
}

fn push_message<'a>(
    wire: &mut Vec<WireMessage<'a>>,
    blocks: &mut usize,
    role: &'static str,
    content: Vec<WireContent<'a>>,
) -> Result<(), AnthropicRequestError> {
    *blocks = blocks
        .checked_add(content.len())
        .filter(|count| *count <= MAX_CONTENT_BLOCKS)
        .ok_or(AnthropicRequestError::TooManyMessages)?;
    // Messages requires its two roles to alternate, and a tool result is a user
    // turn, so a second consecutive turn extends the previous message instead of
    // becoming one the provider rejects.
    if let Some(last) = wire.last_mut() {
        if last.role == role {
            last.content.extend(content);
            return Ok(());
        }
    }
    if wire.len() >= MAX_MESSAGES {
        return Err(AnthropicRequestError::TooManyMessages);
    }
    wire.push(WireMessage { role, content });
    Ok(())
}
fn validate_content(content: &str) -> Result<(), AnthropicRequestError> {
    if content.is_empty() {
        return Err(AnthropicRequestError::InvalidMessage);
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(AnthropicRequestError::ContentTooLarge);
    }
    Ok(())
}

fn validate_plain_value(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn validate_token(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[derive(Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<WireMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireTool<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<WireToolChoice>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<&'a str>,
}

#[derive(Serialize)]
struct WireToolChoice {
    #[serde(rename = "type")]
    kind: &'static str,
    disable_parallel_tool_use: bool,
}

#[derive(Serialize)]
struct WireTool<'a> {
    name: &'a str,
    description: &'a str,
    input_schema: &'a Value,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'static str,
    content: Vec<WireContent<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContent<'a> {
    Text {
        text: &'a str,
    },
    ToolUse {
        id: &'a str,
        name: &'a str,
        input: Value,
    },
    ToolResult {
        tool_use_id: &'a str,
        content: &'a str,
    },
}
