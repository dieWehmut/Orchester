use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use orchester_laufzeit::harness::agent_loop::{AgentLoopConfig, SelfAgentLoop};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_laufzeit::harness::executor::ToolExecutor;
use orchester_laufzeit::harness::files::FileToolLimits;
use orchester_laufzeit::harness::run_store::{RunStore, SqliteRunStore};
use orchester_laufzeit::harness::service::{SelfAgentOutcome, SelfAgentRuntime, SelfAgentTurn};
use orchester_modell::{
    LanguageModel, ModelError, ModelEventSink, ModelRequest, ModelResponse, ModelUsage, ScriptedLlm,
};
use orchester_protokoll::{AgentAction, HarnessEventKind, PolicyDecision};
use tokio_util::sync::CancellationToken;

struct EventfulModel {
    inner: ScriptedLlm,
}

#[async_trait]
impl LanguageModel for EventfulModel {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        self.inner.complete(request, cancel).await
    }

    async fn complete_with_events(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<ModelResponse, ModelError> {
        let response = self.inner.complete(request, cancel).await?;
        if let Some(events) = events {
            if response.assistant_text == "inspection complete" {
                events.text_delta("inspection ");
                events.text_delta("complete");
            } else if !response.assistant_text.is_empty() {
                events.text_delta(&response.assistant_text);
            }
        }
        Ok(response)
    }
}

fn eventful_loop_engine(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> SelfAgentLoop<EventfulModel> {
    SelfAgentLoop::new(
        EventfulModel {
            inner: ScriptedLlm::new(responses),
        },
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        AgentLoopConfig {
            model: "test-model".into(),
            max_steps: 8,
            max_text_bytes: 64 * 1024,
            store: false,
        },
    )
    .expect("loop")
}

struct CollectingSink(Arc<std::sync::Mutex<Vec<String>>>);

impl ModelEventSink for CollectingSink {
    fn text_delta(&self, delta: &str) {
        self.0.lock().expect("sink lock").push(delta.to_owned());
    }
}

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-self-runtime-{label}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("workspace/src")).expect("create workspace");
    root
}

