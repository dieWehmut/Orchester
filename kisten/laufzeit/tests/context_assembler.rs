use orchester_laufzeit::harness::context::{
    ContextAssembler, ContextError, ContextInput, ContextLimits, ContinuationInput, TranscriptEntry,
};
use orchester_laufzeit::harness::transcript::{
    TranscriptCodec, TranscriptError, TranscriptLimits, TranscriptRecord,
};
use orchester_modell::{ModelItem, ModelRole};
use secrecy::SecretString;
use serde_json::json;

fn input(history: Vec<TranscriptEntry>) -> ContextInput {
    ContextInput {
        model: "test-model".into(),
        prompt: "inspect the workspace".into(),
        history,
        store: false,
    }
}

#[test]
fn assembles_provider_neutral_request_with_all_strict_tools_and_store_false() {
    let assembler = ContextAssembler::new(ContextLimits::default(), Vec::new());
    let assembled = assembler.assemble(input(Vec::new())).unwrap();

    assert!(!assembled.request.store);
    assert_eq!(assembled.request.messages[0].role, ModelRole::System);
    assert_eq!(assembled.request.messages[1].role, ModelRole::User);
    let names = assembled
        .request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "list_files",
            "search_text",
            "read_file",
            "write_file",
            "apply_patch",
            "run_command",
            "run_checks",
            "remember",
            "recall",
            "request_approval",
            "finish",
        ]
    );
    assert!(assembled
        .request
        .tools
        .iter()
        .all(|tool| tool.parameters["additionalProperties"] == false));
}

#[test]
fn bounded_view_keeps_recent_history_without_mutating_canonical_entries() {
    let history = vec![
        TranscriptEntry::user("old ".repeat(80)),
        TranscriptEntry::assistant("middle ".repeat(80)),
        TranscriptEntry::tool_result("call-1", "recent observation"),
    ];
    let original = history.clone();
    let assembler = ContextAssembler::new(
        ContextLimits {
            max_bytes: 5_000,
            max_history_entries: 2,
        },
        Vec::new(),
    );
    let assembled = assembler.assemble(input(history.clone())).unwrap();

    assert_eq!(history, original);
    assert!(assembled.omitted_entries > 0);
    assert_eq!(assembled.omitted_prefix_hash.len(), 64);
    assert!(assembled.request.messages.iter().any(|message| {
        message.items.iter().any(|item| {
            matches!(item, ModelItem::ToolResult { output, .. } if output == "recent observation")
        })
    }));
}

#[test]
fn configured_and_key_shaped_secrets_are_rejected_without_echo() {
    let secret = SecretString::new("provider-secret-value".to_owned().into_boxed_str());
    let assembler = ContextAssembler::new(ContextLimits::default(), vec![secret]);
    let err = assembler
        .assemble(ContextInput {
            prompt: "use provider-secret-value".into(),
            ..input(Vec::new())
        })
        .unwrap_err();
    assert!(matches!(err, ContextError::SecretDetected));
    assert!(!err.to_string().contains("provider-secret-value"));

    let prefix_err = ContextAssembler::new(ContextLimits::default(), Vec::new())
        .assemble(ContextInput {
            prompt: "do not retain sk-example-sensitive-value".into(),
            ..input(Vec::new())
        })
        .unwrap_err();
    assert!(matches!(prefix_err, ContextError::SecretDetected));
}

#[test]
fn oversized_prompt_fails_before_a_model_request_exists() {
    let assembler = ContextAssembler::new(
        ContextLimits {
            max_bytes: 1_024,
            max_history_entries: 2,
        },
        Vec::new(),
    );
    let err = assembler
        .assemble(ContextInput {
            prompt: "x".repeat(4_096),
            ..input(Vec::new())
        })
        .unwrap_err();
    assert!(matches!(err, ContextError::BudgetExceeded));
}

