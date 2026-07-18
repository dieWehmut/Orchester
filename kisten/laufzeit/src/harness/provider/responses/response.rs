use orchester_modell::{
    MAX_ARGUMENTS_JSON_BYTES, MAX_CALL_ID_BYTES, MAX_CONTENT_BYTES, ModelResponse, ModelUsage,
    ToolCall,
};
use orchester_protokoll::CallId;
use serde::Deserialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::harness::provider::MAX_HTTP_RESPONSE_BYTES;

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
