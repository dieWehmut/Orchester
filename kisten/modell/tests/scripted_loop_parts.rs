use orchester_modell::{
    ActionDecoder, DecodeError, LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest,
    ModelResponse, ModelRole, ScriptedLlm, ToolCall, ToolDefinition, MAX_LIST_DEPTH,
    MAX_RECALL_LIMIT,
};
use orchester_protokoll::{AgentAction, MemoryKind};
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
    assert_eq!(summaries[0].model, "safe-model");
    assert_eq!(summaries[0].message_count, 2);
    assert_eq!(summaries[0].item_count, 4);
    assert_eq!(summaries[0].tool_definition_names, vec!["read_file"]);
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
            if call_id == "call-1" && tool == "read_file"
    ));

    let unknown = ToolCall::new("call-2", "secret_tool", r#"{"credential":"DO_NOT_ECHO"}"#);
    let error = ActionDecoder.decode(&unknown).expect_err("unknown tool");
    assert!(matches!(
        &error,
        DecodeError::UnknownTool { call_id, tool }
            if call_id == "call-2" && tool == "secret_tool"
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
