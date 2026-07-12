use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_protokoll::{
    ActionId, AgentAction, EventId, HarnessEvent, HarnessEventKind, RunId, StepId,
};

use super::barrier::StartedTool;
use super::governance::WorkspaceLocks;
use super::workspace_patch::{GovernedWorkspacePatcher, PatchError, PatchLimits};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);

fn workspace(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "orchester-patch-tool-{name}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join(".orchester")).unwrap();
    for (path, content) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
    root
}

fn started(action: AgentAction) -> StartedTool {
    StartedTool::new(
        HarnessEvent::new_for_test(
            EventId::from("workspace-patch-event"),
            RunId::from("workspace-patch-run"),
            StepId::from("workspace-patch-step"),
            1,
            HarnessEventKind::ToolStarted {
                action_id: ActionId::from("workspace-patch-action"),
            },
        ),
        action,
    )
}

fn patch(text: &str) -> StartedTool {
    started(AgentAction::ApplyPatch { patch: text.into() })
}

fn writer(root: &PathBuf) -> GovernedWorkspacePatcher {
    GovernedWorkspacePatcher::new(root, PatchLimits::default(), WorkspaceLocks::default()).unwrap()
}

#[tokio::test]
async fn applies_update_and_add_hunks_after_one_full_preflight() {
    let root = workspace("apply", &[("src/value.txt", "old\n")]);
    let patch = patch(
        "*** Begin Patch\n*** Update File: src/value.txt\n@@\n-old\n+new\n*** Add File: src/created.txt\n+created\n*** End Patch\n",
    );

    let result = writer(&root).execute(patch).await.unwrap();

    assert_eq!(result.files_changed, 2);
    assert_eq!(result.bytes_written, 12);
    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "new\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("src/created.txt")).unwrap(),
        "created\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn applies_a_hunk_with_an_explicit_source_location() {
    let root = workspace("location", &[("src/value.txt", "first\nold\nlast\n")]);
    let patch = patch(
        "*** Begin Patch\n*** Update File: src/value.txt\n@@ -2,1 +2,1 @@\n-old\n+new\n*** End Patch\n",
    );

    writer(&root).execute(patch).await.unwrap();

    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "first\nnew\nlast\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn defines_trailing_newline_behavior_for_empty_and_last_line_updates() {
    let root = workspace(
        "newline",
        &[("src/empty.txt", ""), ("src/only.txt", "only\n")],
    );
    let patch = patch(
        "*** Begin Patch\n*** Update File: src/empty.txt\n@@\n+added\n*** Update File: src/only.txt\n@@\n-only\n*** End Patch\n",
    );

    writer(&root).execute(patch).await.unwrap();

    assert_eq!(fs::read(root.join("src/empty.txt")).unwrap(), b"added\n");
    assert_eq!(fs::read(root.join("src/only.txt")).unwrap(), b"");
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn validates_every_file_before_mutating_any_file() {
    let root = workspace(
        "cas",
        &[("src/first.txt", "first\n"), ("src/second.txt", "second\n")],
    );
    let patch = patch(
        "*** Begin Patch\n*** Update File: src/first.txt\n@@\n-first\n+changed\n*** Update File: src/second.txt\n@@\n-wrong\n+never\n*** End Patch\n",
    );

    assert_eq!(
        writer(&root).execute(patch).await.unwrap_err(),
        PatchError::CasMismatch
    );
    assert_eq!(
        fs::read_to_string(root.join("src/first.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("src/second.txt")).unwrap(),
        "second\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rejects_ambiguous_hunks_and_unsupported_delete_operations() {
    let root = workspace(
        "strict",
        &[
            ("src/repeated.txt", "same\nsame\n"),
            ("src/delete.txt", "x\n"),
        ],
    );
    let ambiguous = patch(
        "*** Begin Patch\n*** Update File: src/repeated.txt\n@@\n-same\n+changed\n*** End Patch\n",
    );
    assert_eq!(
        writer(&root).execute(ambiguous).await.unwrap_err(),
        PatchError::AmbiguousMatch
    );
    let too_long = patch(
        "*** Begin Patch\n*** Update File: src/delete.txt\n@@\n-x\n-missing\n+never\n*** End Patch\n",
    );
    assert_eq!(
        writer(&root).execute(too_long).await.unwrap_err(),
        PatchError::CasMismatch
    );
    let delete = patch("*** Begin Patch\n*** Delete File: src/delete.txt\n*** End Patch\n");
    assert_eq!(
        writer(&root).execute(delete).await.unwrap_err(),
        PatchError::UnsupportedOperation
    );
    assert!(root.join("src/delete.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn rejects_duplicate_paths_after_lexical_normalization() {
    let root = workspace("duplicate", &[("src/value.txt", "old\n")]);
    let duplicate = patch(
        "*** Begin Patch\n*** Update File: src/value.txt\n@@\n-old\n+first\n*** Update File: other/../src/./value.txt\n@@\n-old\n+second\n*** End Patch\n",
    );

    assert_eq!(
        writer(&root).execute(duplicate).await.unwrap_err(),
        PatchError::Parse
    );
    assert_eq!(
        fs::read_to_string(root.join("src/value.txt")).unwrap(),
        "old\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn malformed_outside_protected_and_wrong_actions_fail_closed() {
    let root = workspace("reject", &[("src/value.txt", "value\n")]);
    assert!(matches!(
        GovernedWorkspacePatcher::new(
            &root,
            PatchLimits {
                max_files: 0,
                ..PatchLimits::default()
            },
            WorkspaceLocks::default(),
        ),
        Err(PatchError::InvalidInput)
    ));
    let malformed =
        patch("*** Begin Patch\n*** Update File: src/value.txt\nnot a hunk\n*** End Patch\n");
    assert_eq!(
        writer(&root).execute(malformed).await.unwrap_err(),
        PatchError::Parse
    );
    for path in ["../outside.txt", ".orchester/state"] {
        let action = patch(&format!(
            "*** Begin Patch\n*** Add File: {path}\n+x\n*** End Patch\n"
        ));
        assert!(matches!(
            writer(&root).execute(action).await,
            Err(PatchError::Guard(_))
        ));
    }
    assert_eq!(
        writer(&root)
            .execute(started(AgentAction::ReadFile {
                path: "src/value.txt".into(),
                start_line: None,
                end_line: None,
            }))
            .await
            .unwrap_err(),
        PatchError::WrongAction
    );
    fs::remove_dir_all(root).unwrap();
}
