use orchester_laufzeit::harness::provider::MAX_HTTP_RESPONSE_BYTES;
use orchester_laufzeit::harness::provider::responses::{
    ResponsesResponseError, decode_responses_response,
};
use serde_json::{Value, json};

const RESPONSE_CANARY: &str = "response-canary-not-in-errors";

fn body(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("fixture should serialize")
}

#[test]
fn decodes_mixed_text_tool_usage_and_forward_compatible_items() {
    let response = decode_responses_response(&body(json!({
        "id": "resp-1",
        "status": "completed",
        "output": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "private reasoning summary"}]
            },
            {
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [
                    {"type": "output_text", "text": "Ready. "},
                    {"type": "output_text", "text": "Inspecting."}
                ]
            },
            {
                "type": "function_call",
                "status": "completed",
                "call_id": "call-1",
                "name": "read_file",
                "arguments": "{\"path\":\"src/lib.rs\"}"
            }
        ],
        "usage": {"input_tokens": 17, "output_tokens": 9, "total_tokens": 26}
    })))
    .expect("completed response should decode");

    assert_eq!(response.assistant_text, "Ready. Inspecting.");
    let call = response.tool_call().expect("one function call");
    assert_eq!(call.call_id.0, "call-1");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments_json, r#"{"path":"src/lib.rs"}"#);
    assert_eq!(response.usage.input_tokens, 17);
    assert_eq!(response.usage.output_tokens, 9);
    assert_eq!(response.opaque_items.len(), 1);
    assert_eq!(response.opaque_items[0]["type"], "reasoning");
}

#[test]
fn decodes_refusal_text_and_defaults_missing_usage() {
    let response = decode_responses_response(&body(json!({
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "refusal", "refusal": "Cannot comply."}]
        }]
    })))
    .expect("compatible response should decode");

    assert_eq!(response.assistant_text, "Cannot comply.");
    assert_eq!(response.tool_call(), None);
    assert_eq!(response.usage.input_tokens, 0);
    assert_eq!(response.usage.output_tokens, 0);
}

#[test]
fn preserves_bounded_unknown_message_content_without_rendering_it() {
    let response = decode_responses_response(&body(json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "future_content", "payload": RESPONSE_CANARY},
                {"type": "output_text", "text": "visible"}
            ]
        }]
    })))
    .expect("unknown bounded content should be preserved");

    assert_eq!(response.assistant_text, "visible");
    assert_eq!(response.opaque_items.len(), 1);
    assert_eq!(response.opaque_items[0]["payload"], RESPONSE_CANARY);
    assert!(!format!("{response:?}").contains(RESPONSE_CANARY));
}

#[test]
fn rejects_incomplete_ambiguous_and_invalid_tool_responses() {
    let incomplete = body(json!({"status": "incomplete", "output": []}));
    assert!(matches!(
        decode_responses_response(&incomplete),
        Err(ResponsesResponseError::InvalidStatus)
    ));

    let two_calls = body(json!({
        "status": "completed",
        "output": [
            {"type":"function_call", "call_id":"call-1", "name":"read_file", "arguments":"{}"},
            {"type":"function_call", "call_id":"call-2", "name":"read_file", "arguments":"{}"}
        ]
    }));
    assert!(matches!(
        decode_responses_response(&two_calls),
        Err(ResponsesResponseError::MultipleToolCalls)
    ));

    let invalid_arguments = body(json!({
        "output": [{
            "type":"function_call",
            "call_id":"call-1",
            "name":"read_file",
            "arguments":"[]"
        }]
    }));
    assert!(matches!(
        decode_responses_response(&invalid_arguments),
        Err(ResponsesResponseError::InvalidToolCall)
    ));

    let wrong_role = body(json!({
        "output": [{
            "type":"message",
            "role":"user",
            "content":[{"type":"output_text", "text":"bad"}]
        }]
    }));
    assert!(matches!(
        decode_responses_response(&wrong_role),
        Err(ResponsesResponseError::InvalidMessage)
    ));
}

#[test]
fn response_errors_are_bounded_and_do_not_echo_provider_content() {
    let invalid = body(json!({
        "output": [{
            "type":"function_call",
            "call_id":"call-1",
            "name": RESPONSE_CANARY,
            "arguments":"not-json"
        }]
    }));
    let error = decode_responses_response(&invalid).expect_err("invalid call should fail");
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(RESPONSE_CANARY));
    assert!(!rendered.contains("not-json"));

    let oversized = vec![b'x'; MAX_HTTP_RESPONSE_BYTES + 1];
    assert!(matches!(
        decode_responses_response(&oversized),
        Err(ResponsesResponseError::ResponseTooLarge)
    ));

    let oversized_text = body(json!({
        "output": [{
            "type":"message",
            "role":"assistant",
            "content":[{"type":"output_text", "text":"x".repeat(512 * 1024 + 1)}]
        }]
    }));
    assert!(matches!(
        decode_responses_response(&oversized_text),
        Err(ResponsesResponseError::ContentTooLarge)
    ));
}
