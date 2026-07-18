use orchester_laufzeit::harness::provider::responses::{
    ResponsesRequestError, ResponsesRequestOptions, encode_responses_request,
};
use orchester_modell::{
    ModelItem, ModelMessage, ModelRequest, ModelRole, ToolCall, ToolDefinition,
};
use orchester_protokoll::CallId;
use serde_json::{Value, json};

const PROMPT_CANARY: &str = "prompt-canary-not-in-errors";

fn request(messages: Vec<ModelMessage>, tools: Vec<ToolDefinition>) -> ModelRequest {
    ModelRequest {
        model: "gpt-test".into(),
        messages,
        tools,
        store: false,
    }
}

fn text(role: ModelRole, value: &str) -> ModelMessage {
    ModelMessage {
        role,
        items: vec![ModelItem::Text(value.into())],
    }
}

fn options() -> ResponsesRequestOptions {
    ResponsesRequestOptions {
        reasoning_effort: Some("ultra".into()),
        service_tier: Some("default".into()),
    }
}

#[test]
fn encodes_instructions_messages_and_safe_request_defaults() {
    let body = encode_responses_request(
        &request(
            vec![
                text(ModelRole::System, "governed system"),
                text(ModelRole::User, PROMPT_CANARY),
            ],
            Vec::new(),
        ),
        &options(),
    )
    .expect("request should encode");
    let value: Value = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(value["model"], "gpt-test");
    assert_eq!(value["instructions"], "governed system");
    assert_eq!(value["input"][0]["role"], "user");
    assert_eq!(value["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(value["input"][0]["content"][0]["text"], PROMPT_CANARY);
    assert_eq!(value["reasoning"]["effort"], "ultra");
    assert_eq!(value["service_tier"], "default");
    assert_eq!(value["store"], false);
    assert_eq!(value["stream"], false);
    assert_eq!(value["parallel_tool_calls"], false);
    assert_eq!(value.get("tools"), None);
}

#[test]
fn encodes_function_tools_and_sequential_call_history() {
    let tool = ToolDefinition {
        name: "read_file".into(),
        description: "Read a workspace file".into(),
        parameters: json!({
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
            "additionalProperties": false
        }),
    };
    let call_id = CallId::from("call-1");
    let body = encode_responses_request(
        &request(
            vec![
                text(ModelRole::User, "inspect"),
                ModelMessage {
                    role: ModelRole::Assistant,
                    items: vec![ModelItem::ToolCall(ToolCall::new(
                        call_id.clone(),
                        "read_file",
                        r#"{"path":"src/lib.rs"}"#,
                    ))],
                },
                ModelMessage {
                    role: ModelRole::Tool,
                    items: vec![ModelItem::ToolResult {
                        call_id,
                        output: "file contents".into(),
                    }],
                },
            ],
            vec![tool],
        ),
        &ResponsesRequestOptions::default(),
    )
    .expect("request should encode");
    let value: Value = serde_json::from_slice(&body).expect("valid JSON");

    assert_eq!(value["tools"][0]["type"], "function");
    assert_eq!(value["tools"][0]["name"], "read_file");
    assert_eq!(value["tools"][0]["strict"], true);
    assert_eq!(value["input"][1]["type"], "function_call");
    assert_eq!(value["input"][1]["call_id"], "call-1");
    assert_eq!(value["input"][1]["arguments"], r#"{"path":"src/lib.rs"}"#);
    assert_eq!(value["input"][2]["type"], "function_call_output");
    assert_eq!(value["input"][2]["call_id"], "call-1");
    assert_eq!(value["input"][2]["output"], "file contents");
    assert_eq!(value["parallel_tool_calls"], false);
}

#[test]
fn rejects_opaque_items_invalid_tool_schemas_and_invalid_history_roles() {
    let opaque = request(
        vec![ModelMessage {
            role: ModelRole::User,
            items: vec![ModelItem::Opaque(json!({"secret": "opaque"}))],
        }],
        Vec::new(),
    );
    assert!(encode_responses_request(&opaque, &ResponsesRequestOptions::default()).is_err());

    let invalid_schema = request(
        vec![text(ModelRole::User, "prompt")],
        vec![ToolDefinition {
            name: "bad".into(),
            description: "bad".into(),
            parameters: json!("not-an-object"),
        }],
    );
    assert!(
        encode_responses_request(&invalid_schema, &ResponsesRequestOptions::default()).is_err()
    );

    let wrong_role = request(
        vec![ModelMessage {
            role: ModelRole::User,
            items: vec![ModelItem::ToolResult {
                call_id: CallId::from("call-2"),
                output: "output".into(),
            }],
        }],
        Vec::new(),
    );
    assert!(encode_responses_request(&wrong_role, &ResponsesRequestOptions::default()).is_err());

    let invalid_arguments = request(
        vec![ModelMessage {
            role: ModelRole::Assistant,
            items: vec![ModelItem::ToolCall(ToolCall::new(
                CallId::from("call-3"),
                "read_file",
                "[]",
            ))],
        }],
        Vec::new(),
    );
    assert!(matches!(
        encode_responses_request(&invalid_arguments, &ResponsesRequestOptions::default()),
        Err(ResponsesRequestError::InvalidToolArguments)
    ));

    let system_after_user = request(
        vec![
            text(ModelRole::User, "prompt"),
            text(ModelRole::System, "late policy"),
        ],
        Vec::new(),
    );
    assert!(
        encode_responses_request(&system_after_user, &ResponsesRequestOptions::default()).is_err()
    );
}

#[test]
fn encodes_assistant_text_and_rejects_an_oversized_wire_body() {
    let body = encode_responses_request(
        &request(
            vec![
                text(ModelRole::User, "prompt"),
                text(ModelRole::Assistant, "answer"),
            ],
            Vec::new(),
        ),
        &ResponsesRequestOptions::default(),
    )
    .expect("request should encode");
    let value: Value = serde_json::from_slice(&body).expect("valid JSON");
    assert_eq!(value["input"][1]["role"], "assistant");
    assert_eq!(value["input"][1]["content"][0]["type"], "output_text");

    let large_messages = (0..10)
        .map(|_| text(ModelRole::User, &"x".repeat(512 * 1024)))
        .collect();
    assert!(matches!(
        encode_responses_request(
            &request(large_messages, Vec::new()),
            &ResponsesRequestOptions::default()
        ),
        Err(ResponsesRequestError::RequestTooLarge)
    ));
}

#[test]
fn encoding_errors_do_not_render_prompt_or_tool_content() {
    let request = request(
        vec![text(ModelRole::User, PROMPT_CANARY)],
        vec![ToolDefinition {
            name: "bad".into(),
            description: "bad".into(),
            parameters: Value::Null,
        }],
    );
    let error = encode_responses_request(&request, &ResponsesRequestOptions::default())
        .expect_err("invalid schema should fail");
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(PROMPT_CANARY));
    assert!(!rendered.contains("bad"));
}
