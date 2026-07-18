use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use orchester_laufzeit::harness::agent_loop::{AgentLoopConfig, SelfAgentLoop};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::context::{ContextAssembler, ContextLimits};
use orchester_laufzeit::harness::coordinator::{
    CoordinatorClock, CoordinatorContinuationInput, CoordinatorError, CoordinatorInput,
    CoordinatorOutcome, CoordinatorStore, DurableCoordinator, FixedCoordinatorClock,
};
use orchester_laufzeit::harness::execution::{GovernedExecution, GovernedToolOutcome};
use orchester_laufzeit::harness::executor::ToolExecutor;
use orchester_laufzeit::harness::files::FileToolLimits;
use orchester_laufzeit::harness::governance::{PolicyEngine, PolicyResult};
use orchester_laufzeit::harness::run_store::{
    ActionRecord, EventAppend, NewRun, ResumeNext, RunStore, SqliteRunStore, StoreError, Transition,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_laufzeit::harness::SecretSetId;
use orchester_modell::{ModelError, ModelResponse, ModelUsage, ScriptedLlm};
use orchester_protokoll::{ActionId, AgentAction, CallId, PolicyDecision, RunId, StepId, TurnId};
use secrecy::SecretString;
use tokio_util::sync::CancellationToken;

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn temp_db(label: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "orchester-coordinator-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
        .join("state.db")
}

fn remove_temp_db(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().expect("database parent"));
}

fn new_run(id: &str) -> NewRun {
    NewRun {
        run_id: RunId::from(id),
        project_id: format!("project-{id}"),
        owner_actor_id: "owner-coordinator".into(),
        canonical_root: format!("/workspace/{id}"),
        workspace_identity: format!("workspace-{id}"),
        policy_snapshot_hash: PolicyEngine::snapshot_hash(),
        config_snapshot_hash: test_config_snapshot_hash(),
        max_steps: 4,
        occurred_at: "2026-07-13T00:00:00Z".into(),
    }
}

fn input(run_id: &str) -> CoordinatorInput {
    CoordinatorInput {
        run: new_run(run_id),
        prompt: "inspect the workspace".into(),
        turn_id: TurnId::from("turn-1"),
        step_id: StepId::from("step-1"),
        model_call_id: CallId::from("model-call-1"),
        action_id: ActionId::from("action-1"),
    }
}

fn test_config() -> AgentLoopConfig {
    AgentLoopConfig {
        model: "test-model".into(),
        max_steps: 4,
        max_text_bytes: 64 * 1024,
        store: false,
    }
}

fn test_config_snapshot_hash() -> String {
    SelfAgentLoop::new(
        ScriptedLlm::new(Vec::<Result<ModelResponse, ModelError>>::new()),
        ContextAssembler::new(ContextLimits::default(), Vec::new()),
        test_config(),
    )
    .unwrap()
    .config_snapshot_hash()
}

fn agent(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
) -> SelfAgentLoop<ScriptedLlm> {
    agent_with_secrets(responses, Vec::new())
}

fn agent_with_secrets(
    responses: impl IntoIterator<Item = Result<ModelResponse, ModelError>>,
    secrets: Vec<SecretString>,
) -> SelfAgentLoop<ScriptedLlm> {
    SelfAgentLoop::new(
        ScriptedLlm::new(responses),
        ContextAssembler::new(ContextLimits::default(), secrets),
        test_config(),
    )
    .unwrap()
}

fn coordinator(
    loop_engine: SelfAgentLoop<ScriptedLlm>,
    store: SqliteRunStore,
) -> DurableCoordinator<ScriptedLlm, SqliteRunStore, FixedCoordinatorClock> {
    DurableCoordinator::with_clock(
        loop_engine,
        store,
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    )
}

struct SequenceClock {
    values: Mutex<VecDeque<String>>,
}

impl SequenceClock {
    fn new(values: impl IntoIterator<Item = String>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
        }
    }
}

impl CoordinatorClock for SequenceClock {
    fn now(&self) -> String {
        self.values
            .lock()
            .unwrap()
            .pop_front()
            .expect("test clock exhausted")
    }
}

#[derive(Default)]
struct RecordingStore {
    calls: Mutex<Vec<&'static str>>,
}

