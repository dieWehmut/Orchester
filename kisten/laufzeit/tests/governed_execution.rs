use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::coordinator::{CoordinatorClock, FixedCoordinatorClock};
use orchester_laufzeit::harness::execution::{
    GovernedExecution, GovernedExecutionError, GovernedToolOutcome,
};
use orchester_laufzeit::harness::executor::ToolExecutor;
use orchester_laufzeit::harness::files::FileToolLimits;
use orchester_laufzeit::harness::run_store::{ResumeNext, RunStore, SqliteRunStore};
use orchester_protokoll::{AgentAction, HarnessEventKind};

#[path = "support/allowed_run.rs"]
#[allow(dead_code)]
mod allowed_run;

use allowed_run::{create_allowed_run, create_run_with_action};

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

struct LeakyClock;

impl CoordinatorClock for LeakyClock {
    fn now(&self) -> String {
        "2026-07-18T00:00:10Z".into()
    }
}

impl std::fmt::Debug for LeakyClock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("clock-secret-must-not-render")
    }
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-governed-execution-{label}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("workspace/src")).expect("create workspace");
    root
}

fn runtime(
    root: &Path,
    store: Arc<SqliteRunStore>,
    audit: Arc<JsonlAuditSink>,
    owner: &str,
) -> GovernedExecution<JsonlAuditSink, FixedCoordinatorClock> {
    GovernedExecution::with_clock(
        store,
        audit,
        ToolExecutor::new(root.join("workspace"), FileToolLimits::default()).expect("executor"),
        owner,
        FixedCoordinatorClock::new("2026-07-18T00:00:10Z"),
    )
    .expect("governed execution")
}

#[test]
fn allowed_read_is_audited_executed_and_persisted_once() {
    let root = temp_root("read");
    std::fs::write(
        root.join("workspace/src/read.rs"),
        "first\ntrusted result\nthird\n",
    )
    .expect("write fixture");
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state.db"), Vec::new())
            .expect("store"),
    );
    let run = create_allowed_run(store.as_ref(), "read");
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = runtime(&root, store.clone(), audit.clone(), &run.owner);

    let outcome = runtime
        .execute(&run.run_id, &run.action_id, &run.provider_call_id)
        .expect("execute");
    let GovernedToolOutcome::Completed(observation) = outcome else {
        panic!("expected completion");
    };
    assert_eq!(observation.kind, "read_file");
    assert_eq!(
        observation.data["content_lines"],
        serde_json::json!(["first", "trusted result", "third"])
    );
    assert!(observation.data.get("content").is_none());
    assert_eq!(audit.verify().expect("verify audit").entries, 1);

    let events = store.events_owned(&run.run_id, &run.owner).expect("events");
    assert!(matches!(
        events[events.len() - 2].kind,
        HarnessEventKind::ToolStarted { .. }
    ));
    assert!(matches!(
        events.last().expect("completion").kind,
        HarnessEventKind::ToolCompleted { .. }
    ));
    let resume = store
        .resume_point_owned(&run.run_id, &run.owner, "project-read")
        .expect("resume")
        .expect("run");
    assert!(matches!(resume.next, ResumeNext::StartNextStep));

    let retry = runtime
        .execute(&run.run_id, &run.action_id, &run.provider_call_id)
        .expect_err("terminal action cannot execute twice");
    assert!(matches!(retry, GovernedExecutionError::NotReady));
    assert_eq!(audit.verify().expect("verify audit").entries, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_read_is_audited_and_persisted_as_bounded_feedback() {
    let root = temp_root("missing");
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state.db"), Vec::new())
            .expect("store"),
    );
    let run = create_allowed_run(store.as_ref(), "missing");
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = runtime(&root, store.clone(), audit.clone(), &run.owner);

    let outcome = runtime
        .execute(&run.run_id, &run.action_id, &run.provider_call_id)
        .expect("recorded tool failure");
    let GovernedToolOutcome::Failed(feedback) = outcome else {
        panic!("expected failure");
    };
    assert_eq!(feedback.classification, "tool_failed");
    assert!(feedback.summary.len() <= 8 * 1024);
    assert_eq!(audit.verify().expect("verify audit").entries, 1);
    let events = store.events_owned(&run.run_id, &run.owner).expect("events");
    assert!(matches!(
        events.last().expect("failure").kind,
        HarnessEventKind::ToolFailed { .. }
    ));
    let resume = store
        .resume_point_owned(&run.run_id, &run.owner, "project-missing")
        .expect("resume")
        .expect("run");
    assert!(matches!(resume.next, ResumeNext::StartNextStep));

    let debug = format!("{runtime:?} {feedback:?}");
    assert!(!debug.contains(&root.to_string_lossy().to_string()));
    assert!(!debug.contains("src/missing.rs"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ask_and_deny_actions_never_cross_the_execution_barrier() {
    let cases = [
        (
            "approval",
            AgentAction::RunCommand {
                program: "curl".into(),
                args: vec!["https://example.test".into()],
                cwd: None,
            },
        ),
        (
            "denied",
            AgentAction::RunCommand {
                program: "rm".into(),
                args: vec!["-rf".into(), "/".into()],
                cwd: None,
            },
        ),
    ];

    for (label, action) in cases {
        let root = temp_root(label);
        let store = Arc::new(
            SqliteRunStore::open_with_terminal_secrets(root.join("state.db"), Vec::new())
                .expect("store"),
        );
        let run = create_run_with_action(store.as_ref(), label, action);
        let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
        let runtime = runtime(&root, store.clone(), audit.clone(), &run.owner);

        let error = runtime
            .execute(&run.run_id, &run.action_id, &run.provider_call_id)
            .expect_err("non-allow action must not execute");
        assert!(matches!(error, GovernedExecutionError::NotReady));
        assert_eq!(audit.verify().expect("verify audit").entries, 0);
        let events = store.events_owned(&run.run_id, &run.owner).expect("events");
        assert!(events
            .iter()
            .all(|event| !matches!(event.kind, HarnessEventKind::ToolStarted { .. })));
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn deterministic_file_rejections_are_not_marked_retryable() {
    let root = temp_root("path-rejection");
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state.db"), Vec::new())
            .expect("store"),
    );
    let run = create_run_with_action(
        store.as_ref(),
        "path-rejection",
        AgentAction::ReadFile {
            path: "../outside.txt".into(),
            start_line: None,
            end_line: None,
        },
    );
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = runtime(&root, store, audit, &run.owner);

    let outcome = runtime
        .execute(&run.run_id, &run.action_id, &run.provider_call_id)
        .expect("recorded tool failure");
    let GovernedToolOutcome::Failed(feedback) = outcome else {
        panic!("expected failure");
    };
    assert!(!feedback.retryable);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_debug_does_not_delegate_to_dependency_debug_output() {
    let root = temp_root("debug");
    let store = Arc::new(
        SqliteRunStore::open_with_terminal_secrets(root.join("state.db"), Vec::new())
            .expect("store"),
    );
    let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).expect("audit"));
    let runtime = GovernedExecution::with_clock(
        store,
        audit,
        ToolExecutor::new(root.join("workspace"), FileToolLimits::default()).expect("executor"),
        "local-user",
        LeakyClock,
    )
    .expect("runtime");

    let debug = format!("{runtime:?}");
    assert!(!debug.contains("clock-secret-must-not-render"));
    let _ = std::fs::remove_dir_all(root);
}
