use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_protokoll::{
    ActionId, AgentAction, CallId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId, TurnId,
    HARNESS_SCHEMA_VERSION,
};

use super::barrier::StartedTool;
use super::executor::{ToolExecution, ToolExecutor, ToolExecutorError};
use super::files::FileToolLimits;

static NEXT_WORKSPACE: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-tool-executor-{label}-{}-{}",
        std::process::id(),
        NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).expect("create workspace");
    root
}

fn started(action: AgentAction) -> StartedTool {
    StartedTool::new(
        HarnessEvent {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id: EventId::from("event-tool-started"),
            run_id: RunId::from("run-tool"),
            turn_id: Some(TurnId::from("turn-tool")),
            step_id: Some(StepId::from("step-tool")),
            call_id: Some(CallId::from("call-tool")),
            sequence: 6,
            occurred_at: "2026-07-18T00:00:00Z".into(),
            kind: HarnessEventKind::ToolStarted {
                action_id: ActionId::from("action-tool"),
            },
        },
        action,
    )
}

#[test]
fn permit_bound_executor_runs_read_list_and_search_actions() {
    let root = temp_workspace("read-only");
    std::fs::write(root.join("src/lib.rs"), "first\nneedle value\nthird\n").expect("write fixture");
    let executor = ToolExecutor::new(&root, FileToolLimits::default()).expect("executor");

    let read = executor
        .execute(started(AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: Some(2),
            end_line: Some(2),
        }))
        .expect("read");
    let ToolExecution::Read(read) = read else {
        panic!("expected read result");
    };
    assert_eq!(read.content, "needle value");

    let listed = executor
        .execute(started(AgentAction::ListFiles {
            path: "src".into(),
            depth: 1,
        }))
        .expect("list");
    let ToolExecution::Listed(listed) = listed else {
        panic!("expected list result");
    };
    assert_eq!(listed.entries.len(), 1);

    let searched = executor
        .execute(started(AgentAction::SearchText {
            path: "src".into(),
            query: "needle".into(),
        }))
        .expect("search");
    let ToolExecution::Searched(searched) = searched else {
        panic!("expected search result");
    };
    assert_eq!(searched.matches.len(), 1);

    let debug = format!("{executor:?} {read:?} {searched:?}");
    assert!(!debug.contains("needle value"));
    assert!(!debug.contains(&root.to_string_lossy().to_string()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn permit_bound_executor_rejects_non_file_actions_without_side_effects() {
    let root = temp_workspace("wrong-action");
    let executor = ToolExecutor::new(&root, FileToolLimits::default()).expect("executor");

    let error = executor
        .execute(started(AgentAction::WriteFile {
            path: "src/generated.rs".into(),
            content: "must not be written".into(),
        }))
        .expect_err("write is not wired into the read-only executor");

    assert!(matches!(error, ToolExecutorError::UnsupportedAction));
    assert!(!root.join("src/generated.rs").exists());
    let _ = std::fs::remove_dir_all(root);
}