impl CoordinatorStore for RecordingStore {
    fn secret_set_id(&self) -> SecretSetId {
        SecretSetId::empty()
    }

    fn create_run(&self, _input: NewRun) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("run.created");
        Ok(())
    }

    fn load_continuation(
        &self,
        _run_id: &RunId,
        _owner_actor_id: &str,
    ) -> Result<orchester_laufzeit::harness::coordinator::CoordinatorContinuationState, StoreError>
    {
        Err(StoreError::NotFound)
    }

    fn append_transition(
        &self,
        _run_id: &RunId,
        _owner_actor_id: &str,
        _transition: Transition,
    ) -> Result<(), StoreError> {
        self.calls.lock().unwrap().push("step.started");
        Ok(())
    }

    fn append_model_started_with_transcript(
        &self,
        _owner_actor_id: &str,
        _run_id: &RunId,
        _input: EventAppend,
        records: Vec<TranscriptRecord>,
    ) -> Result<(), StoreError> {
        assert_eq!(
            records,
            vec![TranscriptRecord::user("inspect the workspace")]
        );
        self.calls.lock().unwrap().push("model.started");
        Ok(())
    }

    fn append_model_completed(
        &self,
        _owner_actor_id: &str,
        _run_id: &RunId,
        input: EventAppend,
        _usage: ModelUsage,
    ) -> Result<String, StoreError> {
        let orchester_protokoll::HarnessEventKind::ModelCompleted { assistant_text } = input.kind
        else {
            panic!("expected model completion")
        };
        self.calls.lock().unwrap().push("model.completed");
        Ok(assistant_text)
    }

    fn append_model_completed_with_action(
        &self,
        _owner_actor_id: &str,
        _run_id: &RunId,
        _input: EventAppend,
        _action: ActionRecord,
        _usage: ModelUsage,
    ) -> Result<(), StoreError> {
        self.calls
            .lock()
            .unwrap()
            .push("model.completed+action.recorded");
        Ok(())
    }

    fn decide_policy(
        &self,
        _owner_actor_id: &str,
        _run_id: &RunId,
        _action_id: &ActionId,
        _occurred_at: String,
    ) -> Result<PolicyResult, StoreError> {
        self.calls.lock().unwrap().push("policy.decided");
        PolicyEngine::new()
            .evaluate(&AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            })
            .map_err(|_| StoreError::Invariant("test policy failed".into()))
    }
}

