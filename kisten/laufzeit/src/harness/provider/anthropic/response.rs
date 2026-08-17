use futures::StreamExt;
use orchester_modell::{
    ModelEventSink, ModelResponse, ModelUsage, ToolCall, MAX_ARGUMENTS_JSON_BYTES,
    MAX_CALL_ID_BYTES, MAX_CONTENT_BYTES,
};
use orchester_protokoll::CallId;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::harness::provider::json;
use crate::harness::provider::sse::{boundary_length, find_event_boundary, parse_event_frame};
use crate::harness::provider::{HttpResponseStream, HttpTransportError, MAX_HTTP_RESPONSE_BYTES};

const MAX_CONTENT_BLOCKS: usize = 1_024;
const MAX_OPAQUE_ITEMS: usize = 64;
const MAX_OPAQUE_ITEM_BYTES: usize = MAX_CONTENT_BYTES;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_BLOCK_TYPE_BYTES: usize = 64;

/// A Messages body rejected before it reaches the self-agent loop.
///
/// Variants intentionally contain no provider-controlled text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AnthropicResponseError {
    #[error("Messages body exceeds the HTTP response limit")]
    ResponseTooLarge,
    #[error("Messages body is not valid JSON")]
    InvalidJson,
    #[error("Messages body does not match the expected shape")]
    InvalidResponse,
    #[error("Messages content is empty")]
    EmptyContent,
    #[error("Messages content contains too many blocks")]
    TooManyContentBlocks,
    #[error("Messages content block is invalid")]
    InvalidContentBlock,
    #[error("Messages assistant content exceeds its limit")]
    ContentTooLarge,
    #[error("Messages returned more than one tool call")]
    MultipleToolCalls,
    #[error("Messages tool call is invalid")]
    InvalidToolCall,
    #[error("Messages returned too many opaque blocks")]
    TooManyOpaqueItems,
    #[error("Messages opaque block exceeds its limit")]
    OpaqueItemTooLarge,
    #[error("Messages reported an error event")]
    ErrorEvent,
    #[error("Messages event stream is malformed")]
    InvalidEventStream,
    #[error("Messages event stream ended before completion")]
    IncompleteEventStream,
    #[error("Messages event stream was cancelled")]
    Cancelled,
    #[error("Messages event stream transport failed")]
    Transport,
}
/// Decode one bounded, non-streaming Messages result.
pub fn decode_anthropic_response(body: &[u8]) -> Result<ModelResponse, AnthropicResponseError> {
    if body.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(AnthropicResponseError::ResponseTooLarge);
    }

    let wire: WireResponse = serde_json::from_slice(body).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            AnthropicResponseError::InvalidJson
        } else {
            AnthropicResponseError::InvalidResponse
        }
    })?;
    if wire.kind.as_deref().is_some_and(|kind| kind != "message") {
        return Err(AnthropicResponseError::InvalidResponse);
    }
    if wire.content.is_empty() {
        return Err(AnthropicResponseError::EmptyContent);
    }
    if wire.content.len() > MAX_CONTENT_BLOCKS {
        return Err(AnthropicResponseError::TooManyContentBlocks);
    }

    let mut decoded = ResponseBuilder::default();
    if let Some(usage) = &wire.usage {
        decoded.record_usage(usage);
    }
    for block in wire.content {
        decode_content_block(block, &mut decoded)?;
    }
    Ok(decoded.into_response())
}

fn decode_content_block(
    block: Value,
    decoded: &mut ResponseBuilder,
) -> Result<(), AnthropicResponseError> {
    decoded.count_block()?;
    let kind = block_kind(&block)?;
    match kind {
        "text" => {
            let text = block
                .get("text")
                .and_then(Value::as_str)
                .ok_or(AnthropicResponseError::InvalidContentBlock)?;
            decoded.push_text(text, None)
        }
        "tool_use" => {
            let (id, name, arguments) = parse_tool_use(&block)?;
            decoded.set_tool_call(id, name, arguments)
        }
        _ => decoded.push_opaque(block),
    }
}

