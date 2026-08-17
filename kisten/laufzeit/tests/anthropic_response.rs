use orchester_laufzeit::harness::provider::anthropic::{
    decode_anthropic_response, AnthropicResponseError,
};
use serde_json::{json, Value};

const RESPONSE_CANARY: &str = "response-canary-not-in-errors";

fn body(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("fixture should serialize")
}

#[test]
fn decodes_text_a_tool_call_and_forward_compatible_blocks() {
    let response = decode_anthropic_response(&body(json!({
        "id": "msg-1",
        "type": "message",
        "role": "assistant",
        "content": [
            {"type": "thinking", "thinking": "private", "signature": "sig"},
            {"type": "text", "text": "Ready. "},
            {"type": "text", "text": "Inspecting."},
            {
                "type": "tool_use",
                "id": "toolu-1",
                "name": "read_file",
                "input": {"path": "src/lib.rs"}
            }
        ],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 17, "output_tokens": 9}
    })))
    .expect("a complete message should decode");

    assert_eq!(response.assistant_text, "Ready. Inspecting.");
    let call = response.tool_call().expect("one tool call");
    assert_eq!(call.call_id.0, "toolu-1");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments_json, r#"{"path":"src/lib.rs"}"#);
    assert_eq!(response.usage.input_tokens, 17);
    assert_eq!(response.usage.output_tokens, 9);
    // An unknown block is preserved verbatim rather than dropped or trusted.
    assert_eq!(response.opaque_items.len(), 1);
    assert_eq!(response.opaque_items[0]["type"], "thinking");
}

#[test]
fn defaults_missing_usage_to_zero() {
    let response = decode_anthropic_response(&body(json!({
        "content": [{"type": "text", "text": "done"}]
    })))
    .expect("a message without usage should decode");

    assert_eq!(response.assistant_text, "done");
    assert_eq!(response.usage.input_tokens, 0);
    assert_eq!(response.usage.output_tokens, 0);
    assert!(response.tool_call().is_none());
}

#[test]
fn rejects_malformed_bodies_without_echoing_provider_text() {
    let cases: Vec<(Vec<u8>, AnthropicResponseError)> = vec![
        (
            format!("{{ not json {RESPONSE_CANARY}").into_bytes(),
            AnthropicResponseError::InvalidJson,
        ),
        (
            body(json!({"type": "error", "content": []})),
            AnthropicResponseError::InvalidResponse,
        ),
        (
            body(json!({"error": {"message": RESPONSE_CANARY}})),
            AnthropicResponseError::InvalidResponse,
        ),
        (
            body(json!({"content": []})),
            AnthropicResponseError::EmptyContent,
        ),
        (
            body(json!({"content": [{"text": RESPONSE_CANARY}]})),
            AnthropicResponseError::InvalidContentBlock,
        ),
        (
            body(json!({"content": [{"type": "text"}]})),
            AnthropicResponseError::InvalidContentBlock,
        ),
        (
            body(json!({
                "content": [{"type": "tool_use", "id": "toolu-1", "name": "read_file"}]
            })),
            AnthropicResponseError::InvalidToolCall,
        ),
        (
            body(json!({
                "content": [
                    {"type": "tool_use", "id": "a", "name": "read_file", "input": {}},
                    {"type": "tool_use", "id": "b", "name": "read_file", "input": {}}
                ]
            })),
            AnthropicResponseError::MultipleToolCalls,
        ),
    ];

    for (body, expected) in cases {
        let error = decode_anthropic_response(&body).expect_err("body must be rejected");
        assert_eq!(error, expected);
        let rendered = format!("{error} {error:?}");
        assert!(!rendered.contains(RESPONSE_CANARY));
    }
}