#[tokio::test]
async fn model_and_store_ports_are_provider_free_and_ordered() {
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "bounded answer".into(),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        RecordingStore::default(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );

    coordinator
        .start_new_run(input("run-fake-store"), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(coordinator.model().call_count(), 1);
    assert_eq!(
        *coordinator.store().calls.lock().unwrap(),
        vec![
            "run.created",
            "step.started",
            "model.started",
            "model.completed"
        ]
    );
}

#[tokio::test]
async fn coordinator_accepts_a_shared_store_for_governed_execution() {
    let path = temp_db("shared-store");
    let store = Arc::new(SqliteRunStore::open(&path).expect("store"));
    let coordinator = DurableCoordinator::with_clock(
        agent([Ok(ModelResponse {
            assistant_text: "shared result".into(),
            tool_call: None,
            usage: ModelUsage::default(),
            opaque_items: Vec::new(),
        })]),
        store.clone(),
        FixedCoordinatorClock::new("2026-07-18T00:00:00Z"),
    );

    let outcome = coordinator
        .start_new_run(input("run-shared-store"), CancellationToken::new())
        .await
        .expect("turn");
    assert!(matches!(outcome, CoordinatorOutcome::Text { .. }));
    assert_eq!(
        store
            .events_owned(&RunId::from("run-shared-store"), "owner-coordinator")
            .expect("events")
            .len(),
        4
    );
    drop(coordinator);
    drop(store);
    remove_temp_db(&path);
}

#[tokio::test]
async fn dependency_snapshot_mismatch_fails_before_store_or_model() {
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "must remain unused".into(),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        RecordingStore::default(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );

    let mut requests = [
        input("run-config-mismatch"),
        input("run-policy-mismatch"),
        input("run-budget-mismatch"),
    ];
    requests[0].run.config_snapshot_hash = "different-config".into();
    requests[1].run.policy_snapshot_hash = "different-policy".into();
    requests[2].run.max_steps += 1;

    for request in requests {
        let error = coordinator
            .start_new_run(request, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(error, CoordinatorError::DependencyMismatch));
    }
    assert_eq!(coordinator.model().call_count(), 0);
    assert!(coordinator.store().calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn run_metadata_is_rejected_before_any_durable_write() {
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "must remain unused".into(),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        RecordingStore::default(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );
    let mut request = input("run-metadata-rejection");
    request.run.canonical_root = "C:/workspace/sk-metadata-secret".into();

    let error = coordinator
        .start_new_run(request, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::InvalidInput));
    assert_eq!(coordinator.model().call_count(), 0);
    assert!(coordinator.store().calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mismatched_context_and_store_secret_sets_fail_before_model() {
    let path = temp_db("secret-set-mismatch");
    let secret = "configured-store-only-secret";
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "must remain unused".into(),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );

    let error = coordinator
        .start_new_run(input("run-secret-mismatch"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::DependencyMismatch));
    assert_eq!(coordinator.model().call_count(), 0);
    drop(coordinator);
    remove_temp_db(&path);
}

#[tokio::test]
async fn text_outcome_uses_the_store_sanitized_completion() {
    let path = temp_db("sanitized-text-outcome");
    let secret = "configured-model-output-secret";
    let loop_engine = agent_with_secrets(
        [Ok(ModelResponse {
            assistant_text: format!("answer {secret}"),
            tool_call: None,
            usage: ModelUsage::default(),
            opaque_items: Vec::new(),
        })],
        vec![SecretString::new(secret.to_owned().into_boxed_str())],
    );
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );

    let outcome = coordinator
        .start_new_run(input("run-sanitized-text"), CancellationToken::new())
        .await
        .unwrap();
    let CoordinatorOutcome::Text { text, .. } = outcome else {
        panic!("expected text outcome")
    };
    assert_eq!(text, "answer [REDACTED]");
    assert!(!text.contains(secret));
    drop(coordinator);
    remove_temp_db(&path);
}

#[test]
fn coordinator_input_debug_redacts_prompt_and_run_metadata() {
    let secret = "sk-coordinator-debug-secret";
    let mut request = input("run-debug");
    request.prompt = secret.into();
    request.run.canonical_root = format!("/workspace/{secret}");
    request.run.workspace_identity = secret.into();

    let rendered = format!("{request:?}");
    assert!(!rendered.contains(secret));
    assert!(rendered.contains("prompt_bytes"));
}

#[tokio::test]
async fn text_step_is_durable_before_the_next_resume_boundary() {
    let path = temp_db("text");
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "bounded answer".into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens: 3,
            output_tokens: 5,
        },
        opaque_items: Vec::new(),
    })]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let outcome = coordinator
        .start_new_run(input("run-text"), CancellationToken::new())
        .await
        .unwrap();
    assert!(matches!(outcome, CoordinatorOutcome::Text { .. }));
    assert_eq!(coordinator.model().call_count(), 1);
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let run_id = RunId::from("run-text");
    let snapshot = store.load_run_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(snapshot.input_tokens_used, 3);
    assert_eq!(snapshot.output_tokens_used, 5);
    let events = store.events_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind_name())
            .collect::<Vec<_>>(),
        vec![
            "run.created",
            "step.started",
            "model.started",
            "model.completed"
        ]
    );
    assert_eq!(events[0].occurred_at, "2026-07-13T00:00:01Z");
    let resume = store
        .resume_point_owned(&run_id, "owner-coordinator", "project-run-text")
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::ProcessModelOutput { .. }));
    assert!(!matches!(
        resume.status,
        orchester_laufzeit::harness::run_store::RunStatus::Succeeded
    ));
    remove_temp_db(&path);
}

#[tokio::test]
async fn oversized_model_usage_rolls_back_the_completion_transaction() {
    let path = temp_db("oversized-model-usage");
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "bounded answer".into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens: u64::MAX,
            output_tokens: 1,
        },
        opaque_items: Vec::new(),
    })]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let error = coordinator
        .start_new_run(input("run-oversized-model-usage"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        CoordinatorError::Store(StoreError::Invariant(_))
    ));
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let run_id = RunId::from("run-oversized-model-usage");
    let snapshot = store.load_run_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(snapshot.input_tokens_used, 0);
    assert_eq!(snapshot.output_tokens_used, 0);
    let events = store.events_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(events.last().unwrap().kind_name(), "model.started");
    remove_temp_db(&path);
}

