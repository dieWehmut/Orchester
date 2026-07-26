use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use orchester_laufzeit::harness::agent_loop::{AgentLoopConfig, SelfAgentLoop};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_laufzeit::harness::coordinator::FixedCoordinatorClock;
use orchester_laufzeit::harness::execution::GovernedExecution;
use orchester_laufzeit::harness::executor::ToolExecutor;
use orchester_laufzeit::harness::files::FileToolLimits;
use orchester_laufzeit::harness::run_store::{RunStatus, RunStore, SqliteRunStore};
use orchester_laufzeit::harness::service::{
    SelfAgentService, SelfAgentServiceError, SelfAgentTurn,
};
use orchester_modell::{ModelError, ModelResponse, ModelUsage, ScriptedLlm};
use orchester_protokoll::{AgentAction, HarnessEventKind, PolicyDecision};
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
        policy,
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
    assert_eq!(policy.decision, PolicyDecision::Allow);
    assert_eq!(policy.rule_id, "workspace.read");

    let events = service
        .store()
        .events_owned(turn.run_id(), "local-user")
        .expect("durable events");
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, HarnessEventKind::ActionRecorded { .. })));
    assert!(events
        .iter()
        .any(|event| matches!(event.kind, HarnessEventKind::PolicyDecided { .. })));
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn exposes_a_policy_ask_for_external_network_commands() {
    let workspace = temp_workspace("policy-review");
    let service = service(
        &workspace,
        [Ok(ModelResponse::tool(
            "provider-call-ask",
            "run_command",
            r#"{"program":"curl","args":["https://example.test"],"cwd":null}"#,
        ))],
    );
    let turn = service
        .start("fetch external data", CancellationToken::new())
        .await
        .expect("turn");

    let SelfAgentTurn::Action { policy, .. } = turn else {
        panic!("expected action");
    };
    assert_eq!(policy.decision, PolicyDecision::Ask);
    assert_eq!(policy.rule_id, "network.external");
    let _ = std::fs::remove_dir_all(workspace);
}

#[tokio::test]
async fn exposes_a_policy_deny_for_root_destructive_commands() {
    let workspace = temp_workspace("policy-deny");
    let service = service(
        &workspace,
        [Ok(ModelResponse::tool(
            "provider-call-deny",
            "run_command",
            r#"{"program":"rm","args":["-rf","/"],"cwd":null}"#,
        ))],
    );
    let turn = service
        .start("remove the root filesystem", CancellationToken::new())
        .await
        .expect("turn");

    let SelfAgentTurn::Action { policy, .. } = turn else {
        panic!("expected action");
    };
    assert_eq!(policy.decision, PolicyDecision::Deny);
    assert_eq!(policy.rule_id, "system.destructive");
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

#[tokio::test]
async fn continues_an_observed_tool_step_without_creating_a_second_run() {
    let workspace = temp_workspace("continue");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/lib.rs"), "pub const VALUE: u8 = 7;\n").unwrap();
    let mut tool_response = ModelResponse::tool(
        "provider-read-1",
        "read_file",
        r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
    );
    tool_response.usage = ModelUsage {
        input_tokens: 3,
        output_tokens: 5,
    };
    let final_response = ModelResponse {
        assistant_text: "inspection complete".into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens: 7,
            output_tokens: 11,
        },
        opaque_items: Vec::new(),
    };
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(workspace.join("state/runs.db"), Vec::new())
            .unwrap(),
    );
    let service = SelfAgentService::with_clock(
        loop_engine([Ok(tool_response), Ok(final_response)]),
        store.clone(),
        &workspace,
        "local-user",
        FixedCoordinatorClock::new("2026-07-18T00:00:00Z"),
    )
    .unwrap();
    let first = service
        .start("inspect", CancellationToken::new())
        .await
        .unwrap();
    let SelfAgentTurn::Action {
        run_id,
        action_id,
        call_id,
        ..
    } = first
    else {
        panic!("expected read action");
    };
    let audit = Arc::new(JsonlAuditSink::open(workspace.join("state/audit.jsonl")).unwrap());
    let execution = GovernedExecution::with_clock(
        store.clone(),
        audit,
        ToolExecutor::new(&workspace, FileToolLimits::default()).unwrap(),
        "local-user",
        FixedCoordinatorClock::new("2026-07-18T00:00:01Z"),
    )
    .unwrap();
    execution.execute(&run_id, &action_id, &call_id).unwrap();

    let continued = service
        .continue_run(run_id.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(continued.run_id(), &run_id);
    assert_eq!(continued.text(), Some("inspection complete"));
    assert_eq!(continued.model_calls(), 2);
    assert_eq!(continued.usage().input_tokens, 10);
    assert_eq!(continued.usage().output_tokens, 16);
    let events = store.events_owned(&run_id, "local-user").unwrap();
    let steps = events
        .iter()
        .filter(|event| matches!(event.kind, HarnessEventKind::StepStarted))
        .collect::<Vec<_>>();
    assert_eq!(steps.len(), 2);
    assert_ne!(steps[0].step_id, steps[1].step_id);
    assert_eq!(steps[0].turn_id, steps[1].turn_id);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, HarnessEventKind::RunCreated))
            .count(),
        1
    );
    drop(execution);
    drop(service);
    drop(store);
    let _ = std::fs::remove_dir_all(workspace);
}
