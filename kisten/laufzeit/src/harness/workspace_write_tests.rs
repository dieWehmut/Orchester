use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_protokoll::{
    ActionId, AgentAction, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
};

use super::barrier::StartedTool;
use super::governance::{WorkspaceGuard, WorkspaceLocks};
use super::workspace_write::{GovernedWorkspaceWriter, WorkspaceWriteError, WorkspaceWriteLimits};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);

fn workspace(name: &str) -> PathBuf {
    let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "orchester-write-tool-{name}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".orchester")).unwrap();
    root
}

fn started(action: AgentAction) -> StartedTool {
    let action_id = ActionId::from("workspace-write-action");
    StartedTool::new(
        HarnessEvent::new_for_test(
            EventId::from("workspace-write-event"),
            RunId::from("workspace-write-run"),
            StepId::from("workspace-write-step"),
            1,
            HarnessEventKind::ToolStarted { action_id },
        ),
        action,
    )
}

fn write(path: &str, content: &str) -> StartedTool {
    started(AgentAction::WriteFile {
        path: path.into(),
        content: content.into(),
    })
}

#[tokio::test]
async fn writes_a_new_file_from_the_durable_started_action() {
    let root = workspace("new");
    let writer = GovernedWorkspaceWriter::new(
        &root,
        WorkspaceWriteLimits::default(),
        WorkspaceLocks::default(),
    )
    .unwrap();

    let result = writer
        .execute(write("src/generated.rs", "pub const VALUE: u8 = 7;\n"))
        .await
        .unwrap();

    assert_eq!(result.bytes_written, 25);
    assert_eq!(
        fs::read_to_string(root.join("src/generated.rs")).unwrap(),
        "pub const VALUE: u8 = 7;\n"
    );
    drop(writer);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn replaces_a_regular_file_atomically_without_temp_artifacts() {
    let root = workspace("replace");
    fs::write(root.join("src/value.txt"), "old").unwrap();
    let writer = GovernedWorkspaceWriter::new(
        &root,
        WorkspaceWriteLimits::default(),
        WorkspaceLocks::default(),
    )
    .unwrap();

    writer
        .execute(write("src/value.txt", "replacement"))
        .await
        .unwrap();

    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "replacement"
    );
    assert!(fs::read_dir(root.join("src")).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".orchester-write-")
    }));
    drop(writer);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rejects_oversized_protected_and_outside_writes_without_mutation() {
    let root = workspace("reject");
    let outside = root
        .parent()
        .unwrap()
        .join(format!("outside-write-tool-{}.txt", std::process::id()));
    let _ = fs::remove_file(&outside);
    let outside_request = format!("../{}", outside.file_name().unwrap().to_string_lossy());
    let writer = GovernedWorkspaceWriter::new(
        &root,
        WorkspaceWriteLimits {
            max_content_bytes: 4,
        },
        WorkspaceLocks::default(),
    )
    .unwrap();

    assert_eq!(
        writer
            .execute(write("src/large.txt", "12345"))
            .await
            .unwrap_err(),
        WorkspaceWriteError::LimitExceeded
    );
    assert!(matches!(
        writer.execute(write(".orchester/state", "x")).await,
        Err(WorkspaceWriteError::Guard(_))
    ));
    assert!(matches!(
        writer.execute(write(&outside_request, "x")).await,
        Err(WorkspaceWriteError::Guard(_))
    ));
    assert!(!root.join("src/large.txt").exists());
    assert!(!root.join(".orchester/state").exists());
    assert!(!outside.exists());
    drop(writer);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn wrong_actions_and_invalid_limits_fail_before_writing() {
    let root = workspace("wrong-action");
    assert!(matches!(
        GovernedWorkspaceWriter::new(
            &root,
            WorkspaceWriteLimits {
                max_content_bytes: u64::MAX,
            },
            WorkspaceLocks::default(),
        ),
        Err(WorkspaceWriteError::InvalidInput)
    ));
    let writer = GovernedWorkspaceWriter::new(
        &root,
        WorkspaceWriteLimits::default(),
        WorkspaceLocks::default(),
    )
    .unwrap();
    let error = writer
        .execute(started(AgentAction::ReadFile {
            path: "src/value.txt".into(),
            start_line: None,
            end_line: None,
        }))
        .await
        .unwrap_err();

    assert_eq!(error, WorkspaceWriteError::WrongAction);
    assert!(!Path::new(&root).join("src/value.txt").exists());
    drop(writer);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn shared_workspace_locks_block_a_second_writer_until_release() {
    let root = workspace("lock");
    let locks = WorkspaceLocks::default();
    let held = locks.mutate(&WorkspaceGuard::new(&root).unwrap()).await;
    let writer =
        GovernedWorkspaceWriter::new(&root, WorkspaceWriteLimits::default(), locks).unwrap();
    let contender = tokio::spawn(async move {
        writer
            .execute(write("src/locked.txt", "after release"))
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(!contender.is_finished());
    drop(held);
    contender.await.unwrap().unwrap();
    assert_eq!(
        fs::read_to_string(root.join("src/locked.txt")).unwrap(),
        "after release"
    );
    fs::remove_dir_all(root).unwrap();
}