#[tokio::test]
async fn tool_step_persists_the_store_owned_policy_decision() {
    let path = temp_db("tool");
    let mut response = ModelResponse::tool(
        "tool-call-1",
        "read_file",
        r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
    );
    response.assistant_text = "I will inspect the file first.".into();
    response.usage = ModelUsage {
        input_tokens: 7,
        output_tokens: 11,
    };
    let loop_engine = agent([Ok(response)]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let outcome = coordinator
        .start_new_run(input("run-tool"), CancellationToken::new())
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CoordinatorOutcome::Action {
            ref policy,
            ..
        } if policy.decision == PolicyDecision::Allow
            && policy.rule_id == "workspace.read"
    ));
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let run_id = RunId::from("run-tool");
    let snapshot = store.load_run_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(snapshot.input_tokens_used, 7);
    assert_eq!(snapshot.output_tokens_used, 11);
    let events = store.events_owned(&run_id, "owner-coordinator").unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind_name())
            .collect::<Vec<_>>(),
        vec![
            "run.created",
            "step.started",
            "model.started",
            "model.completed",
            "action.recorded",
            "policy.decided"
        ]
    );
    let assistant_text = match &events[3].kind {
        orchester_protokoll::HarnessEventKind::ModelCompleted { assistant_text } => assistant_text,
        other => panic!("unexpected event kind: {other:?}"),
    };
    assert_eq!(assistant_text, "I will inspect the file first.");
    let resume = store
        .resume_point_owned(&run_id, "owner-coordinator", "project-run-tool")
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::PrepareExecution { .. }));
    remove_temp_db(&path);
}

#[tokio::test]
async fn model_call_is_not_started_when_model_started_cannot_be_persisted() {
    let path = temp_db("start-failure");
    let secret = "configured-coordinator-secret";
    let loop_engine = agent_with_secrets(
        [Ok(ModelResponse {
            assistant_text: "must not run".into(),
            tool_call: None,
            usage: ModelUsage::default(),
            opaque_items: Vec::new(),
        })],
        vec![SecretString::new(secret.to_owned().into_boxed_str())],
    );
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap(),
        SequenceClock::new([
            "2026-07-13T00:00:01Z".into(),
            "2026-07-13T00:00:02Z".into(),
            format!("2026-07-13T00:00:03Z-{secret}"),
        ]),
    );

    let error = coordinator
        .start_new_run(input("run-start-failure"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::InvalidInput));
    assert_eq!(coordinator.model().call_count(), 0);
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let resume = store
        .resume_point_owned(
            &RunId::from("run-start-failure"),
            "owner-coordinator",
            "project-run-start-failure",
        )
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::StartModel { .. }));
    remove_temp_db(&path);
}

#[tokio::test]
async fn oversized_model_text_stays_at_manual_model_reconciliation() {
    let path = temp_db("oversized-model-text");
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "x".repeat(32 * 1024 + 1),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let error = coordinator
        .start_new_run(input("run-oversized-model-text"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::DurableTextTooLarge));
    assert_eq!(coordinator.model().call_count(), 1);
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let resume = store
        .resume_point_owned(
            &RunId::from("run-oversized-model-text"),
            "owner-coordinator",
            "project-run-oversized-model-text",
        )
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::ReconcileModelCall { .. }));
    remove_temp_db(&path);
}

