use std::io::{self, Write};

use orchester_modell::{
    MAX_ARGUMENTS_JSON_BYTES, MAX_CALL_ID_BYTES, MAX_CONTENT_BYTES, ModelItem, ModelMessage,
    ModelRequest, ModelRole, ToolCall,
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::harness::provider::MAX_HTTP_REQUEST_BYTES;

const MAX_MODEL_BYTES: usize = 4 * 1024;
const MAX_INPUT_ITEMS: usize = 512;
const MAX_TOOLS: usize = 128;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_TOOL_DESCRIPTION_BYTES: usize = 16 * 1024;
const MAX_OPTION_BYTES: usize = 64;

/// Optional Responses fields supplied by the effective provider configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponsesRequestOptions {
    pub reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
}

/// A request rejected before it crosses the model HTTP boundary.
///
/// Variants intentionally carry no provider, prompt, tool, or credential text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ResponsesRequestError {
    #[error("Responses model name is invalid")]
    InvalidModel,
    #[error("Responses input is empty")]
    EmptyInput,
    #[error("Responses input contains too many items")]
    TooManyInputItems,
    #[error("Responses message history is invalid")]
    InvalidMessage,
    #[error("Responses message content exceeds its limit")]
    ContentTooLarge,
    #[error("opaque model input cannot be sent to Responses")]
    OpaqueItem,
    #[error("Responses request contains too many tools")]
    TooManyTools,
    #[error("Responses tool definition is invalid")]
    InvalidTool,
    #[error("Responses tool arguments are invalid")]
    InvalidToolArguments,
    #[error("Responses request option is invalid")]
    InvalidOption,
    #[error("Responses request exceeds the HTTP request limit")]
    RequestTooLarge,
    #[error("Responses request serialization failed")]
    Serialization,
}

/// Encode the provider-neutral request into a bounded Responses JSON body.
pub fn encode_responses_request(
    request: &ModelRequest,
    options: &ResponsesRequestOptions,
) -> Result<Vec<u8>, ResponsesRequestError> {
    validate_plain_value(&request.model, MAX_MODEL_BYTES)
        .then_some(())
        .ok_or(ResponsesRequestError::InvalidModel)?;
    validate_options(options)?;

    let tools = encode_tools(request)?;
    let (instructions, input) = encode_messages(&request.messages)?;
    if input.is_empty() {
        return Err(ResponsesRequestError::EmptyInput);
    }

    let wire = WireRequest {
        model: &request.model,
        instructions: (!instructions.is_empty()).then_some(instructions.as_str()),
        input,
        tools,
        tool_choice: "auto",
        parallel_tool_calls: false,
        reasoning: options
            .reasoning_effort
            .as_deref()
            .map(|effort| WireReasoning { effort }),
        store: request.store,
        stream: false,
        service_tier: options.service_tier.as_deref(),
    };

    let mut output = BoundedBuffer::new(MAX_HTTP_REQUEST_BYTES);
    if serde_json::to_writer(&mut output, &wire).is_err() {
        return Err(if output.exceeded {
            ResponsesRequestError::RequestTooLarge
        } else {
            ResponsesRequestError::Serialization
        });
    }
    Ok(output.bytes)
}

fn validate_options(options: &ResponsesRequestOptions) -> Result<(), ResponsesRequestError> {
    for value in [
        options.reasoning_effort.as_deref(),
        options.service_tier.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !validate_token(value, MAX_OPTION_BYTES) {
            return Err(ResponsesRequestError::InvalidOption);
        }
    }
    Ok(())
}

fn encode_tools(
    request: &ModelRequest,
) -> Result<Option<Vec<WireTool<'_>>>, ResponsesRequestError> {
    if request.tools.len() > MAX_TOOLS {
        return Err(ResponsesRequestError::TooManyTools);
    }

    let mut tools = Vec::with_capacity(request.tools.len());
    for tool in &request.tools {
        if !validate_token(&tool.name, MAX_TOOL_NAME_BYTES)
            || tool.description.len() > MAX_TOOL_DESCRIPTION_BYTES
            || !tool.parameters.is_object()
            || !json_fits(&tool.parameters, MAX_ARGUMENTS_JSON_BYTES)
        {
            return Err(ResponsesRequestError::InvalidTool);
        }
        tools.push(WireTool {
            kind: "function",
            name: &tool.name,
            description: &tool.description,
            parameters: &tool.parameters,
            strict: true,
        });
    }

    Ok((!tools.is_empty()).then_some(tools))
}