fn loop_engine(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> SelfAgentLoop<ScriptedLlm> {
    SelfAgentLoop::new(
        ScriptedLlm::new(responses),
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        AgentLoopConfig {
            model: "test-model".into(),
            max_steps: 8,
            max_text_bytes: 64 * 1024,
            store: false,
        },
    )
    .expect("loop")
}

fn tool_response(
    call_id: &str,
    name: &str,
    arguments_json: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> ModelResponse {
    let mut response = ModelResponse::tool(call_id, name, arguments_json);
    response.usage = ModelUsage {
        input_tokens,
        output_tokens,
    };
    response
}

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

fn runtime(
    root: &Path,
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> (
    SelfAgentRuntime<ScriptedLlm, JsonlAuditSink>,
    Arc<JsonlAuditSink>,
) {
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state/runs.db"), Vec::new())
            .expect("store"),
    );
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = SelfAgentRuntime::new(
        loop_engine(responses),
        store,
        audit.clone(),
        ToolExecutor::new(root.join("workspace"), FileToolLimits::default()).expect("executor"),
        root.join("workspace"),
        "local-user",
    )
    .expect("runtime");
    (runtime, audit)
}

#[tokio::test]
async fn allowed_read_runs_through_the_durable_execution_boundary() {
    let root = temp_root("read");
    std::fs::write(
        root.join("workspace/src/lib.rs"),
        "pub const VALUE: u8 = 7;\n",
    )
    .expect("fixture");
    let (runtime, audit) = runtime(
        &root,
        [Ok(ModelResponse::tool(
            "provider-call-read",
            "read_file",
            r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
        ))],
    );

    let outcome = runtime
        .start("read the source", CancellationToken::new())
        .await
        .expect("outcome");
    let SelfAgentOutcome::Tool {
        run_id,
        outcome: orchester_laufzeit::harness::execution::GovernedToolOutcome::Completed(observation),
        ..
    } = outcome
    else {
        panic!("expected completed tool outcome");
    };
    assert_eq!(observation.kind, "read_file");
    assert_eq!(
        observation.data["content_lines"],
        serde_json::json!(["pub const VALUE: u8 = 7;"])
    );
    assert_eq!(audit.verify().expect("audit").entries, 1);
    assert_eq!(runtime.model().call_count(), 1);
    let events = runtime
        .store()
        .events_owned(&run_id, "local-user")
        .expect("events");
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, HarnessEventKind::ToolCompleted { .. })));
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn non_file_actions_remain_visible_without_crossing_the_audit_barrier() {
    let root = temp_root("paused-actions");
    let responses = [
        Ok(ModelResponse::tool(
            "provider-call-network",
            "run_command",
            r#"{"program":"curl","args":["https://example.test"],"cwd":null}"#,
        )),
        Ok(ModelResponse::tool(
            "provider-call-write",
            "write_file",
            r#"{"path":"src/generated.rs","content":"not written"}"#,
        )),
        Ok(ModelResponse::tool(
            "provider-call-finish",
            "finish",
            r#"{"summary":"candidate"}"#,
        )),
    ];
    let (runtime, audit) = runtime(&root, responses);

    let network = runtime
        .start("network", CancellationToken::new())
        .await
        .expect("network");
    assert!(matches!(
        network,
        SelfAgentOutcome::Model(SelfAgentTurn::Action { ref policy, .. })
            if policy.decision == PolicyDecision::Ask
    ));
    let write = runtime
        .start("write", CancellationToken::new())
        .await
        .expect("write");
    assert!(matches!(
        write,
        SelfAgentOutcome::Model(SelfAgentTurn::Action {
            action: AgentAction::WriteFile { .. },
            ..
        })
    ));
    let finish = runtime
        .start("finish", CancellationToken::new())
        .await
        .expect("finish");
    assert!(matches!(
        finish,
        SelfAgentOutcome::Model(SelfAgentTurn::Action {
            action: AgentAction::Finish { .. },
            ..
        })
    ));
    assert_eq!(audit.verify().expect("audit").entries, 0);
    assert!(!root.join("workspace/src/generated.rs").exists());
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn run_executes_multiple_allowed_tools_then_returns_the_final_text() {
    let root = temp_root("multi-step");
    std::fs::write(
        root.join("workspace/src/lib.rs"),
        "pub const VALUE: u8 = 7;\n",
    )
    .expect("fixture");
    let responses = [
        Ok(tool_response(
            "provider-call-list",
            "list_files",
            r#"{"path":"src","depth":1}"#,
            3,
            5,
        )),
        Ok(tool_response(
            "provider-call-read",
            "read_file",
            r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
            4,
            6,
        )),
        Ok(text_response("inspection complete", 7, 11)),
    ];
    let (runtime, audit) = runtime(&root, responses);

    let outcome = runtime
        .run("inspect the source", CancellationToken::new())
        .await
        .expect("outcome");

    assert_eq!(outcome.final_turn().text(), Some("inspection complete"));
    assert_eq!(outcome.model_calls(), 3);
    assert_eq!(outcome.usage().input_tokens, 14);
    assert_eq!(outcome.usage().output_tokens, 22);
    assert_eq!(outcome.tool_steps().len(), 2);
    assert!(matches!(
        outcome.tool_steps()[0].outcome(),
        orchester_laufzeit::harness::execution::GovernedToolOutcome::Completed(observation)
            if observation.kind == "list_files"
    ));
    assert!(matches!(
        outcome.tool_steps()[1].outcome(),
        orchester_laufzeit::harness::execution::GovernedToolOutcome::Completed(observation)
            if observation.kind == "read_file"
    ));
    assert_eq!(audit.verify().expect("audit").entries, 2);
    assert_eq!(runtime.model().call_count(), 3);
    let events = runtime
        .store()
        .events_owned(outcome.run_id(), "local-user")
        .expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, HarnessEventKind::RunCreated))
            .count(),
        1
    );
    let steps = events
        .iter()
        .filter(|event| matches!(event.kind, HarnessEventKind::StepStarted))
        .collect::<Vec<_>>();
    assert_eq!(steps.len(), 3);
    assert!(steps
        .windows(2)
        .all(|pair| pair[0].turn_id == pair[1].turn_id));
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn run_with_events_forwards_ordered_text_without_changing_tool_sequence() {
    let root = temp_root("events");
    std::fs::write(
        root.join("workspace/src/lib.rs"),
        "pub const VALUE: u8 = 7;\n",
    )
    .expect("fixture");
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state/runs.db"), Vec::new())
            .expect("store"),
    );
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = SelfAgentRuntime::new(
        eventful_loop_engine([
            Ok(tool_response(
                "provider-call-read",
                "read_file",
                r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
                2,
                3,
            )),
            Ok(text_response("inspection complete", 4, 5)),
        ]),
        store,
        audit.clone(),
        ToolExecutor::new(root.join("workspace"), FileToolLimits::default()).expect("executor"),
        root.join("workspace"),
        "local-user",
    )
    .expect("runtime");
    let deltas = Arc::new(std::sync::Mutex::new(Vec::new()));

    let outcome = runtime
        .run_with_events(
            "inspect the source",
            CancellationToken::new(),
            Some(Arc::new(CollectingSink(Arc::clone(&deltas)))),
        )
        .await
        .expect("outcome");

    assert_eq!(outcome.final_turn().text(), Some("inspection complete"));
    assert_eq!(outcome.tool_steps().len(), 1);
    assert_eq!(
        *deltas.lock().expect("sink lock"),
        ["inspection ", "complete"]
    );
    assert_eq!(audit.verify().expect("audit").entries, 1);
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn run_stops_for_approval_after_returning_the_tool_observation_to_the_model() {
    let root = temp_root("approval-pause");
    std::fs::write(root.join("workspace/src/lib.rs"), "pub fn inspect() {}\n").expect("fixture");
    let responses = [
        Ok(tool_response(
            "provider-call-read",
            "read_file",
            r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
            2,
            3,
        )),
        Ok(tool_response(
            "provider-call-network",
            "run_command",
            r#"{"program":"curl","args":["https://example.test"],"cwd":null}"#,
            5,
            8,
        )),
    ];
    let (runtime, audit) = runtime(&root, responses);

    let outcome = runtime
        .run("inspect then fetch", CancellationToken::new())
        .await
        .expect("outcome");

    assert!(matches!(
        outcome.final_turn(),
        SelfAgentTurn::Action { policy, .. } if policy.decision == PolicyDecision::Ask
    ));
    assert_eq!(outcome.tool_steps().len(), 1);
    assert_eq!(outcome.model_calls(), 2);
    assert_eq!(audit.verify().expect("audit").entries, 1);
    assert_eq!(runtime.model().call_count(), 2);
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn run_continues_after_a_governed_tool_failure() {
    let root = temp_root("failed-tool");
    let responses = [
        Ok(tool_response(
            "provider-call-read",
            "read_file",
            r#"{"path":"src/missing.rs","start_line":null,"end_line":null}"#,
            2,
            3,
        )),
        Ok(text_response("the requested file is missing", 5, 8)),
    ];
    let (runtime, audit) = runtime(&root, responses);

    let outcome = runtime
        .run("inspect the missing file", CancellationToken::new())
        .await
        .expect("outcome");

    assert_eq!(
        outcome.final_turn().text(),
        Some("the requested file is missing")
    );
    assert!(matches!(
        outcome.tool_steps()[0].outcome(),
        orchester_laufzeit::harness::execution::GovernedToolOutcome::Failed(_)
    ));
    assert_eq!(outcome.model_calls(), 2);
    assert_eq!(audit.verify().expect("audit").entries, 1);
    assert_eq!(runtime.model().call_count(), 2);
    drop(runtime);
    drop(audit);
    let _ = std::fs::remove_dir_all(root);
}