#[test]
fn continuation_keeps_the_call_pair_without_repeating_the_user_prompt() {
    let assembler = ContextAssembler::new(ContextLimits::default(), Vec::new());
    let assembled = assembler
        .assemble_continuation(ContinuationInput {
            model: "test-model".into(),
            history: vec![
                TranscriptEntry::user("inspect the workspace"),
                TranscriptEntry::tool_call("call-1", "read_file", r#"{"path":"src/lib.rs"}"#),
                TranscriptEntry::tool_result("call-1", "bounded observation"),
            ],
            store: false,
        })
        .unwrap();

    assert_eq!(
        assembled
            .request
            .messages
            .iter()
            .map(|message| message.role)
            .collect::<Vec<_>>(),
        vec![
            ModelRole::System,
            ModelRole::User,
            ModelRole::Assistant,
            ModelRole::Tool,
        ]
    );
    assert!(matches!(
        &assembled.request.messages[2].items[0],
        ModelItem::ToolCall(call) if call.call_id.0 == "call-1"
    ));
    assert!(matches!(
        &assembled.request.messages[3].items[0],
        ModelItem::ToolResult { call_id, output }
            if call_id.0 == "call-1" && output == "bounded observation"
    ));
}

#[test]
fn continuation_rejects_unpaired_or_mismatched_tool_results() {
    let assembler = ContextAssembler::new(ContextLimits::default(), Vec::new());
    for history in [
        vec![TranscriptEntry::tool_result("call-1", "orphan")],
        vec![
            TranscriptEntry::tool_call("call-1", "read_file", r#"{"path":"a"}"#),
            TranscriptEntry::tool_result("call-2", "mismatch"),
        ],
    ] {
        let error = assembler
            .assemble_continuation(ContinuationInput {
                model: "test-model".into(),
                history,
                store: false,
            })
            .unwrap_err();
        assert!(matches!(error, ContextError::InvalidContinuation));
    }
}

#[test]
fn continuation_budget_never_splits_the_required_call_pair() {
    let assembler = ContextAssembler::new(
        ContextLimits {
            max_bytes: 20_000,
            max_history_entries: 2,
        },
        Vec::new(),
    );
    let error = assembler
        .assemble_continuation(ContinuationInput {
            model: "test-model".into(),
            history: vec![
                TranscriptEntry::tool_call("call-1", "read_file", r#"{"path":"a"}"#),
                TranscriptEntry::tool_result("call-1", "x".repeat(20_000)),
            ],
            store: false,
        })
        .unwrap_err();

    assert!(matches!(error, ContextError::BudgetExceeded));
}

#[test]
fn bounded_transcript_codec_round_trips_redacted_records_and_opaque_references() {
    let secret = "configured-transcript-secret";
    let codec = TranscriptCodec::new(
        TranscriptLimits {
            max_record_bytes: 4_096,
            max_text_bytes: 1_024,
            max_opaque_bytes: 2_048,
        },
        vec![SecretString::new(secret.to_owned().into_boxed_str())],
    );
    let records = vec![
        TranscriptRecord::system("system policy"),
        TranscriptRecord::user("inspect the workspace"),
        TranscriptRecord::assistant(format!("answer with {secret}")),
        TranscriptRecord::tool_call("call-1", "read_file", r#"{"path":"src/lib.rs"}"#),
        TranscriptRecord::tool_result("call-1", format!("Authorization: Bearer {secret}")),
        TranscriptRecord::opaque_json(&json!({"provider_item": "opaque"}), &codec).unwrap(),
    ];

    codec.validate_sequence(&records).unwrap();
    let encoded = codec.encode_all(&records).unwrap();
    let joined = encoded.join("\n");
    assert!(!joined.contains(secret));
    assert!(!joined.contains('\u{1b}'));

    let decoded = codec.decode_all(&encoded).unwrap();
    assert_eq!(decoded.len(), records.len());
    assert!(matches!(
        &decoded[2],
        TranscriptRecord::Assistant(text) if text.contains("[REDACTED]")
    ));
    assert!(matches!(decoded[5], TranscriptRecord::Opaque { .. }));
}

#[test]
fn transcript_codec_rejects_oversized_records_and_unpaired_tool_results() {
    let codec = TranscriptCodec::new(
        TranscriptLimits {
            max_record_bytes: 128,
            max_text_bytes: 64,
            max_opaque_bytes: 64,
        },
        Vec::new(),
    );
    assert!(matches!(
        codec.encode(&TranscriptRecord::user("x".repeat(65))),
        Err(TranscriptError::TextTooLarge)
    ));
    assert!(matches!(
        codec.validate_sequence(&[TranscriptRecord::tool_result("call-1", "orphan")]),
        Err(TranscriptError::UnpairedToolResult)
    ));
}

#[test]
fn transcript_codec_rejects_noncanonical_wire_and_escaped_tool_arguments() {
    let codec = TranscriptCodec::new(
        TranscriptLimits::default(),
        vec![SecretString::new(
            "provider-secret-value".to_owned().into_boxed_str(),
        )],
    );
    assert!(matches!(
        codec.decode(r#"{ "kind": "user", "text": "hello" }"#),
        Err(TranscriptError::NonCanonical)
    ));
    assert!(matches!(
        codec.decode(
            r#"{"kind":"opaque","digest":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","byte_len":1}"#
        ),
        Err(TranscriptError::InvalidWire)
    ));
    assert!(matches!(
        codec.encode(&TranscriptRecord::tool_call(
            "call-1",
            "read_file",
            r#"{"path":"provider-\u0073ecret-value"}"#
        )),
        Err(TranscriptError::InvalidWire)
    ));
}

#[test]
fn opaque_byte_length_changes_context_prefix_hash() {
    let assembler = ContextAssembler::new(
        ContextLimits {
            max_bytes: 128 * 1024,
            max_history_entries: 1,
        },
        Vec::new(),
    );
    let first = assembler
        .assemble(input(vec![
            TranscriptEntry::Opaque {
                digest: "a".repeat(64),
                byte_len: 1,
            },
            TranscriptEntry::user("retained"),
        ]))
        .unwrap();
    let second = assembler
        .assemble(input(vec![
            TranscriptEntry::Opaque {
                digest: "a".repeat(64),
                byte_len: 2,
            },
            TranscriptEntry::user("retained"),
        ]))
        .unwrap();
    assert_ne!(first.omitted_prefix_hash, second.omitted_prefix_hash);
}

#[test]
fn structured_tool_results_preserve_json_shape_after_recursive_redaction() {
    let secret = "provider-structured-secret";
    let codec = TranscriptCodec::new(
        TranscriptLimits::default(),
        vec![SecretString::new(secret.to_owned().into_boxed_str())],
    );
    let encoded = codec
        .encode_all(&[
            TranscriptRecord::tool_call("call-json", "read_file", r#"{"path":"src/lib.rs"}"#),
            TranscriptRecord::tool_result_json(
                "call-json",
                json!({
                    "summary": format!("Authorization: Bearer {secret}"),
                    "data": {"token": secret, "safe": true},
                }),
            ),
        ])
        .unwrap();
    assert!(!encoded.join("\n").contains(secret));

    let decoded = codec.decode_all(&encoded).unwrap();
    let TranscriptRecord::ToolResultJson { payload, .. } = &decoded[1] else {
        panic!("expected structured tool result");
    };
    assert_eq!(payload["data"]["safe"], true);
    assert_eq!(payload["data"]["token"], "[REDACTED]");
    assert_eq!(payload["summary"], "Authorization: Bearer [REDACTED]");
}