fn encode_messages(
    messages: &[ModelMessage],
) -> Result<(String, Vec<WireInputItem<'_>>), ResponsesRequestError> {
    let mut instructions = String::new();
    let mut input = Vec::new();
    let mut input_started = false;

    for message in messages {
        if message.items.is_empty() {
            return Err(ResponsesRequestError::InvalidMessage);
        }
        match message.role {
            ModelRole::System if !input_started => {
                append_instructions(&mut instructions, message)?;
            }
            ModelRole::System => return Err(ResponsesRequestError::InvalidMessage),
            ModelRole::User => {
                input_started = true;
                encode_text_message(message, "user", TextKind::Input, &mut input)?;
            }
            ModelRole::Assistant => {
                input_started = true;
                encode_assistant_message(message, &mut input)?;
            }
            ModelRole::Tool => {
                input_started = true;
                encode_tool_results(message, &mut input)?;
            }
        }
    }
    Ok((instructions, input))
}

fn append_instructions(
    instructions: &mut String,
    message: &ModelMessage,
) -> Result<(), ResponsesRequestError> {
    for item in &message.items {
        let ModelItem::Text(text) = item else {
            return Err(match item {
                ModelItem::Opaque(_) => ResponsesRequestError::OpaqueItem,
                _ => ResponsesRequestError::InvalidMessage,
            });
        };
        validate_content(text)?;
        let separator_bytes = usize::from(!instructions.is_empty()) * 2;
        let new_len = instructions
            .len()
            .checked_add(separator_bytes)
            .and_then(|length| length.checked_add(text.len()))
            .ok_or(ResponsesRequestError::ContentTooLarge)?;
        if new_len > MAX_CONTENT_BYTES {
            return Err(ResponsesRequestError::ContentTooLarge);
        }
        if separator_bytes != 0 {
            instructions.push_str("\n\n");
        }
        instructions.push_str(text);
    }
    Ok(())
}

fn encode_text_message<'a>(
    message: &'a ModelMessage,
    role: &'static str,
    kind: TextKind,
    input: &mut Vec<WireInputItem<'a>>,
) -> Result<(), ResponsesRequestError> {
    let mut content = Vec::with_capacity(message.items.len());
    for item in &message.items {
        match item {
            ModelItem::Text(text) => {
                validate_content(text)?;
                content.push(kind.wire(text));
            }
            ModelItem::Opaque(_) => return Err(ResponsesRequestError::OpaqueItem),
            ModelItem::ToolCall(_) | ModelItem::ToolResult { .. } => {
                return Err(ResponsesRequestError::InvalidMessage);
            }
        }
    }
    push_input(input, WireInputItem::Message { role, content })
}

fn encode_assistant_message<'a>(
    message: &'a ModelMessage,
    input: &mut Vec<WireInputItem<'a>>,
) -> Result<(), ResponsesRequestError> {
    let mut text = Vec::new();
    for item in &message.items {
        match item {
            ModelItem::Text(value) => {
                validate_content(value)?;
                text.push(WireContent::OutputText { text: value });
            }
            ModelItem::ToolCall(call) => {
                flush_assistant_text(input, &mut text)?;
                validate_tool_call(call)?;
                push_input(
                    input,
                    WireInputItem::FunctionCall {
                        call_id: &call.call_id.0,
                        name: &call.name,
                        arguments: &call.arguments_json,
                    },
                )?;
            }
            ModelItem::Opaque(_) => return Err(ResponsesRequestError::OpaqueItem),
            ModelItem::ToolResult { .. } => return Err(ResponsesRequestError::InvalidMessage),
        }
    }
    flush_assistant_text(input, &mut text)
}

fn flush_assistant_text<'a>(
    input: &mut Vec<WireInputItem<'a>>,
    text: &mut Vec<WireContent<'a>>,
) -> Result<(), ResponsesRequestError> {
    if text.is_empty() {
        return Ok(());
    }
    push_input(
        input,
        WireInputItem::Message {
            role: "assistant",
            content: std::mem::take(text),
        },
    )
}