fn block_kind(block: &Value) -> Result<&str, AnthropicResponseError> {
    block
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| validate_token(kind, MAX_BLOCK_TYPE_BYTES))
        .ok_or(AnthropicResponseError::InvalidContentBlock)
}
fn parse_tool_identity(block: &Value) -> Result<(String, String), AnthropicResponseError> {
    let id = block
        .get("id")
        .and_then(Value::as_str)
        .ok_or(AnthropicResponseError::InvalidToolCall)?;
    let name = block
        .get("name")
        .and_then(Value::as_str)
        .ok_or(AnthropicResponseError::InvalidToolCall)?;
    if !validate_plain_value(id, MAX_CALL_ID_BYTES) || !validate_token(name, MAX_TOOL_NAME_BYTES) {
        return Err(AnthropicResponseError::InvalidToolCall);
    }
    Ok((id.to_owned(), name.to_owned()))
}

fn parse_tool_use(block: &Value) -> Result<(String, String, String), AnthropicResponseError> {
    let (id, name) = parse_tool_identity(block)?;
    let input = block
        .get("input")
        .filter(|input| input.is_object())
        .ok_or(AnthropicResponseError::InvalidToolCall)?;
    if !json::fits(input, MAX_ARGUMENTS_JSON_BYTES) {
        return Err(AnthropicResponseError::InvalidToolCall);
    }
    // The harness carries tool arguments as an encoded object, so the Messages
    // object is rendered back into the string the tool registry validates.
    let arguments =
        serde_json::to_string(input).map_err(|_| AnthropicResponseError::InvalidToolCall)?;
    Ok((id, name, arguments))
}

/// Decode a bounded Messages Server-Sent Events body and forward text deltas.
pub async fn decode_anthropic_event_stream(
    mut response: HttpResponseStream,
    cancel: CancellationToken,
    events: Option<&dyn ModelEventSink>,
) -> Result<ModelResponse, AnthropicResponseError> {
    let mut pending = Vec::new();
    let mut state = StreamState::default();

    while let Some(chunk) = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(AnthropicResponseError::Cancelled),
        chunk = response.next() => chunk,
    } {
        let chunk = chunk.map_err(map_stream_error)?;
        pending.extend_from_slice(&chunk);
        if pending.len() > MAX_HTTP_RESPONSE_BYTES {
            return Err(AnthropicResponseError::ResponseTooLarge);
        }
        while let Some(frame_end) = find_event_boundary(&pending) {
            let frame = pending.drain(..frame_end).collect::<Vec<_>>();
            let frame = &frame[..frame
                .len()
                .saturating_sub(boundary_length(frame.as_slice()))];
            state.decode_frame(frame, events)?;
        }
    }

    if !pending.iter().all(u8::is_ascii_whitespace) {
        state.decode_frame(&pending, events)?;
    }
    state.finish()
}

fn map_stream_error(error: HttpTransportError) -> AnthropicResponseError {
    match error {
        HttpTransportError::ResponseTooLarge => AnthropicResponseError::ResponseTooLarge,
        HttpTransportError::Cancelled => AnthropicResponseError::Cancelled,
        _ => AnthropicResponseError::Transport,
    }
}
/// One response under construction, enforcing the harness content bounds once
/// for both the unary body and the event stream.
#[derive(Default)]
struct ResponseBuilder {
    assistant_text: String,
    tool_call: Option<ToolCall>,
    opaque_items: Vec<Value>,
    usage: ModelUsage,
    blocks: usize,
}

impl ResponseBuilder {
    fn count_block(&mut self) -> Result<(), AnthropicResponseError> {
        self.blocks = self
            .blocks
            .checked_add(1)
            .filter(|count| *count <= MAX_CONTENT_BLOCKS)
            .ok_or(AnthropicResponseError::TooManyContentBlocks)?;
        Ok(())
    }

