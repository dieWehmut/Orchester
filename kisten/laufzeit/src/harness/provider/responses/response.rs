use futures::StreamExt;
use orchester_modell::{
    ModelEventSink, ModelResponse, ModelUsage, ToolCall, MAX_ARGUMENTS_JSON_BYTES,
    MAX_CALL_ID_BYTES, MAX_CONTENT_BYTES,
};
use orchester_protokoll::CallId;
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::harness::provider::{HttpResponseStream, HttpTransportError, MAX_HTTP_RESPONSE_BYTES};
use tokio_util::sync::CancellationToken;

use super::json;

const MAX_OUTPUT_ITEMS: usize = 512;
const MAX_CONTENT_ITEMS: usize = 1_024;
const MAX_OPAQUE_ITEMS: usize = 64;
const MAX_OPAQUE_ITEM_BYTES: usize = MAX_CONTENT_BYTES;
const MAX_TOOL_NAME_BYTES: usize = 64;
const MAX_ITEM_TYPE_BYTES: usize = 64;

/// A Responses body rejected before it reaches the self-agent loop.
///
/// Variants intentionally contain no provider-controlled text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ResponsesResponseError {
    #[error("Responses body exceeds the HTTP response limit")]
    ResponseTooLarge,
    #[error("Responses body is not valid JSON")]
    InvalidJson,
    #[error("Responses body does not match the expected shape")]
    InvalidResponse,
    #[error("Responses result is not complete")]
    InvalidStatus,
    #[error("Responses output is empty")]
    EmptyOutput,
    #[error("Responses output contains too many items")]
    TooManyOutputItems,
    #[error("Responses output item is invalid")]
    InvalidOutputItem,
    #[error("Responses assistant message is invalid")]
    InvalidMessage,
    #[error("Responses assistant content exceeds its limit")]
    ContentTooLarge,
    #[error("Responses returned more than one tool call")]
    MultipleToolCalls,
    #[error("Responses tool call is invalid")]
    InvalidToolCall,
    #[error("Responses returned too many opaque items")]
    TooManyOpaqueItems,
    #[error("Responses opaque item exceeds its limit")]
    OpaqueItemTooLarge,
    #[error("Responses event stream is malformed")]
    InvalidEventStream,
    #[error("Responses event stream ended before completion")]
    IncompleteEventStream,
    #[error("Responses event stream was cancelled")]
    Cancelled,
    #[error("Responses event stream transport failed")]
    Transport,
}

/// Decode one bounded, non-streaming Responses result.
pub fn decode_responses_response(body: &[u8]) -> Result<ModelResponse, ResponsesResponseError> {
    if body.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(ResponsesResponseError::ResponseTooLarge);
    }

    let wire: WireResponse = serde_json::from_slice(body).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            ResponsesResponseError::InvalidJson
        } else {
            ResponsesResponseError::InvalidResponse
        }
    })?;
    if wire
        .status
        .as_deref()
        .is_some_and(|status| status != "completed")
    {
        return Err(ResponsesResponseError::InvalidStatus);
    }
    if wire.output.is_empty() {
        return Err(ResponsesResponseError::EmptyOutput);
    }
    if wire.output.len() > MAX_OUTPUT_ITEMS {
        return Err(ResponsesResponseError::TooManyOutputItems);
    }

    let usage = wire.usage.unwrap_or_default();
    let mut decoded = ResponseBuilder::default();
    for item in wire.output {
        decode_output_item(item, &mut decoded)?;
    }

    Ok(ModelResponse {
        assistant_text: decoded.assistant_text,
        tool_call: decoded.tool_call,
        usage: ModelUsage {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
        },
        opaque_items: decoded.opaque_items,
    })
}