#[tokio::test]
async fn oversized_prompt_fails_before_store_or_model() {
    let loop_engine = agent([Ok(ModelResponse {
        assistant_text: "must remain unused".into(),
        tool_call: None,
        usage: ModelUsage::default(),
        opaque_items: Vec::new(),
    })]);
    let coordinator = DurableCoordinator::with_clock(
        loop_engine,
        RecordingStore::default(),
        FixedCoordinatorClock::new("2026-07-13T00:00:01Z"),
    );
    let mut request = input("run-oversized-prompt");
    request.prompt = "x".repeat(32 * 1024 + 1);

    let error = coordinator
        .start_new_run(request, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::DurableTextTooLarge));
    assert_eq!(coordinator.model().call_count(), 0);
    assert!(coordinator.store().calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn oversized_tool_call_stays_at_manual_model_reconciliation() {
    let path = temp_db("oversized-tool-call");
    let arguments = serde_json::json!({
        "path": "src/generated.rs",
        "content": "x".repeat(70 * 1024),
    })
    .to_string();
    let loop_engine = agent([Ok(ModelResponse::tool(
        "oversized-tool-call-1",
        "write_file",
        arguments,
    ))]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let error = coordinator
        .start_new_run(input("run-oversized-tool"), CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoordinatorError::DurableActionInvalid));
    assert_eq!(coordinator.model().call_count(), 1);
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let resume = store
        .resume_point_owned(
            &RunId::from("run-oversized-tool"),
            "owner-coordinator",
            "project-run-oversized-tool",
        )
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::ReconcileModelCall { .. }));
    remove_temp_db(&path);
}

#[tokio::test]
async fn finish_tool_is_persisted_for_governance_instead_of_completing_the_run() {
    let path = temp_db("finish");
    let loop_engine = agent([Ok(ModelResponse::tool(
        "finish-call-1",
        "finish",
        r#"{"summary":"candidate completion"}"#,
    ))]);
    let coordinator = coordinator(loop_engine, SqliteRunStore::open(&path).unwrap());

    let outcome = coordinator
        .start_new_run(input("run-finish"), CancellationToken::new())
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        CoordinatorOutcome::Action {
            action: orchester_protokoll::AgentAction::Finish { .. },
            ref policy,
            ..
        } if policy.decision == PolicyDecision::Allow
            && policy.rule_id == "run.finish"
    ));
    drop(coordinator);

    let store = SqliteRunStore::open(&path).unwrap();
    let run_id = RunId::from("run-finish");
    let events = store.events_owned(&run_id, "owner-coordinator").unwrap();
    assert!(events
        .iter()
        .all(|event| event.kind_name() != "run.completed"));
    assert!(events
        .iter()
        .all(|event| event.kind_name() != "tool.started"));
    assert_eq!(events.last().unwrap().kind_name(), "policy.decided");
    let resume = store
        .resume_point_owned(&run_id, "owner-coordinator", "project-run-finish")
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::PrepareExecution { .. }));
    remove_temp_db(&path);
}