    fn push_text(
        &mut self,
        text: &str,
        events: Option<&dyn ModelEventSink>,
    ) -> Result<(), AnthropicResponseError> {
        let length = self
            .assistant_text
            .len()
            .checked_add(text.len())
            .ok_or(AnthropicResponseError::ContentTooLarge)?;
        if length > MAX_CONTENT_BYTES {
            return Err(AnthropicResponseError::ContentTooLarge);
        }
        self.assistant_text.push_str(text);
        // Messages has no terminal frame that repeats the whole message, so the
        // stream is both accumulated here and forwarded once as it arrives.
        if let Some(events) = events {
            if !text.is_empty() {
                events.text_delta(text);
            }
        }
        Ok(())
    }

    fn set_tool_call(
        &mut self,
        id: String,
        name: String,
        arguments: String,
    ) -> Result<(), AnthropicResponseError> {
        if self.tool_call.is_some() {
            return Err(AnthropicResponseError::MultipleToolCalls);
        }
        if arguments.len() > MAX_ARGUMENTS_JSON_BYTES {
            return Err(AnthropicResponseError::InvalidToolCall);
        }
        let Ok(Value::Object(_)) = serde_json::from_str::<Value>(&arguments) else {
            return Err(AnthropicResponseError::InvalidToolCall);
        };
        self.tool_call = Some(ToolCall::new(CallId::from(id.as_str()), name, arguments));
        Ok(())
    }

    fn push_opaque(&mut self, block: Value) -> Result<(), AnthropicResponseError> {
        if self.opaque_items.len() >= MAX_OPAQUE_ITEMS {
            return Err(AnthropicResponseError::TooManyOpaqueItems);
        }
        if !json::fits(&block, MAX_OPAQUE_ITEM_BYTES) {
            return Err(AnthropicResponseError::OpaqueItemTooLarge);
        }
        self.opaque_items.push(block);
        Ok(())
    }

    fn record_usage(&mut self, usage: &WireUsage) {
        if let Some(tokens) = usage.input_tokens {
            self.usage.input_tokens = tokens;
        }
        if let Some(tokens) = usage.output_tokens {
            self.usage.output_tokens = tokens;
        }
    }

    fn into_response(self) -> ModelResponse {
        ModelResponse {
            assistant_text: self.assistant_text,
            tool_call: self.tool_call,
            usage: self.usage,
            opaque_items: self.opaque_items,
        }
    }
}
/// The event-stream cursor: one builder plus the block currently open.
#[derive(Default)]
struct StreamState {
    builder: ResponseBuilder,
    active: Option<ActiveBlock>,
    stopped: bool,
    completed: bool,
}

enum ActiveBlock {
    Text,
    ToolUse {
        id: String,
        name: String,
        arguments: String,
    },
    Opaque(Value),
}

impl StreamState {
    fn decode_frame(
        &mut self,
        frame: &[u8],
        events: Option<&dyn ModelEventSink>,
    ) -> Result<(), AnthropicResponseError> {
        let Some(frame) =
            parse_event_frame(frame).map_err(|_| AnthropicResponseError::InvalidEventStream)?
        else {
            return Ok(());
        };
        if self.completed {
            return Err(AnthropicResponseError::InvalidEventStream);
        }
        let value: Value = serde_json::from_str(&frame.data)
            .map_err(|_| AnthropicResponseError::InvalidEventStream)?;
        let kind = frame
            .name
            .or_else(|| value.get("type").and_then(Value::as_str))
            .ok_or(AnthropicResponseError::InvalidEventStream)?;

        match kind {
            "message_start" => {
                self.record_usage(value.pointer("/message/usage"));
                Ok(())
            }
            "content_block_start" => self.start_block(
                value
                    .get("content_block")
                    .ok_or(AnthropicResponseError::InvalidEventStream)?,
            ),
            "content_block_delta" => self.apply_delta(
                value
                    .get("delta")
                    .ok_or(AnthropicResponseError::InvalidEventStream)?,
                events,
            ),
            "content_block_stop" => self.stop_block(),
            "message_delta" => {
                self.stopped |= value
                    .pointer("/delta/stop_reason")
                    .and_then(Value::as_str)
                    .is_some();
                self.record_usage(value.get("usage"));
                Ok(())
            }
            "message_stop" => {
                self.completed = true;
                Ok(())
            }
            "error" => Err(AnthropicResponseError::ErrorEvent),
            // `ping` and any event named after this release carry no state the
            // harness reads, and dropping them keeps a new frame name from
            // failing a turn that already succeeded.
            _ => Ok(()),
        }
    }

