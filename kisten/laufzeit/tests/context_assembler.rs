use orchester_laufzeit::harness::context::{
    ContextAssembler, ContextError, ContextInput, ContextLimits, TranscriptEntry,
};
use orchester_modell::{ModelItem, ModelRole};
use secrecy::SecretString;

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
