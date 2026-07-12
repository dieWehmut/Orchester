use orchester_modell::{
    ActionDecoder, DecodeError, LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest,
    ModelResponse, ModelRole, RetryAfter, ScriptedLlm, ToolCall, ToolDefinition, ToolKind,
    MAX_ARGUMENTS_JSON_BYTES, MAX_COMMAND_PART_BYTES, MAX_CONTENT_BYTES, MAX_LIST_DEPTH,
    MAX_LIST_ITEMS, MAX_PATH_BYTES, MAX_QUERY_BYTES, MAX_RECALL_LIMIT,
};
use orchester_protokoll::{AgentAction, CallId, MemoryKind};
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn scripted_llm_returns_one_queued_tool_response_and_counts_one_call() {
    let expected = ModelResponse::tool("c1", "read_file", r#"{"path":"src/lib.rs"}"#);
    let llm = ScriptedLlm::new([Ok(expected.clone())]);

    let actual = llm
        .complete(ModelRequest::test(), CancellationToken::new())
        .await
        .expect("scripted response");

    assert_eq!(actual, expected);
    assert_eq!(actual.tool_call(), expected.tool_call.as_ref());
    assert_eq!(llm.call_count(), 1);
}

#[tokio::test]
async fn cancellation_before_pop_returns_cancelled_without_consuming_script() {
    let expected = ModelResponse::tool("c1", "finish", r#"{"summary":"done"}"#);
    let llm = ScriptedLlm::new([Ok(expected.clone())]);
    let cancelled = CancellationToken::new();
    cancelled.cancel();

    assert_eq!(
        llm.complete(ModelRequest::test(), cancelled)
            .await
            .expect_err("pre-cancelled request"),
        ModelError::Cancelled
    );
    assert_eq!(llm.call_count(), 0);
    assert!(llm.request_summaries().is_empty());

    let actual = llm
        .complete(ModelRequest::test(), CancellationToken::new())
        .await
        .expect("queued response remains");
    assert_eq!(actual, expected);
    assert_eq!(llm.call_count(), 1);
}

#[tokio::test]
async fn script_exhaustion_is_reported_after_counting_the_attempt() {
    let llm = ScriptedLlm::new(Vec::<Result<ModelResponse, ModelError>>::new());

    assert_eq!(
        llm.complete(ModelRequest::test(), CancellationToken::new())
            .await
            .expect_err("empty script"),
        ModelError::ScriptExhausted
    );
    assert_eq!(llm.call_count(), 1);
    assert_eq!(llm.request_summaries().len(), 1);
}

#[tokio::test]
async fn request_summary_contains_only_counts_names_and_flags() {
    let request = ModelRequest {
        model: "safe-model".into(),
        messages: vec![
            ModelMessage {
                role: ModelRole::System,
                items: vec![
                    ModelItem::Text("TOP_SECRET_MESSAGE".into()),
                    ModelItem::Opaque(json!({"credential":"OPAQUE_SECRET"})),
                ],
            },
            ModelMessage {
                role: ModelRole::Tool,
                items: vec![
                    ModelItem::ToolCall(ToolCall::new(
                        "call-secret",
                        "read_file",
                        r#"{"path":"ARGUMENT_SECRET"}"#,
                    )),
                    ModelItem::ToolResult {
                        call_id: "call-secret".into(),
                        output: "TOOL_OUTPUT_SECRET".into(),
                    },
                ],
            },
        ],
        tools: vec![ToolDefinition {
            name: "read_file".into(),
            description: "DESCRIPTION_SECRET".into(),
            parameters: json!({"password":"PARAMETER_SECRET"}),
        }],
        store: true,
    };
    let llm = ScriptedLlm::new([Ok(ModelResponse {
        assistant_text: "RESPONSE_SECRET".into(),
        tool_call: None,
        usage: Default::default(),
        opaque_items: vec![json!({"response":"OPAQUE_RESPONSE_SECRET"})],
    })]);

    llm.complete(request, CancellationToken::new())
        .await
        .expect("scripted response");

    let summaries = llm.request_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].message_count, 2);
    assert_eq!(summaries[0].item_count, 4);
    assert_eq!(summaries[0].tool_definition_count, 1);
    assert!(summaries[0].store);

    let rendered = format!("{:?}", summaries[0]);
    for secret in [
        "TOP_SECRET_MESSAGE",
        "OPAQUE_SECRET",
        "ARGUMENT_SECRET",
        "TOOL_OUTPUT_SECRET",
        "DESCRIPTION_SECRET",
        "PARAMETER_SECRET",
        "RESPONSE_SECRET",
        "OPAQUE_RESPONSE_SECRET",
    ] {
        assert!(!rendered.contains(secret), "summary leaked {secret}");
    }
}