async fn continue_after_file_tool(label: &str, create_fixture: bool) -> GovernedToolOutcome {
    let path = temp_db(label);
    let workspace = path.parent().unwrap().join("workspace");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    if create_fixture {
        std::fs::write(workspace.join("src/lib.rs"), "pub const VALUE: u8 = 7;\n").unwrap();
    }

    let mut first_response = ModelResponse::tool(
        "provider-call-1",
        "read_file",
        r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
    );
    first_response.usage = ModelUsage {
        input_tokens: 3,
        output_tokens: 5,
    };
    let second_response = ModelResponse {
        assistant_text: "inspection complete".into(),
        tool_call: None,
        usage: ModelUsage {
            input_tokens: 7,
            output_tokens: 11,
        },
        opaque_items: Vec::new(),
    };
    let store =
        Arc::new(SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).expect("store"));
    let coordinator = DurableCoordinator::with_clock(
        agent([Ok(first_response)]),
        store.clone(),
        FixedCoordinatorClock::new("2026-07-18T00:00:01Z"),
    );
    let first = coordinator
        .start_new_run(input(label), CancellationToken::new())
        .await
        .expect("first model step");
    let CoordinatorOutcome::Action {
        action_id, call_id, ..
    } = first
    else {
        panic!("expected file action");
    };
    let audit = Arc::new(JsonlAuditSink::open(path.parent().unwrap().join("audit.jsonl")).unwrap());
    let execution = GovernedExecution::with_clock(
        store.clone(),
        audit,
        ToolExecutor::new(&workspace, FileToolLimits::default()).unwrap(),
        "owner-coordinator",
        FixedCoordinatorClock::new("2026-07-18T00:00:02Z"),
    )
    .unwrap();
    let tool_outcome = execution
        .execute(&RunId::from(label), &action_id, &call_id)
        .expect("governed tool outcome");
    assert_eq!(coordinator.model().call_count(), 1);
    drop(execution);
    drop(coordinator);
    drop(store);

    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).expect("reopen store"),
    );
    let coordinator = DurableCoordinator::with_clock(
        agent([Ok(second_response)]),
        store.clone(),
        FixedCoordinatorClock::new("2026-07-18T00:00:03Z"),
    );

    let continued = coordinator
        .continue_run(
            CoordinatorContinuationInput {
                run_id: RunId::from(label),
                owner_actor_id: "owner-coordinator".into(),
                step_id: StepId::from("step-2"),
                model_call_id: CallId::from("model-call-2"),
                action_id: ActionId::from("action-2"),
            },
            CancellationToken::new(),
        )
        .await
        .expect("continued model step");
    assert!(matches!(
        continued,
        CoordinatorOutcome::Text {
            ref text,
            model_calls: 2,
            usage: ModelUsage {
                input_tokens: 10,
                output_tokens: 16,
            },
        } if text == "inspection complete"
    ));
    assert_eq!(coordinator.model().call_count(), 1);
    let summaries = coordinator.model().request_summaries();
    assert_eq!(summaries[0].tool_call_item_count, 1);
    assert_eq!(summaries[0].tool_result_item_count, 1);

    let snapshot = store
        .load_run_owned(&RunId::from(label), "owner-coordinator")
        .unwrap();
    assert_eq!(snapshot.steps_used, 2);
    assert_eq!(snapshot.input_tokens_used, 10);
    assert_eq!(snapshot.output_tokens_used, 16);
    let events = store
        .events_owned(&RunId::from(label), "owner-coordinator")
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.kind,
                orchester_protokoll::HarnessEventKind::RunCreated
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.kind,
                orchester_protokoll::HarnessEventKind::StepStarted
            ))
            .count(),
        2
    );
    let turn_ids = events
        .iter()
        .filter_map(|event| event.turn_id.as_ref())
        .collect::<Vec<_>>();
    assert!(turn_ids.windows(2).all(|pair| pair[0] == pair[1]));
    drop(coordinator);
    drop(store);
    remove_temp_db(&path);
    tool_outcome
}

#[tokio::test]
async fn completed_tool_observation_continues_the_same_durable_run() {
    assert!(matches!(
        continue_after_file_tool("run-continue-completed", true).await,
        GovernedToolOutcome::Completed(_)
    ));
}

#[tokio::test]
async fn failed_tool_observation_continues_the_same_durable_run() {
    assert!(matches!(
        continue_after_file_tool("run-continue-failed", false).await,
        GovernedToolOutcome::Failed(_)
    ));
}

#[tokio::test]
async fn continuation_refuses_an_unobserved_action_before_calling_the_model() {
    let path = temp_db("continuation-before-tool");
    let coordinator = coordinator(
        agent([
            Ok(ModelResponse::tool(
                "provider-call-1",
                "read_file",
                r#"{"path":"src/lib.rs","start_line":null,"end_line":null}"#,
            )),
            Ok(ModelResponse {
                assistant_text: "must remain unused".into(),
                tool_call: None,
                usage: ModelUsage::default(),
                opaque_items: Vec::new(),
            }),
        ]),
        SqliteRunStore::open(&path).unwrap(),
    );
    coordinator
        .start_new_run(
            input("run-continuation-before-tool"),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    let error = coordinator
        .continue_run(
            CoordinatorContinuationInput {
                run_id: RunId::from("run-continuation-before-tool"),
                owner_actor_id: "owner-coordinator".into(),
                step_id: StepId::from("step-2"),
                model_call_id: CallId::from("model-call-2"),
                action_id: ActionId::from("action-2"),
            },
            CancellationToken::new(),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        CoordinatorError::Store(StoreError::Invariant(_))
    ));
    assert_eq!(coordinator.model().call_count(), 1);
    let snapshot = coordinator
        .store()
        .load_run_owned(
            &RunId::from("run-continuation-before-tool"),
            "owner-coordinator",
        )
        .unwrap();
    assert_eq!(snapshot.steps_used, 1);
    drop(coordinator);
    remove_temp_db(&path);
}
