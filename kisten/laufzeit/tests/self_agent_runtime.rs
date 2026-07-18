use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use orchester_laufzeit::harness::agent_loop::{AgentLoopConfig, SelfAgentLoop};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_laufzeit::harness::executor::ToolExecutor;
use orchester_laufzeit::harness::files::FileToolLimits;
use orchester_laufzeit::harness::run_store::{RunStore, SqliteRunStore};
use orchester_laufzeit::harness::service::{SelfAgentOutcome, SelfAgentRuntime, SelfAgentTurn};
use orchester_modell::{ModelError, ModelResponse, ScriptedLlm};
use orchester_protokoll::{AgentAction, HarnessEventKind, PolicyDecision};
use tokio_util::sync::CancellationToken;

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