#[test]
fn malformed_and_unknown_tools_are_distinguished_without_echoing_arguments() {
    let malformed = ToolCall::new("call-1", "read_file", "{not-json");
    assert!(matches!(
        ActionDecoder.decode(&malformed),
        Err(DecodeError::InvalidArguments { call_id, tool })
            if call_id == CallId::from("call-1") && tool == ToolKind::ReadFile
    ));

    let unknown = ToolCall::new("call-2", "secret_tool", r#"{"credential":"DO_NOT_ECHO"}"#);
    let error = ActionDecoder.decode(&unknown).expect_err("unknown tool");
    assert!(matches!(
        &error,
        DecodeError::UnknownTool { call_id }
            if call_id == &CallId::from("call-2")
    ));
    assert!(!error.to_string().contains("DO_NOT_ECHO"));
}

#[test]
fn read_file_rejects_unknown_argument_fields() {
    let call = ToolCall::new(
        "c1",
        "read_file",
        r#"{"path":"a","start_line":null,"end_line":null,"surprise":true}"#,
    );
    assert!(matches!(
        ActionDecoder.decode(&call),
        Err(DecodeError::InvalidArguments { .. })
    ));
}

#[test]
fn every_protocol_action_has_a_strict_decoder_mapping() {
    let cases = [
        (
            "list_files",
            r#"{"path":"src","depth":2}"#,
            AgentAction::ListFiles {
                path: "src".into(),
                depth: 2,
            },
        ),
        (
            "search_text",
            r#"{"path":"src","query":"needle"}"#,
            AgentAction::SearchText {
                path: "src".into(),
                query: "needle".into(),
            },
        ),
        (
            "read_file",
            r#"{"path":"a","start_line":1,"end_line":3}"#,
            AgentAction::ReadFile {
                path: "a".into(),
                start_line: Some(1),
                end_line: Some(3),
            },
        ),
        (
            "write_file",
            r#"{"path":"a","content":"hello"}"#,
            AgentAction::WriteFile {
                path: "a".into(),
                content: "hello".into(),
            },
        ),
        (
            "apply_patch",
            r#"{"patch":"*** Begin Patch"}"#,
            AgentAction::ApplyPatch {
                patch: "*** Begin Patch".into(),
            },
        ),
        (
            "run_command",
            r#"{"program":"cargo","args":["test","-p","x"],"cwd":"repo"}"#,
            AgentAction::RunCommand {
                program: "cargo".into(),
                args: vec!["test".into(), "-p".into(), "x".into()],
                cwd: Some("repo".into()),
            },
        ),
        (
            "run_checks",
            r#"{"ids":["fmt","clippy"]}"#,
            AgentAction::RunChecks {
                ids: vec!["fmt".into(), "clippy".into()],
            },
        ),
        (
            "remember",
            r#"{"kind":"architecture_decision","content":"use typed DTOs"}"#,
            AgentAction::Remember {
                kind: MemoryKind::ArchitectureDecision,
                content: "use typed DTOs".into(),
            },
        ),
        (
            "recall",
            r#"{"query":"DTO","limit":4}"#,
            AgentAction::Recall {
                query: "DTO".into(),
                limit: 4,
            },
        ),
        (
            "request_approval",
            r#"{"reason":"network access"}"#,
            AgentAction::RequestApproval {
                reason: "network access".into(),
            },
        ),
        (
            "finish",
            r#"{"summary":"complete"}"#,
            AgentAction::Finish {
                summary: "complete".into(),
            },
        ),
    ];

    for (tool, arguments, expected) in cases {
        let call = ToolCall::new(format!("call-{tool}"), tool, arguments);
        assert_eq!(ActionDecoder.decode(&call).expect(tool), expected);
    }
}

#[test]
fn empty_command_program_is_rejected() {
    for program in ["", "   "] {
        let call = ToolCall::new(
            "c1",
            "run_command",
            json!({"program": program, "args": []}).to_string(),
        );
        assert!(matches!(
            ActionDecoder.decode(&call),
            Err(DecodeError::EmptyProgram { .. })
        ));
    }
}

#[test]
fn line_ranges_must_be_positive_and_ordered() {
    for arguments in [
        json!({"path":"a","start_line":0,"end_line":1}),
        json!({"path":"a","start_line":2,"end_line":1}),
        json!({"path":"a","start_line":1,"end_line":0}),
    ] {
        let call = ToolCall::new("c1", "read_file", arguments.to_string());
        assert!(matches!(
            ActionDecoder.decode(&call),
            Err(DecodeError::InvalidRange { .. })
        ));
    }
}

#[test]
fn bounded_depth_and_limit_reject_zero_and_oversized_values() {
    for depth in [0, MAX_LIST_DEPTH + 1] {
        let call = ToolCall::new(
            "c1",
            "list_files",
            json!({"path":".","depth":depth}).to_string(),
        );
        assert!(matches!(
            ActionDecoder.decode(&call),
            Err(DecodeError::InvalidDepth { .. })
        ));
    }
    for limit in [0, MAX_RECALL_LIMIT + 1] {
        let call = ToolCall::new(
            "c2",
            "recall",
            json!({"query":"q","limit":limit}).to_string(),
        );
        assert!(matches!(
            ActionDecoder.decode(&call),
            Err(DecodeError::InvalidLimit { .. })
        ));
    }
}

#[test]
fn json_strings_are_never_coerced_into_numbers_or_arrays() {
    let calls = [
        ToolCall::new("c1", "list_files", r#"{"path":".","depth":"2"}"#),
        ToolCall::new(
            "c2",
            "read_file",
            r#"{"path":"a","start_line":"1","end_line":2}"#,
        ),
        ToolCall::new("c3", "recall", r#"{"query":"q","limit":"3"}"#),
        ToolCall::new("c4", "run_command", r#"{"program":"cargo","args":"test"}"#),
    ];

    for call in calls {
        assert!(matches!(
            ActionDecoder.decode(&call),
            Err(DecodeError::InvalidArguments { .. })
        ));
    }
}

#[test]
fn each_tool_args_struct_rejects_an_unknown_field() {
    let calls = [
        ("list_files", r#"{"path":".","depth":1,"x":true}"#),
        ("search_text", r#"{"path":".","query":"q","x":true}"#),
        (
            "read_file",
            r#"{"path":"a","start_line":null,"end_line":null,"x":true}"#,
        ),
        ("write_file", r#"{"path":"a","content":"x","x":true}"#),
        ("apply_patch", r#"{"patch":"x","x":true}"#),
        (
            "run_command",
            r#"{"program":"p","args":[],"cwd":null,"x":true}"#,
        ),
        ("run_checks", r#"{"ids":[],"x":true}"#),
        ("remember", r#"{"kind":"lesson","content":"x","x":true}"#),
        ("recall", r#"{"query":"q","limit":1,"x":true}"#),
        ("request_approval", r#"{"reason":"r","x":true}"#),
        ("finish", r#"{"summary":"s","x":true}"#),
    ];

    for (tool, arguments) in calls {
        let call = ToolCall::new("c1", tool, arguments);
        assert!(
            matches!(
                ActionDecoder.decode(&call),
                Err(DecodeError::InvalidArguments { .. })
            ),
            "tool {tool} accepted an unknown field"
        );
    }
}

#[test]
fn call_ids_remain_typed_across_model_and_decoder_boundaries() {
    let expected = CallId::from("typed-call");
    let call = ToolCall::new(expected.clone(), "read_file", r#"{"path":"a"}"#);
    let actual: CallId = call.call_id.clone();
    assert_eq!(actual, expected);

    let malformed = ToolCall::new(expected.clone(), "read_file", "{");
    let error = ActionDecoder
        .decode(&malformed)
        .expect_err("malformed JSON");
    assert!(matches!(
        error,
        DecodeError::InvalidArguments { call_id, .. } if call_id == expected
    ));

    let result = ModelItem::ToolResult {
        call_id: CallId::from("typed-result"),
        output: String::new(),
    };
    assert!(matches!(
        result,
        ModelItem::ToolResult { call_id, .. } if call_id == CallId::from("typed-result")
    ));
}

#[test]
fn raw_argument_json_is_bounded_before_deserialization() {
    let oversized = "x".repeat(MAX_ARGUMENTS_JSON_BYTES + 1);
    let call = ToolCall::new(
        "bounded-raw",
        "write_file",
        format!(r#"{{"path":"a","content":"{oversized}"}}"#),
    );

    assert!(matches!(
        ActionDecoder.decode(&call),
        Err(DecodeError::ArgumentsTooLarge { actual_bytes, max_bytes, .. })
            if actual_bytes > max_bytes && max_bytes == MAX_ARGUMENTS_JSON_BYTES
    ));
}

#[test]
fn every_durable_string_field_is_bounded() {
    let cases = [
        (
            "list_files",
            json!({"path":"x".repeat(MAX_PATH_BYTES + 1),"depth":1}),
        ),
        (
            "search_text",
            json!({"path":".","query":"x".repeat(MAX_QUERY_BYTES + 1)}),
        ),
        (
            "write_file",
            json!({"path":"a","content":"x".repeat(MAX_CONTENT_BYTES + 1)}),
        ),
        (
            "run_command",
            json!({"program":"x".repeat(MAX_COMMAND_PART_BYTES + 1),"args":[]}),
        ),
        (
            "remember",
            json!({"kind":"lesson","content":"x".repeat(MAX_CONTENT_BYTES + 1)}),
        ),
    ];

    for (tool, arguments) in cases {
        let call = ToolCall::new("bounded-field", tool, arguments.to_string());
        assert!(
            matches!(
                ActionDecoder.decode(&call),
                Err(DecodeError::FieldTooLarge { actual_bytes, max_bytes, .. })
                    if actual_bytes > max_bytes
            ),
            "{tool} accepted an oversized field"
        );
    }
}

#[test]
fn command_arguments_and_check_ids_have_item_and_element_bounds() {
    let too_many_args = ToolCall::new(
        "many-args",
        "run_command",
        json!({"program":"cargo","args":vec!["x"; MAX_LIST_ITEMS + 1]}).to_string(),
    );
    assert!(matches!(
        ActionDecoder.decode(&too_many_args),
        Err(DecodeError::TooManyItems { actual, max, .. })
            if actual == MAX_LIST_ITEMS + 1 && max == MAX_LIST_ITEMS
    ));

    let oversized_arg = ToolCall::new(
        "large-arg",
        "run_command",
        json!({"program":"cargo","args":["x".repeat(MAX_COMMAND_PART_BYTES + 1)]}).to_string(),
    );
    assert!(matches!(
        ActionDecoder.decode(&oversized_arg),
        Err(DecodeError::FieldTooLarge { .. })
    ));

    let too_many_ids = ToolCall::new(
        "many-checks",
        "run_checks",
        json!({"ids":vec!["fmt"; MAX_LIST_ITEMS + 1]}).to_string(),
    );
    assert!(matches!(
        ActionDecoder.decode(&too_many_ids),
        Err(DecodeError::TooManyItems { .. })
    ));
}

#[tokio::test]
async fn retained_and_rendered_diagnostics_drop_arbitrary_model_strings() {
    let secret = "MODEL_SECRET\n\u{1b}[31mCONTROL";
    let request = ModelRequest {
        model: secret.into(),
        messages: vec![ModelMessage {
            role: ModelRole::Assistant,
            items: vec![ModelItem::ToolCall(ToolCall::new(
                "call-secret\r\n",
                secret,
                r#"{"api_key":"sk-private"}"#,
            ))],
        }],
        tools: vec![ToolDefinition {
            name: secret.into(),
            description: secret.into(),
            parameters: json!({"secret":secret}),
        }],
        store: false,
    };
    let request_debug = format!("{request:?}");
    assert!(!request_debug.contains("MODEL_SECRET"));
    assert!(!request_debug.contains('\u{1b}'));
    let llm = ScriptedLlm::new([Ok(ModelResponse {
        assistant_text: String::new(),
        tool_call: None,
        usage: Default::default(),
        opaque_items: Vec::new(),
    })]);

    llm.complete(request, CancellationToken::new())
        .await
        .expect("scripted response");
    let rendered = format!("{:?}", llm.request_summaries());
    assert!(!rendered.contains("MODEL_SECRET"));
    assert!(!rendered.contains('\u{1b}'));

    let call = ToolCall::new("call-secret\r\n", secret, r#"{"token":"sk-private"}"#);
    let call_debug = format!("{call:?}");
    assert!(!call_debug.contains("MODEL_SECRET"));
    assert!(!call_debug.contains("sk-private"));
    assert!(!call_debug.contains('\u{1b}'));
    let error = ActionDecoder.decode(&call).expect_err("unknown tool");
    for diagnostic in [format!("{error}"), format!("{error:?}")] {
        assert!(!diagnostic.contains("MODEL_SECRET"));
        assert!(!diagnostic.contains("call-secret"));
        assert!(!diagnostic.contains("sk-private"));
        assert!(!diagnostic.contains('\u{1b}'));
        assert!(!diagnostic.contains('\r'));
        assert!(!diagnostic.contains('\n'));
    }
}

#[test]
fn model_errors_expose_bounded_retry_metadata_without_provider_text() {
    let authentication = ModelError::Authentication;
    assert!(!authentication.retry_metadata().retryable());
    assert_eq!(authentication.retry_metadata().retry_after(), None);

    let requested = std::time::Duration::from_secs(86_400);
    let rate_limited = ModelError::rate_limited(Some(requested));
    assert!(matches!(&rate_limited, ModelError::RateLimited { .. }));
    let retry = rate_limited.retry_metadata();
    assert!(retry.retryable());
    assert_eq!(retry.retry_after(), Some(RetryAfter::MAX.as_duration()));

    for error in [
        authentication,
        rate_limited,
        ModelError::Transport,
        ModelError::Protocol,
    ] {
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("authorization"));
        assert!(!rendered.contains('\u{1b}'));
    }
}