/// Decode a bounded Responses Server-Sent Events body and forward text deltas.
pub async fn decode_responses_event_stream(
    mut response: HttpResponseStream,
    cancel: CancellationToken,
    events: Option<&dyn ModelEventSink>,
) -> Result<ModelResponse, ResponsesResponseError> {
    let mut pending = Vec::new();
    let mut emitted_bytes = 0usize;
    let mut completed = None;

    while let Some(chunk) = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(ResponsesResponseError::Cancelled),
        chunk = response.next() => chunk,
    } {
        let chunk = chunk.map_err(map_stream_error)?;
        pending.extend_from_slice(&chunk);
        if pending.len() > MAX_HTTP_RESPONSE_BYTES {
            return Err(ResponsesResponseError::ResponseTooLarge);
        }
        while let Some(frame_end) = find_event_boundary(&pending) {
            let frame = pending.drain(..frame_end).collect::<Vec<_>>();
            let frame = &frame[..frame
                .len()
                .saturating_sub(boundary_length(frame.as_slice()))];
            if completed.is_some() && !frame.iter().all(u8::is_ascii_whitespace) {
                return Err(ResponsesResponseError::InvalidEventStream);
            }
            if let Some(response) = decode_event_frame(frame, &mut emitted_bytes, events)? {
                if completed.replace(response).is_some() {
                    return Err(ResponsesResponseError::InvalidEventStream);
                }
            }
        }
    }

    if !pending.iter().all(u8::is_ascii_whitespace) {
        if completed.is_some() {
            return Err(ResponsesResponseError::InvalidEventStream);
        }
        if let Some(response) = decode_event_frame(&pending, &mut emitted_bytes, events)? {
            if completed.replace(response).is_some() {
                return Err(ResponsesResponseError::InvalidEventStream);
            }
        }
    }
    completed.ok_or(ResponsesResponseError::IncompleteEventStream)
}

fn map_stream_error(error: HttpTransportError) -> ResponsesResponseError {
    match error {
        HttpTransportError::ResponseTooLarge => ResponsesResponseError::ResponseTooLarge,
        HttpTransportError::Cancelled => ResponsesResponseError::Cancelled,
        _ => ResponsesResponseError::Transport,
    }
}

fn find_event_boundary(bytes: &[u8]) -> Option<usize> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| index + 2);
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4);
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(index), None) | (None, Some(index)) => Some(index),
        (None, None) => None,
    }
}

fn boundary_length(bytes: &[u8]) -> usize {
    if bytes.ends_with(b"\r\n\r\n") {
        4
    } else {
        2
    }
}

fn decode_event_frame(
    frame: &[u8],
    emitted_bytes: &mut usize,
    events: Option<&dyn ModelEventSink>,
) -> Result<Option<ModelResponse>, ResponsesResponseError> {
    let frame =
        std::str::from_utf8(frame).map_err(|_| ResponsesResponseError::InvalidEventStream)?;
    let mut event_name = None;
    let mut data = String::new();
    for line in frame.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event_name = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        } else if !line.trim().is_empty() {
            return Err(ResponsesResponseError::InvalidEventStream);
        }
    }
    if data.is_empty() {
        return Ok(None);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|_| ResponsesResponseError::InvalidEventStream)?;
    let kind = event_name
        .or_else(|| value.get("type").and_then(Value::as_str))
        .ok_or(ResponsesResponseError::InvalidEventStream)?;
    match kind {
        "response.output_text.delta" => {
            let delta = value
                .get("delta")
                .and_then(Value::as_str)
                .ok_or(ResponsesResponseError::InvalidEventStream)?;
            *emitted_bytes = emitted_bytes
                .checked_add(delta.len())
                .ok_or(ResponsesResponseError::ContentTooLarge)?;
            if *emitted_bytes > MAX_CONTENT_BYTES {
                return Err(ResponsesResponseError::ContentTooLarge);
            }
            if let Some(events) = events {
                events.text_delta(delta);
            }
            Ok(None)
        }
        "response.completed" => {
            let response = value
                .get("response")
                .ok_or(ResponsesResponseError::InvalidEventStream)?;
            let body = serde_json::to_vec(response)
                .map_err(|_| ResponsesResponseError::InvalidEventStream)?;
            decode_responses_response(&body).map(Some)
        }
        "response.failed" => Err(ResponsesResponseError::InvalidStatus),
        _ => Ok(None),
    }
}

fn decode_output_item(
    item: Value,
    decoded: &mut ResponseBuilder,
) -> Result<(), ResponsesResponseError> {
    let kind = item
        .as_object()
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str)
        .ok_or(ResponsesResponseError::InvalidOutputItem)?;
    if !validate_token(kind, MAX_ITEM_TYPE_BYTES) {
        return Err(ResponsesResponseError::InvalidOutputItem);
    }

    match kind {
        "message" => decode_message(&item, decoded),
        "function_call" => decode_tool_call(&item, decoded),
        _ => push_opaque(item, decoded),
    }
}

