use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::feedback::{
    FailureLoopGuard, FeedbackClass, FeedbackEngine, FeedbackInput, FeedbackLimits,
};
use orchester_laufzeit::harness::governance::WorkspaceLocks;
use orchester_laufzeit::harness::mutation::{
    MutationTracker, SnapshotLimits, SnapshotResult, SourceWatchConfig, WorkspaceSnapshotter,
};
use orchester_laufzeit::harness::run_store::{RunSnapshot, RunStatus};
use orchester_laufzeit::harness::validator::{
    can_finish, FinishBlocked, ProcessResult, ValidatorClassification, ValidatorEngine,
    ValidatorSpec, ValidatorSpecError, ValidatorState,
};
use orchester_protokoll::{RunId, StopReason};
use secrecy::SecretString;

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-feedback-{name}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("src")).unwrap();
    root
}

fn watcher(root: &PathBuf) -> WorkspaceSnapshotter {
    WorkspaceSnapshotter::new(
        root,
        SourceWatchConfig {
            includes: vec![PathBuf::from("src")],
            excludes: vec![PathBuf::from("target")],
            limits: SnapshotLimits {
                max_files: 100,
                max_bytes: 1024 * 1024,
            },
        },
    )
    .unwrap()
}

fn run_at_generation(generation: u64) -> RunSnapshot {
    RunSnapshot {
        run_id: RunId("run-feedback".into()),
        project_id: "project".into(),
        owner_actor_id: "owner".into(),
        status: RunStatus::Validating,
        next_sequence: 1,
        current_turn_id: None,
        current_step_id: None,
        mutation_generation: generation,
        policy_snapshot_hash: "policy".into(),
        config_snapshot_hash: "config".into(),
        max_steps: 80,
        steps_used: 1,
        input_tokens_used: 0,
        output_tokens_used: 0,
        row_version: 1,
        stop_reason: None,
    }
}

fn validator() -> ValidatorSpec {
    ValidatorSpec::new(
        "unit",
        "cargo",
        vec!["test".into(), "--workspace".into()],
        true,
    )
    .unwrap()
}

#[test]
fn feedback_is_redacted_before_bounding_and_contains_no_terminal_controls() {
    let engine = FeedbackEngine::new(FeedbackLimits {
        summary_bytes: 48,
        stdout_bytes: 56,
        stderr_bytes: 56,
    })
    .with_secret(SecretString::new(
        "configured-provider-secret".to_owned().into_boxed_str(),
    ));
    let built = engine.build(FeedbackInput {
        source: "validator".into(),
        validator_id: Some("unit".into()),
        exit_code: Some(1),
        class: FeedbackClass::ValidatorFailed,
        summary: "configured-provider-secret and an intentionally long diagnostic summary".into(),
        stdout: "prefix\u{1b}[31m configured-provider-secret configured\u{0}-provider-secret Authorization: Bearer hidden-token suffix".into(),
        stderr: "x".repeat(100),
        retryable: true,
    });

    for text in [
        &built.report.summary,
        &built.report.stdout_tail,
        &built.report.stderr_tail,
    ] {
        assert!(!text.contains("configured-provider-secret"));
        assert!(!text.contains("provider-secret"));
        assert!(!text.contains("hidden-token"));
        assert!(!text.contains('\u{1b}'));
    }
    assert!(built.report.summary.len() <= 48);
    assert!(built.report.stdout_tail.len() <= 56);
    assert!(built.report.stderr_tail.len() <= 56);
    assert!(built.truncated.any());
}

#[test]
fn feedback_identifiers_are_redacted_before_hashing_and_serialization() {
    let engine = FeedbackEngine::default().with_secret(SecretString::new(
        "identifier-secret".to_owned().into_boxed_str(),
    ));
    let built = engine.build(FeedbackInput {
        source: "identifier-secret\u{1b}[31m".into(),
        validator_id: Some("identifier-secret".into()),
        exit_code: Some(1),
        class: FeedbackClass::ValidatorFailed,
        summary: "failure".into(),
        stdout: String::new(),
        stderr: String::new(),
        retryable: true,
    });
    assert!(!built.report.source.contains("identifier-secret"));
    assert!(!built
        .report
        .validator_id
        .as_deref()
        .unwrap_or_default()
        .contains("identifier-secret"));
}

#[test]
fn control_bytes_cannot_split_a_configured_secret_to_evade_redaction() {
    let engine = FeedbackEngine::default().with_secret(SecretString::new(
        "configured-provider-secret".to_owned().into_boxed_str(),
    ));
    let built = engine.build(FeedbackInput {
        source: "tool".into(),
        validator_id: None,
        exit_code: Some(1),
        class: FeedbackClass::ToolFailed,
        summary: String::new(),
        stdout: "configured\u{0}-provider-secret".into(),
        stderr: String::new(),
        retryable: false,
    });
    assert!(!built.report.stdout_tail.contains("configured"));
    assert!(!built.report.stdout_tail.contains("provider-secret"));
}

