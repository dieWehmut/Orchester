use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::agent_loop::{AgentLoopConfig, SelfAgentLoop};
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_laufzeit::harness::coordinator::FixedCoordinatorClock;
use orchester_laufzeit::harness::run_store::{RunStatus, RunStore, SqliteRunStore};
use orchester_laufzeit::harness::service::{
    SelfAgentService, SelfAgentServiceError, SelfAgentTurn,
};
use orchester_modell::{ModelError, ModelResponse, ModelUsage, ScriptedLlm};
use orchester_protokoll::{AgentAction, HarnessEventKind};
use tokio_util::sync::CancellationToken;

static NEXT_WORKSPACE: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "orchester-self-service-{label}-{}-{}",
        std::process::id(),
        NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("create workspace");
    path
}

fn loop_engine(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> SelfAgentLoop<ScriptedLlm> {
    SelfAgentLoop::new(
        ScriptedLlm::new(responses),
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        AgentLoopConfig {
            model: "test-model".into(),
            max_steps: 4,
            max_text_bytes: 64 * 1024,
            store: false,
        },
    )
    .expect("valid loop")
}

fn service(
    workspace: &Path,
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> SelfAgentService<ScriptedLlm, SqliteRunStore, FixedCoordinatorClock> {
    SelfAgentService::with_clock(
        loop_engine(responses),
        SqliteRunStore::in_memory().expect("store"),
        workspace,
        "local-user",
        FixedCoordinatorClock::new("2026-07-18T00:00:00Z"),
    )
    .expect("service")
}

fn text_response(text: &str) -> ModelResponse {
    ModelResponse {
        assistant_text: text.into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens: 8,
            output_tokens: 3,
        },
        opaque_items: Vec::new(),
    }
}

#[tokio::test]
async fn starts_a_durable_text_turn_and_exposes_bounded_metadata() {
    let workspace = temp_workspace("text");
    let service = service(&workspace, [Ok(text_response("finished"))]);
    let identity = service.identity();
    let turn = service
        .start("inspect", CancellationToken::new())
        .await
        .expect("turn");

    assert_eq!(turn.text(), Some("finished"));
    assert_eq!(turn.model_calls(), 1);
    assert_eq!(turn.usage().input_tokens, 8);
    assert!(turn.run_id().0.starts_with("run-"));
    assert!(identity.project_id.starts_with("project-"));
    assert!(identity.workspace_identity.starts_with("workspace-"));
    assert_eq!(identity.owner_actor_id, "local-user");

    let snapshot = service
        .store()
        .load_run_owned(turn.run_id(), "local-user")
        .expect("durable run");
    assert_eq!(snapshot.status, RunStatus::Running);
    let events = service
        .store()
        .events_owned(turn.run_id(), "local-user")
        .expect("durable events");
    assert!(events.iter().any(|event| matches!(
        event.kind,
        HarnessEventKind::ModelCompleted { ref assistant_text } if assistant_text == "finished"
    )));

    let debug = format!("{service:?} {turn:?} {identity:?}");
    assert!(!debug.contains("finished"));
    assert!(!debug.contains(&workspace.to_string_lossy().to_string()));
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn returns_a_policy_classified_action_with_durable_identity() {
    let workspace = temp_workspace("action");
    let service = service(
        &workspace,
        [Ok(ModelResponse::tool(
            "provider-call-1",
            "read_file",
            r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
        ))],
    );
    let turn = service
        .start("read the file", CancellationToken::new())
        .await
        .expect("turn");

    let SelfAgentTurn::Action {
        action_id,
        call_id,
        action,
        ..
    } = &turn
    else {
        panic!("expected action");
    };
    assert!(action_id.0.starts_with("action-"));
    assert_eq!(call_id.0, "provider-call-1");
    assert!(matches!(
        action,
        AgentAction::ReadFile { path, .. } if path == "src/lib.rs"
    ));

    let events = service
        .store()
        .events_owned(turn.run_id(), "local-user")
        .expect("durable events");
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, HarnessEventKind::ActionRecorded { .. })));
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn gives_each_new_turn_a_unique_run_identity() {
    let workspace = temp_workspace("unique");
    let service = service(
        &workspace,
        [Ok(text_response("one")), Ok(text_response("two"))],
    );
    let first = service
        .start("first", CancellationToken::new())
        .await
        .expect("first");
    let second = service
        .start("second", CancellationToken::new())
        .await
        .expect("second");
    assert_ne!(first.run_id(), second.run_id());
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn rejects_missing_workspaces_and_invalid_owner_identifiers() {
    let missing = temp_workspace("missing").join("not-there");
    let error = SelfAgentService::with_clock(
        loop_engine(Vec::<Result<ModelResponse, ModelError>>::new()),
        SqliteRunStore::in_memory().expect("store"),
        &missing,
        "local-user",
        FixedCoordinatorClock::new("2026-07-18T00:00:00Z"),
    )
    .expect_err("missing workspace");
    assert!(matches!(error, SelfAgentServiceError::Identity(_)));

    let workspace = temp_workspace("invalid-owner");
    let error = SelfAgentService::with_clock(
        loop_engine(Vec::<Result<ModelResponse, ModelError>>::new()),
        SqliteRunStore::in_memory().expect("store"),
        &workspace,
        "bad\nowner",
        FixedCoordinatorClock::new("2026-07-18T00:00:00Z"),
    )
    .expect_err("invalid owner");
    assert!(matches!(error, SelfAgentServiceError::Identity(_)));
    let _ = std::fs::remove_dir_all(workspace);
}