    fn record_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage else {
            return;
        };
        if let Ok(usage) = serde_json::from_value::<WireUsage>(usage.clone()) {
            self.builder.record_usage(&usage);
        }
    }
}
impl StreamState {
    fn start_block(&mut self, block: &Value) -> Result<(), AnthropicResponseError> {
        if self.active.is_some() {
            return Err(AnthropicResponseError::InvalidEventStream);
        }
        self.builder.count_block()?;
        self.active = Some(match block_kind(block)? {
            "text" => ActiveBlock::Text,
            "tool_use" => {
                let (id, name) = parse_tool_identity(block)?;
                ActiveBlock::ToolUse {
                    id,
                    name,
                    arguments: String::new(),
                }
            }
            _ => ActiveBlock::Opaque(block.clone()),
        });
        Ok(())
    }

    fn apply_delta(
        &mut self,
        delta: &Value,
        events: Option<&dyn ModelEventSink>,
    ) -> Result<(), AnthropicResponseError> {
        let kind = delta
            .get("type")
            .and_then(Value::as_str)
            .ok_or(AnthropicResponseError::InvalidEventStream)?;
        match kind {
            "text_delta" => {
                if !matches!(self.active, Some(ActiveBlock::Text)) {
                    return Err(AnthropicResponseError::InvalidEventStream);
                }
                let text = delta
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(AnthropicResponseError::InvalidEventStream)?;
                self.builder.push_text(text, events)
            }
            "input_json_delta" => {
                let partial = delta
                    .get("partial_json")
                    .and_then(Value::as_str)
                    .ok_or(AnthropicResponseError::InvalidEventStream)?;
                let Some(ActiveBlock::ToolUse { arguments, .. }) = &mut self.active else {
                    return Err(AnthropicResponseError::InvalidEventStream);
                };
                let length = arguments
                    .len()
                    .checked_add(partial.len())
                    .ok_or(AnthropicResponseError::InvalidToolCall)?;
                if length > MAX_ARGUMENTS_JSON_BYTES {
                    return Err(AnthropicResponseError::InvalidToolCall);
                }
                arguments.push_str(partial);
                Ok(())
            }
            // Thinking and signature deltas belong to a block the harness keeps
            // opaque, so they are counted but not read.
            _ if self.active.is_some() => Ok(()),
            _ => Err(AnthropicResponseError::InvalidEventStream),
        }
    }

    fn stop_block(&mut self) -> Result<(), AnthropicResponseError> {
        match self
            .active
            .take()
            .ok_or(AnthropicResponseError::InvalidEventStream)?
        {
            ActiveBlock::Text => Ok(()),
            ActiveBlock::ToolUse {
                id,
                name,
                arguments,
            } => {
                // A tool invoked without arguments streams no partial JSON at
                // all, which is an empty object rather than a malformed call.
                let arguments = if arguments.trim().is_empty() {
                    "{}".to_owned()
                } else {
                    arguments
                };
                self.builder.set_tool_call(id, name, arguments)
            }
            ActiveBlock::Opaque(block) => self.builder.push_opaque(block),
        }
    }

    fn finish(self) -> Result<ModelResponse, AnthropicResponseError> {
        if self.active.is_some() || !(self.completed || self.stopped) {
            return Err(AnthropicResponseError::IncompleteEventStream);
        }
        Ok(self.builder.into_response())
    }
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

#[derive(Deserialize)]
struct WireResponse {
    #[serde(rename = "type")]
    kind: Option<String>,
    content: Vec<Value>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct WireUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
}