#[test]
fn provider_token_prefix_is_redacted_without_a_leading_word_boundary() {
    let token = "sk-embedded-provider-secret-123456";
    let built = FeedbackEngine::default().build(FeedbackInput {
        source: "tool".into(),
        validator_id: None,
        exit_code: Some(1),
        class: FeedbackClass::ToolFailed,
        summary: String::new(),
        stdout: format!("diagnostic\u{0}{token}"),
        stderr: String::new(),
        retryable: false,
    });

    assert!(!built.report.stdout_tail.contains(token));
    assert!(built.report.stdout_tail.contains("[REDACTED_TOKEN]"));
}

#[test]
fn volatile_diagnostic_fragments_share_one_stable_fingerprint() {
    let engine = FeedbackEngine::default();
    let report = |summary: &str, stdout: &str| {
        engine
            .build(FeedbackInput {
                source: "validator".into(),
                validator_id: Some("unit".into()),
                exit_code: Some(1),
                class: FeedbackClass::ValidatorFailed,
                summary: summary.into(),
                stdout: stdout.into(),
                stderr: String::new(),
                retryable: true,
            })
            .report
    };

    let first = report(
        "failed at 2026-07-12T12:01:02Z after 18ms",
        "/tmp/run-111/src/lib.rs:19:4 connection on port 43121",
    );
    let second = report(
        "failed at 2026-07-13T08:44:55Z after 942ms",
        "/tmp/run-999/src/lib.rs:87:12 connection on port 59999",
    );
    assert_eq!(first.fingerprint, second.fingerprint);
}

#[test]
fn repeated_failure_and_no_progress_action_guards_are_independent() {
    let mut guard = FailureLoopGuard::new(3).unwrap();
    assert_eq!(guard.record_failure("fingerprint-a"), None);
    assert_eq!(guard.record_failure("fingerprint-a"), None);
    assert_eq!(
        guard.record_failure("fingerprint-a"),
        Some(StopReason::RepeatedFailure)
    );

    guard.record_success(true);
    assert_eq!(guard.record_no_progress_action("action-a"), None);
    assert_eq!(guard.record_no_progress_action("action-a"), None);
    assert_eq!(
        guard.record_no_progress_action("action-a"),
        Some(StopReason::RepeatedFailure)
    );

    guard.record_success(true);
    assert_eq!(guard.failure_count(), 0);
    assert_eq!(guard.action_count(), 0);
}

