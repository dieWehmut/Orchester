use orchester_laufzeit::harness::provider::anthropic::{
    encode_anthropic_request, encode_anthropic_stream_request, AnthropicRequestError,
    AnthropicRequestOptions, DEFAULT_MAX_OUTPUT_TOKENS,
};
use orchester_modell::{
    ModelItem, ModelMessage, ModelRequest, ModelRole, ToolCall, ToolDefinition,
};
use orchester_protokoll::CallId;
use serde_json::{json, Value};

const PROMPT_CANARY: &str = "prompt-canary-not-in-errors";

fn request(messages: Vec<ModelMessage>, tools: Vec<ToolDefinition>) -> ModelRequest {
    ModelRequest {
        model: "claude-test".into(),
        messages,
        tools,
        store: false,
    }
}

fn message(role: ModelRole, items: Vec<ModelItem>) -> ModelMessage {
    ModelMessage { role, items }
}

fn text(role: ModelRole, body: &str) -> ModelMessage {
    message(role, vec![ModelItem::Text(body.to_owned())])
}

fn encoded(request: &ModelRequest) -> Value {
    let body = encode_anthropic_request(request, &AnthropicRequestOptions::default())
        .expect("request should encode");
    serde_json::from_slice(&body).expect("encoded body should be JSON")
}

#[test]
fn folds_system_turns_into_the_system_field_and_keeps_roles_alternating() {
    let body = encoded(&request(
        vec![
            text(ModelRole::System, "you are precise"),
            text(ModelRole::System, "you cite files"),
            text(ModelRole::User, "read src/lib.rs"),
            message(
                ModelRole::Assistant,
                vec![
                    ModelItem::Text("reading".into()),
                    ModelItem::ToolCall(ToolCall::new(
                        CallId::from("toolu-1"),
                        "read_file",
                        r#"{"path":"src/lib.rs"}"#,
                    )),
                ],
            ),
            message(
                ModelRole::Tool,
                vec![ModelItem::ToolResult {
                    call_id: CallId::from("toolu-1"),
                    output: "pub fn main() {}".into(),
                }],
            ),
            text(ModelRole::User, "summarize it"),
        ],
        Vec::new(),
    ));

    assert_eq!(body["model"], "claude-test");
    assert_eq!(body["max_tokens"], DEFAULT_MAX_OUTPUT_TOKENS);
    assert_eq!(body["system"], "you are precise\n\nyou cite files");
    assert_eq!(body["stream"], false);
    assert!(body.get("tools").is_none());
    assert!(body.get("tool_choice").is_none());

    let messages = body["messages"].as_array().expect("messages array");
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"][0]["text"], "read src/lib.rs");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"][1]["type"], "tool_use");
    assert_eq!(messages[1]["content"][1]["id"], "toolu-1");
    assert_eq!(messages[1]["content"][1]["input"]["path"], "src/lib.rs");
    // A tool result is a user turn, so it must merge with the prompt that
    // follows it rather than becoming a second consecutive user message.
    assert_eq!(messages[2]["role"], "user");
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    assert_eq!(messages[2]["content"][0]["tool_use_id"], "toolu-1");
    assert_eq!(messages[2]["content"][1]["text"], "summarize it");
}

#[test]
fn declares_tools_with_a_tool_choice_and_disables_parallel_calls() {
    let body = encoded(&request(
        vec![text(ModelRole::User, "list the files")],
        vec![ToolDefinition {
            name: "list_files".into(),
            description: "list workspace files".into(),
            parameters: json!({"type": "object", "properties": {}}),
        }],
    ));

    assert_eq!(body["tools"][0]["name"], "list_files");
    assert_eq!(body["tools"][0]["description"], "list workspace files");
    assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    assert_eq!(body["tool_choice"]["type"], "auto");
    assert_eq!(body["tool_choice"]["disable_parallel_tool_use"], true);
}

#[test]
fn a_stream_request_only_differs_by_the_stream_flag() {
    let request = request(vec![text(ModelRole::User, "hello")], Vec::new());
    let options = AnthropicRequestOptions::default();
    let mut unary: Value = serde_json::from_slice(
        &encode_anthropic_request(&request, &options).expect("unary should encode"),
    )
    .expect("unary JSON");
    let streamed: Value = serde_json::from_slice(
        &encode_anthropic_stream_request(&request, &options).expect("stream should encode"),
    )
    .expect("stream JSON");

    assert_eq!(unary["stream"], false);
    assert_eq!(streamed["stream"], true);
    unary["stream"] = Value::Bool(true);
    assert_eq!(unary, streamed);
}

#[test]
fn carries_a_configured_service_tier_and_output_budget() {
    let body = encode_anthropic_request(
        &request(vec![text(ModelRole::User, "hello")], Vec::new()),
        &AnthropicRequestOptions {
            max_output_tokens: 4_096,
            service_tier: Some("standard_only".into()),
        },
    )
    .expect("request should encode");
    let body: Value = serde_json::from_slice(&body).expect("encoded body should be JSON");

    assert_eq!(body["max_tokens"], 4_096);
    assert_eq!(body["service_tier"], "standard_only");
}

#[test]
fn rejects_invalid_requests_without_echoing_the_prompt() {
    let cases = [
        (
            request(Vec::new(), Vec::new()),
            AnthropicRequestError::EmptyMessages,
        ),
        (
            request(vec![message(ModelRole::User, Vec::new())], Vec::new()),
            AnthropicRequestError::InvalidMessage,
        ),
        (
            request(
                vec![
                    text(ModelRole::User, PROMPT_CANARY),
                    text(ModelRole::System, PROMPT_CANARY),
                ],
                Vec::new(),
            ),
            AnthropicRequestError::InvalidMessage,
        ),
        (
            request(
                vec![message(
                    ModelRole::User,
                    vec![ModelItem::Opaque(json!({"type": "thinking"}))],
                )],
                Vec::new(),
            ),
            AnthropicRequestError::OpaqueItem,
        ),
        (
            request(
                vec![message(
                    ModelRole::Assistant,
                    vec![ModelItem::ToolCall(ToolCall::new(
                        CallId::from("toolu-1"),
                        "read_file",
                        "[]",
                    ))],
                )],
                Vec::new(),
            ),
            AnthropicRequestError::InvalidToolArguments,
        ),
        (
            request(
                vec![text(ModelRole::User, PROMPT_CANARY)],
                vec![ToolDefinition {
                    name: "not a token".into(),
                    description: String::new(),
                    parameters: json!({"type": "object"}),
                }],
            ),
            AnthropicRequestError::InvalidTool,
        ),
    ];

    for (request, expected) in cases {
        let error = encode_anthropic_request(&request, &AnthropicRequestOptions::default())
            .expect_err("invalid request must be rejected");
        assert_eq!(error, expected);
        let rendered = format!("{error} {error:?}");
        assert!(!rendered.contains(PROMPT_CANARY));
    }
}

#[test]
fn rejects_an_empty_output_budget() {
    let error = encode_anthropic_request(
        &request(vec![text(ModelRole::User, "hello")], Vec::new()),
        &AnthropicRequestOptions {
            max_output_tokens: 0,
            service_tier: None,
        },
    )
    .expect_err("a zero budget must be rejected");
    assert_eq!(error, AnthropicRequestError::InvalidOption);
}