fn decode_message(
    item: &Value,
    decoded: &mut ResponseBuilder,
) -> Result<(), ResponsesResponseError> {
    let object = item
        .as_object()
        .ok_or(ResponsesResponseError::InvalidMessage)?;
    validate_optional_completed(object, ResponsesResponseError::InvalidMessage)?;
    if object.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(ResponsesResponseError::InvalidMessage);
    }
    let content = object
        .get("content")
        .and_then(Value::as_array)
        .filter(|content| !content.is_empty())
        .ok_or(ResponsesResponseError::InvalidMessage)?;

    for content_item in content {
        if decoded.content_items >= MAX_CONTENT_ITEMS {
            return Err(ResponsesResponseError::TooManyOutputItems);
        }
        decoded.content_items += 1;
        decode_content_item(content_item, decoded)?;
    }
    Ok(())
}

fn decode_content_item(
    item: &Value,
    decoded: &mut ResponseBuilder,
) -> Result<(), ResponsesResponseError> {
    let object = item
        .as_object()
        .ok_or(ResponsesResponseError::InvalidMessage)?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| validate_token(kind, MAX_ITEM_TYPE_BYTES))
        .ok_or(ResponsesResponseError::InvalidMessage)?;

    match kind {
        "output_text" => append_text(
            object
                .get("text")
                .and_then(Value::as_str)
                .ok_or(ResponsesResponseError::InvalidMessage)?,
            decoded,
        ),
        "refusal" => append_text(
            object
                .get("refusal")
                .and_then(Value::as_str)
                .ok_or(ResponsesResponseError::InvalidMessage)?,
            decoded,
        ),
        "input_text" => Err(ResponsesResponseError::InvalidMessage),
        _ => push_opaque(item.clone(), decoded),
    }
}

fn append_text(text: &str, decoded: &mut ResponseBuilder) -> Result<(), ResponsesResponseError> {
    let length = decoded
        .assistant_text
        .len()
        .checked_add(text.len())
        .ok_or(ResponsesResponseError::ContentTooLarge)?;
    if length > MAX_CONTENT_BYTES {
        return Err(ResponsesResponseError::ContentTooLarge);
    }
    decoded.assistant_text.push_str(text);
    Ok(())
}

fn decode_tool_call(
    item: &Value,
    decoded: &mut ResponseBuilder,
) -> Result<(), ResponsesResponseError> {
    if decoded.tool_call.is_some() {
        return Err(ResponsesResponseError::MultipleToolCalls);
    }
    let object = item
        .as_object()
        .ok_or(ResponsesResponseError::InvalidToolCall)?;
    validate_optional_completed(object, ResponsesResponseError::InvalidToolCall)?;
    let call_id = object
        .get("call_id")
        .and_then(Value::as_str)
        .ok_or(ResponsesResponseError::InvalidToolCall)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or(ResponsesResponseError::InvalidToolCall)?;
    let arguments = object
        .get("arguments")
        .and_then(Value::as_str)
        .ok_or(ResponsesResponseError::InvalidToolCall)?;
    if !validate_plain_value(call_id, MAX_CALL_ID_BYTES)
        || !validate_token(name, MAX_TOOL_NAME_BYTES)
        || arguments.len() > MAX_ARGUMENTS_JSON_BYTES
    {
        return Err(ResponsesResponseError::InvalidToolCall);
    }
    let Ok(Value::Object(_)) = serde_json::from_str::<Value>(arguments) else {
        return Err(ResponsesResponseError::InvalidToolCall);
    };

    decoded.tool_call = Some(ToolCall::new(
        CallId::from(call_id),
        name.to_owned(),
        arguments.to_owned(),
    ));
    Ok(())
}

fn validate_optional_completed(
    object: &Map<String, Value>,
    error: ResponsesResponseError,
) -> Result<(), ResponsesResponseError> {
    match object.get("status") {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(status)) if status == "completed" => Ok(()),
        Some(_) => Err(error),
    }
}

fn push_opaque(item: Value, decoded: &mut ResponseBuilder) -> Result<(), ResponsesResponseError> {
    if decoded.opaque_items.len() >= MAX_OPAQUE_ITEMS {
        return Err(ResponsesResponseError::TooManyOpaqueItems);
    }
    if !json::fits(&item, MAX_OPAQUE_ITEM_BYTES) {
        return Err(ResponsesResponseError::OpaqueItemTooLarge);
    }
    decoded.opaque_items.push(item);
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

#[derive(Default)]
struct ResponseBuilder {
    assistant_text: String,
    tool_call: Option<ToolCall>,
    opaque_items: Vec<Value>,
    content_items: usize,
}

#[derive(Deserialize)]
struct WireResponse {
    #[serde(default)]
    status: Option<String>,
    output: Vec<Value>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Default, Deserialize)]
struct WireUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}