#[test]
fn indirect_source_change_advances_generation_and_stales_validator_passes() {
    let root = temp_workspace("generation");
    fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 1 }").unwrap();
    let watch = watcher(&root);
    let before = watch.capture().unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 2 }").unwrap();
    let after = watch.capture().unwrap();

    let mut tracker = MutationTracker::new(4);
    let observation = tracker.observe(&before, &after);
    assert!(observation.changed);
    assert_eq!(observation.generation, 5);

    let state = ValidatorState::passed("unit", true, 4);
    let run = run_at_generation(tracker.generation());
    assert_eq!(
        can_finish(&run, &[state]),
        Err(FinishBlocked::ValidationRequired)
    );
    drop(watch);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn excluded_build_tree_does_not_advance_source_snapshot() {
    let root = temp_workspace("excluded");
    fs::write(root.join("src/lib.rs"), "stable").unwrap();
    fs::create_dir_all(root.join("target/debug")).unwrap();
    let watch = watcher(&root);
    let before = watch.capture().unwrap();
    fs::write(root.join("target/debug/generated"), "ignored").unwrap();
    let after = watch.capture().unwrap();
    assert_eq!(before, after);
    drop(watch);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validator_that_mutates_sources_fails_even_with_exit_zero() {
    let root = temp_workspace("mutating-validator");
    fs::write(root.join("src/lib.rs"), "before").unwrap();
    let watch = watcher(&root);
    let before = watch.capture().unwrap();
    fs::write(root.join("src/lib.rs"), "after").unwrap();
    let after = watch.capture().unwrap();

    let mut tracker = MutationTracker::new(4);
    let mut state = ValidatorState::passed("unit", true, 4);
    let evaluation = ValidatorEngine::default().evaluate(
        &validator(),
        &mut state,
        &mut tracker,
        &before,
        &after,
        ProcessResult::exited(0, "ok", ""),
    );
    assert_eq!(
        evaluation.classification,
        ValidatorClassification::MutatedSources
    );
    assert_eq!(
        evaluation.report.classification,
        "validator_mutated_sources"
    );
    assert!(!evaluation.report.retryable);
    assert_eq!(state.last_passed_generation, None);
    assert_eq!(tracker.generation(), 5);
    drop(watch);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn required_validator_must_pass_without_truncation_at_current_generation() {
    let root = temp_workspace("finish-guard");
    fs::write(root.join("src/lib.rs"), "unchanged").unwrap();
    let snapshot = watcher(&root).capture().unwrap();
    let mut tracker = MutationTracker::new(7);
    let mut state = ValidatorState::new("unit", true);
    let engine = ValidatorEngine::new(FeedbackEngine::new(FeedbackLimits {
        summary_bytes: 32,
        stdout_bytes: 16,
        stderr_bytes: 16,
    }));

    let pass = engine.evaluate(
        &validator(),
        &mut state,
        &mut tracker,
        &snapshot,
        &snapshot,
        ProcessResult::exited(0, "ok", ""),
    );
    assert_eq!(pass.classification, ValidatorClassification::Passed);
    assert_eq!(state.last_passed_generation, Some(7));
    assert_eq!(can_finish(&run_at_generation(7), &[state.clone()]), Ok(()));

    let truncated = engine.evaluate(
        &validator(),
        &mut state,
        &mut tracker,
        &snapshot,
        &snapshot,
        ProcessResult::exited(0, "output that is much too long", ""),
    );
    assert_eq!(
        truncated.classification,
        ValidatorClassification::OutputTruncated
    );
    assert!(truncated.truncation.any());
    assert_eq!(state.last_passed_generation, None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_validator_spec_is_classified_without_consuming_a_process_result() {
    let spec = ValidatorSpec {
        id: "unit".into(),
        program: "sh".into(),
        args: vec!["-c".into(), "danger".into()],
        required: true,
        timeout_ms: None,
    };
    let root = temp_workspace("invalid-spec");
    fs::write(root.join("src/lib.rs"), "unchanged").unwrap();
    let snapshot = watcher(&root).capture().unwrap();
    let mut tracker = MutationTracker::new(0);
    let mut state = ValidatorState::new("unit", true);
    let evaluation = ValidatorEngine::default().evaluate(
        &spec,
        &mut state,
        &mut tracker,
        &snapshot,
        &snapshot,
        ProcessResult::exited(0, "must not run", ""),
    );
    assert_eq!(
        evaluation.classification,
        ValidatorClassification::SpawnFailed
    );
    assert_eq!(evaluation.report.classification, "process_spawn_failed");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validator_state_must_match_the_validated_spec() {
    let spec = validator();
    let root = temp_workspace("mismatched-state");
    fs::write(root.join("src/lib.rs"), "unchanged").unwrap();
    let snapshot = watcher(&root).capture().unwrap();
    let mut tracker = MutationTracker::new(3);
    let mut state = ValidatorState::new("different-validator", true);

    let evaluation = ValidatorEngine::default().evaluate(
        &spec,
        &mut state,
        &mut tracker,
        &snapshot,
        &snapshot,
        ProcessResult::exited(0, "ok", ""),
    );

    assert_eq!(
        evaluation.classification,
        ValidatorClassification::SpawnFailed
    );
    assert_eq!(state.last_passed_generation, None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn validator_termination_is_typed_and_specs_never_request_a_shell() {
    assert!(matches!(
        ValidatorSpec::new("bad", "sh", vec!["-c".into(), "cargo test".into()], true),
        Err(ValidatorSpecError::ShellInterpreter)
    ));
    assert!(matches!(
        ValidatorSpec::new("bad", "cargo test", Vec::new(), true),
        Err(ValidatorSpecError::ShellWrapper)
    ));
    assert!(matches!(
        ValidatorSpec::new(
            "bad",
            r"C:\Windows\System32\PowerShell.EXE",
            Vec::new(),
            true
        ),
        Err(ValidatorSpecError::ShellInterpreter)
    ));
    let spec = validator();
    assert_eq!(spec.program, "cargo");
    assert_eq!(spec.args, ["test", "--workspace"]);

    let root = temp_workspace("terminations");
    fs::write(root.join("src/lib.rs"), "unchanged").unwrap();
    let snapshot = watcher(&root).capture().unwrap();
    for (result, expected) in [
        (
            ProcessResult::exited(2, "", "failed"),
            ValidatorClassification::ExitFailure,
        ),
        (
            ProcessResult::cancelled(),
            ValidatorClassification::Cancelled,
        ),
        (
            ProcessResult::timed_out(),
            ValidatorClassification::TimedOut,
        ),
    ] {
        let mut tracker = MutationTracker::new(1);
        let mut state = ValidatorState::new("unit", true);
        let evaluation = ValidatorEngine::default().evaluate(
            &spec,
            &mut state,
            &mut tracker,
            &snapshot,
            &snapshot,
            result,
        );
        assert_eq!(evaluation.classification, expected);
        assert_eq!(state.last_passed_generation, None);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshot_limit_is_typed_and_invalidates_prior_validation() {
    let root = temp_workspace("snapshot-limit");
    fs::write(root.join("src/a.rs"), "a").unwrap();
    fs::write(root.join("src/b.rs"), "b").unwrap();
    let watch = WorkspaceSnapshotter::new(
        &root,
        SourceWatchConfig {
            includes: vec![PathBuf::from("src")],
            excludes: Vec::new(),
            limits: SnapshotLimits {
                max_files: 1,
                max_bytes: 1024,
            },
        },
    )
    .unwrap();
    let limited = watch.capture().unwrap();
    assert!(matches!(
        limited,
        SnapshotResult::LimitExceeded { files: 2, .. }
    ));

    let mut tracker = MutationTracker::new(8);
    let observation = tracker.observe(&limited, &limited);
    assert!(observation.uncertain);
    assert_eq!(tracker.generation(), 9);
    drop(watch);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshot_limit_reports_only_the_bounded_detection_size() {
    let root = temp_workspace("bounded-snapshot-read");
    fs::write(root.join("src/large.rs"), vec![b'x'; 1024]).unwrap();
    let watch = WorkspaceSnapshotter::new(
        &root,
        SourceWatchConfig {
            includes: vec![PathBuf::from("src")],
            excludes: Vec::new(),
            limits: SnapshotLimits {
                max_files: 10,
                max_bytes: 4,
            },
        },
    )
    .unwrap();

    assert_eq!(
        watch.capture().unwrap(),
        SnapshotResult::LimitExceeded { files: 1, bytes: 5 }
    );
    drop(watch);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn snapshot_stops_before_unbounded_directory_depth() {
    let root = temp_workspace("deep-tree");
    let mut current = root.join("src");
    for _ in 0..140 {
        current.push("d");
        fs::create_dir(&current).unwrap();
    }
    fs::write(current.join("leaf.rs"), "deep").unwrap();

    let snapshot = watcher(&root).capture().unwrap();
    assert!(matches!(snapshot, SnapshotResult::LimitExceeded { .. }));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn snapshot_lock_identity_is_derived_from_the_workspace_guard() {
    let root = temp_workspace("snapshot-lock-identity");
    fs::write(root.join("src/lib.rs"), "stable").unwrap();
    let first_watch = watcher(&root);
    let second_watch = watcher(&root.join("."));
    let locks = WorkspaceLocks::default();
    let first = locks.mutate(first_watch.workspace()).await;
    let contender_locks = locks.clone();
    let contender =
        tokio::spawn(async move { second_watch.capture_locked(&contender_locks).await });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(!contender.is_finished());
    drop(first);
    tokio::time::timeout(std::time::Duration::from_secs(1), contender)
        .await
        .expect("snapshot should acquire the derived workspace lock")
        .expect("snapshot task")
        .expect("snapshot capture");
    drop(first_watch);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn snapshot_rejects_source_link_traversal_when_platform_allows_link_creation() {
    let root = temp_workspace("link");
    let outside = root.join("outside.rs");
    fs::write(&outside, "outside").unwrap();
    let link = root.join("src/link.rs");
    let created = create_file_link(&outside, &link);
    if created.is_err() {
        eprintln!("skipping link traversal assertion: platform denied link creation");
        fs::remove_dir_all(root).unwrap();
        return;
    }
    let error = watcher(&root).capture().unwrap_err();
    assert!(matches!(
        error,
        orchester_laufzeit::harness::mutation::SnapshotError::LinkTraversal { .. }
    ));
    fs::remove_dir_all(root).unwrap();

    let broken_root = temp_workspace("broken-link");
    let broken = broken_root.join("src/broken.rs");
    let missing_target = broken_root.join("does-not-exist.rs");
    if create_file_link(&missing_target, &broken).is_err() {
        eprintln!("skipping broken-link assertion: platform denied link creation");
        fs::remove_dir_all(broken_root).unwrap();
        return;
    }
    let error = watcher(&broken_root).capture().unwrap_err();
    assert!(matches!(
        error,
        orchester_laufzeit::harness::mutation::SnapshotError::LinkTraversal { .. }
    ));
    fs::remove_dir_all(broken_root).unwrap();
}

#[cfg(unix)]
fn create_file_link(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_link(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(any(unix, windows)))]
fn create_file_link(_target: &std::path::Path, _link: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "link creation is unsupported",
    ))
}
