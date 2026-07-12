use orchester_laufzeit::harness::agent_loop::{
    AgentLoopConfig, AgentLoopError, AgentLoopOutcome, AgentLoopStop, SelfAgentLoop,
};
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_modell::{ModelError, ModelResponse, ModelRole, ModelUsage, ScriptedLlm};
use orchester_protokoll::AgentAction;
use tokio_util::sync::CancellationToken;

fn text_response(text: &str, input_tokens: u64, output_tokens: u64) -> ModelResponse {
    ModelResponse {
        assistant_text: text.into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens,
            output_tokens,
        },
        opaque_items: Vec::new(),
    }
}

fn test_loop(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
    max_steps: u32,
) -> SelfAgentLoop<ScriptedLlm> {
    SelfAgentLoop::new(
        ScriptedLlm::new(responses),
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        AgentLoopConfig {
            model: "test-model".into(),
            max_steps,
            max_text_bytes: 64 * 1024,
            store: false,
        },
    )
    .unwrap()
}

#[test]
fn loop_configuration_rejects_unbounded_or_empty_limits() {
    for (max_steps, max_text_bytes) in [(0, 1), (257, 1), (1, 0), (1, 512 * 1024 + 1)] {
        let error = SelfAgentLoop::new(
            ScriptedLlm::new(Vec::<Result<ModelResponse, ModelError>>::new()),
            ContextAssembler::new(ContextLimits::default(), Vec::new()),
            AgentLoopConfig {
                model: "test-model".into(),
                max_steps,
                max_text_bytes,
                store: false,
            },
        )
        .unwrap_err();
        assert!(matches!(error, AgentLoopError::InvalidConfig));
    }
}

#[tokio::test]
async fn assistant_text_finishes_one_model_step() {
    let agent = test_loop([Ok(text_response("done", 7, 3))], 4);

    let outcome = agent
        .start("inspect the workspace", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Final(result) = outcome else {
        panic!("expected final result");
    };

    assert_eq!(result.final_text(), "done");
    assert_eq!(result.stop(), AgentLoopStop::AssistantText);
    assert_eq!(result.model_calls(), 1);
    assert_eq!(result.usage().input_tokens, 7);
    assert_eq!(result.usage().output_tokens, 3);
    assert_eq!(agent.model().call_count(), 1);
}

#[tokio::test]
async fn pending_action_resumes_with_one_paired_tool_result() {
    let agent = test_loop(
        [
            Ok(ModelResponse::tool(
                "call-1",
                "read_file",
                r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
            )),
            Ok(text_response("finished after reading", 11, 5)),
        ],
        4,
    );

    let first = agent
        .start("inspect the workspace", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Pending(pending) = first else {
        panic!("expected pending action");
    };
    assert_eq!(pending.call_id().0, "call-1");
    assert!(matches!(
        pending.action(),
        AgentAction::ReadFile { path, .. } if path == "src/lib.rs"
    ));

    let second = agent
        .resume(pending, "bounded file contents", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Final(result) = second else {
        panic!("expected final result");
    };
    assert_eq!(result.final_text(), "finished after reading");
    assert_eq!(result.model_calls(), 2);

    let summaries = agent.model().request_summaries();
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[1].message_roles,
        vec![
            ModelRole::System,
            ModelRole::User,
            ModelRole::Assistant,
            ModelRole::Tool,
        ]
    );
    assert_eq!(summaries[1].tool_call_item_count, 1);
    assert_eq!(summaries[1].tool_result_item_count, 1);
}

#[tokio::test]
async fn finish_tool_completes_without_leaking_an_action_to_an_executor() {
    let agent = test_loop(
        [Ok(ModelResponse::tool(
            "call-finish",
            "finish",
            r#"{"summary":"validated"}"#,
        ))],
        2,
    );

    let outcome = agent
        .start("complete the task", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Final(result) = outcome else {
        panic!("finish must be terminal");
    };
    assert_eq!(result.final_text(), "validated");
    assert_eq!(result.stop(), AgentLoopStop::FinishTool);
}

#[tokio::test]
async fn step_budget_stops_before_a_second_model_call() {
    let agent = test_loop(
        [
            Ok(ModelResponse::tool(
                "call-1",
                "read_file",
                r#"{"path":"a","start_line":null,"end_line":null}"#,
            )),
            Ok(text_response("must remain queued", 1, 1)),
        ],
        1,
    );
    let first = agent
        .start("inspect", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Pending(pending) = first else {
        panic!("expected pending action");
    };

    let error = agent
        .resume(pending, "observation", CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, AgentLoopError::StepBudgetExceeded));
    assert_eq!(agent.model().call_count(), 1);
}

#[tokio::test]
async fn oversized_tool_result_is_rejected_before_a_follow_up_model_call() {
    let agent = SelfAgentLoop::new(
        ScriptedLlm::new([Ok(ModelResponse::tool(
            "call-1",
            "read_file",
            r#"{"path":"a","start_line":null,"end_line":null}"#,
        ))]),
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        AgentLoopConfig {
            model: "test-model".into(),
            max_steps: 2,
            max_text_bytes: 8,
            store: false,
        },
    )
    .unwrap();
    let first = agent
        .start("inspect", CancellationToken::new())
        .await
        .unwrap();
    let AgentLoopOutcome::Pending(pending) = first else {
        panic!("expected pending action");
    };

    let error = agent
        .resume(pending, "123456789", CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, AgentLoopError::ToolResultTooLarge));
    assert_eq!(agent.model().call_count(), 1);
}

#[tokio::test]
async fn cancellation_does_not_consume_the_scripted_model() {
    let agent = test_loop([Ok(text_response("unused", 1, 1))], 2);
    let cancelled = CancellationToken::new();
    cancelled.cancel();

    let error = agent.start("inspect", cancelled).await.unwrap_err();
    assert!(matches!(
        error,
        AgentLoopError::Model(ModelError::Cancelled)
    ));
    assert_eq!(agent.model().call_count(), 0);
}

#[tokio::test]
async fn malformed_model_actions_fail_closed_without_echoing_arguments() {
    let secret = "sk-do-not-echo";
    let agent = test_loop(
        [Ok(ModelResponse::tool(
            "call-1",
            "unknown_tool",
            format!(r#"{{"api_key":"{secret}"}}"#),
        ))],
        2,
    );

    let error = agent
        .start("inspect", CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, AgentLoopError::Decode(_)));
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(secret));
    assert!(!rendered.contains("unknown_tool"));
}