fn encode_tool_results<'a>(
    message: &'a ModelMessage,
    input: &mut Vec<WireInputItem<'a>>,
) -> Result<(), ResponsesRequestError> {
    for item in &message.items {
        match item {
            ModelItem::ToolResult { call_id, output } => {
                if !validate_plain_value(&call_id.0, MAX_CALL_ID_BYTES) {
                    return Err(ResponsesRequestError::InvalidMessage);
                }
                validate_content(output)?;
                push_input(
                    input,
                    WireInputItem::FunctionCallOutput {
                        call_id: &call_id.0,
                        output,
                    },
                )?;
            }
            ModelItem::Opaque(_) => return Err(ResponsesRequestError::OpaqueItem),
            ModelItem::Text(_) | ModelItem::ToolCall(_) => {
                return Err(ResponsesRequestError::InvalidMessage);
            }
        }
    }
    Ok(())
}

fn validate_tool_call(call: &ToolCall) -> Result<(), ResponsesRequestError> {
    if !validate_plain_value(&call.call_id.0, MAX_CALL_ID_BYTES)
        || !validate_token(&call.name, MAX_TOOL_NAME_BYTES)
        || call.arguments_json.len() > MAX_ARGUMENTS_JSON_BYTES
    {
        return Err(ResponsesRequestError::InvalidToolArguments);
    }
    let Ok(Value::Object(_)) = serde_json::from_str::<Value>(&call.arguments_json) else {
        return Err(ResponsesRequestError::InvalidToolArguments);
    };
    Ok(())
}

fn validate_content(content: &str) -> Result<(), ResponsesRequestError> {
    if content.is_empty() {
        return Err(ResponsesRequestError::InvalidMessage);
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(ResponsesRequestError::ContentTooLarge);
    }
    Ok(())
}

fn push_input<'a>(
    input: &mut Vec<WireInputItem<'a>>,
    item: WireInputItem<'a>,
) -> Result<(), ResponsesRequestError> {
    if input.len() >= MAX_INPUT_ITEMS {
        return Err(ResponsesRequestError::TooManyInputItems);
    }
    input.push(item);
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

fn json_fits(value: &Value, limit: usize) -> bool {
    let mut counter = BoundedCounter::new(limit);
    serde_json::to_writer(&mut counter, value).is_ok()
}

#[derive(Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    input: Vec<WireInputItem<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireTool<'a>>>,
    tool_choice: &'static str,
    parallel_tool_calls: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<WireReasoning<'a>>,
    store: bool,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<&'a str>,
}

#[derive(Serialize)]
struct WireReasoning<'a> {
    effort: &'a str,
}

#[derive(Serialize)]
struct WireTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'a str,
    description: &'a str,
    parameters: &'a Value,
    strict: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireInputItem<'a> {
    Message {
        role: &'static str,
        content: Vec<WireContent<'a>>,
    },
    FunctionCall {
        call_id: &'a str,
        name: &'a str,
        arguments: &'a str,
    },
    FunctionCallOutput {
        call_id: &'a str,
        output: &'a str,
    },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContent<'a> {
    InputText { text: &'a str },
    OutputText { text: &'a str },
}

#[derive(Clone, Copy)]
enum TextKind {
    Input,
}

impl TextKind {
    fn wire<'a>(self, text: &'a str) -> WireContent<'a> {
        match self {
            Self::Input => WireContent::InputText { text },
        }
    }
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            exceeded: false,
        }
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("bounded JSON buffer exceeded"));
        };
        if next_len > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("bounded JSON buffer exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BoundedCounter {
    bytes: usize,
    limit: usize,
}

impl BoundedCounter {
    const fn new(limit: usize) -> Self {
        Self { bytes: 0, limit }
    }
}

impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.checked_add(bytes.len()) else {
            return Err(io::Error::other("bounded JSON counter exceeded"));
        };
        if next_len > self.limit {
            return Err(io::Error::other("bounded JSON counter exceeded"));
        }
        self.bytes = next_len;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
