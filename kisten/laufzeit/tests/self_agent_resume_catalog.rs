use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::config::ConfigLoader;
use orchester_laufzeit::harness::credentials::InMemoryCredentialStore;
use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{NewRun, RunStore, SqliteRunStore, Transition};
use orchester_laufzeit::harness::service::{
    load_self_agent_resume_catalog, resolve_self_agent_resume_handle, SelfAgentResumeAvailability,
    SelfAgentResumeStep, SelfAgentResumeTargetError, WorkspaceIdentitySnapshot,
};
use orchester_protokoll::{StepId, StopReason, TurnId};

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-self-resume-catalog-{label}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("workspace");
    root
}

fn state_database(workspace: &Path) -> PathBuf {
    workspace.join("state/runs.db")
}

fn create_run(
    store: &SqliteRunStore,
    workspace: &WorkspaceIdentitySnapshot,
    run_id: &str,
    occurred_at: &str,
) {
    store
        .create_run(NewRun {
            run_id: run_id.into(),
            project_id: workspace.project_id.clone(),
            owner_actor_id: workspace.owner_actor_id.clone(),
            canonical_root: workspace.canonical_root.clone(),
            workspace_identity: workspace.workspace_identity.clone(),
            policy_snapshot_hash: PolicyEngine::snapshot_hash(),
            config_snapshot_hash: "config-resume-catalog".into(),
            max_steps: 4,
            occurred_at: occurred_at.into(),
        })
        .expect("run");
}

#[test]
fn reports_current_workspace_runs_newest_first_through_opaque_handles() {
    let workspace_root = temp_workspace("ordered");
    let state = state_database(&workspace_root);
    let identity =
        WorkspaceIdentitySnapshot::for_workspace(&workspace_root, "local-user").expect("identity");
    let store = SqliteRunStore::open_with_terminal_secrets(&state, Vec::new()).expect("store");
    create_run(
        &store,
        &identity,
        "run-secret-older",
        "2026-07-30T00:00:00Z",
    );
    create_run(
        &store,
        &identity,
        "run-secret-newer",
        "2026-07-31T00:00:00Z",
    );
    drop(store);

    let config = ConfigLoader::test().load_user("{}").expect("config");
    let first = load_self_agent_resume_catalog(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace_root,
        &state,
        "local-user",
    )
    .expect("catalog");
    let second = load_self_agent_resume_catalog(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace_root,
        &state,
        "local-user",
    )
    .expect("catalog reload");

    assert!(first.database_present);
    assert!(!first.truncated);
    assert_eq!(first.entries.len(), 2);
    assert!(first.entries[0].latest);
    assert!(!first.entries[1].latest);
    assert_eq!(
        first.entries[0].availability,
        SelfAgentResumeAvailability::Unsupported
    );
    assert_eq!(first.entries[0].step, SelfAgentResumeStep::StartStep);
    assert_eq!(
        first, second,
        "opaque handles must be stable across reloads"
    );
    assert_ne!(first.entries[0].handle, first.entries[1].handle);
    for entry in &first.entries {
        assert!(entry.handle.starts_with("r-"));
        assert_eq!(entry.handle.len(), 34);
    }
    let rendered = format!("{first:?}");
    assert!(!rendered.contains("run-secret-newer"));
    assert!(!rendered.contains("run-secret-older"));
    assert!(!rendered.contains(&workspace_root.display().to_string()));

    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn missing_state_returns_an_empty_catalog_without_creating_a_database() {
    let workspace_root = temp_workspace("missing");
    let state = state_database(&workspace_root);
    let config = ConfigLoader::test().load_user("{}").expect("config");

    let catalog = load_self_agent_resume_catalog(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace_root,
        &state,
        "local-user",
    )
    .expect("empty catalog");

    assert!(!catalog.database_present);
    assert!(catalog.entries.is_empty());
    assert!(!catalog.truncated);
    assert!(!state.exists(), "catalog lookup must not create state");
    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn terminal_runs_do_not_hide_older_resumable_runs_from_the_bounded_query() {
    let workspace_root = temp_workspace("terminal-filter");
    let state = state_database(&workspace_root);
    let identity =
        WorkspaceIdentitySnapshot::for_workspace(&workspace_root, "local-user").expect("identity");
    let store = SqliteRunStore::open_with_terminal_secrets(&state, Vec::new()).expect("store");
    create_run(
        &store,
        &identity,
        "run-resumable-older",
        "2026-07-29T00:00:00Z",
    );
    create_run(
        &store,
        &identity,
        "run-terminal-newer",
        "2026-07-30T00:00:00Z",
    );
    store
        .append_transition(
            &"run-terminal-newer".into(),
            "local-user",
            Transition::StartStep {
                turn_id: TurnId::from("turn-terminal"),
                step_id: StepId::from("step-terminal"),
                occurred_at: "2026-07-30T00:00:01Z".into(),
            },
        )
        .expect("start terminal run");
    store
        .append_transition(
            &"run-terminal-newer".into(),
            "local-user",
            Transition::Complete {
                reason: StopReason::Succeeded,
                summary: "done".into(),
                occurred_at: "2026-07-30T00:00:02Z".into(),
            },
        )
        .expect("complete terminal run");
    drop(store);

    let catalog = load_self_agent_resume_catalog(
        &ConfigLoader::test().load_user("{}").expect("config"),
        &InMemoryCredentialStore::default(),
        &workspace_root,
        &state,
        "local-user",
    )
    .expect("catalog");

    assert_eq!(catalog.entries.len(), 1);
    assert!(catalog.entries[0].latest);
    assert_eq!(catalog.entries[0].step, SelfAgentResumeStep::StartStep);
    assert!(!format!("{catalog:?}").contains("run-terminal-newer"));
    let _ = std::fs::remove_dir_all(workspace_root);
}

#[test]
fn opaque_resume_handles_are_owner_and_workspace_scoped() {
    let workspace_root = temp_workspace("handle-scope");
    let other_workspace = temp_workspace("handle-other-workspace");
    let state = state_database(&workspace_root);
    let identity =
        WorkspaceIdentitySnapshot::for_workspace(&workspace_root, "local-user").expect("identity");
    let store = SqliteRunStore::open_with_terminal_secrets(&state, Vec::new()).expect("store");
    create_run(
        &store,
        &identity,
        "run-private-scoped",
        "2026-07-31T00:00:00Z",
    );
    let config = ConfigLoader::test().load_user("{}").expect("config");
    let catalog = load_self_agent_resume_catalog(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace_root,
        &state,
        "local-user",
    )
    .expect("catalog");
    let handle = &catalog.entries[0].handle;

    assert_eq!(
        catalog.entries[0].availability,
        SelfAgentResumeAvailability::Unsupported
    );
    assert!(matches!(
        resolve_self_agent_resume_handle(&store, &identity, handle),
        Err(SelfAgentResumeTargetError::Unsupported)
    ));

    let other_owner = WorkspaceIdentitySnapshot::for_workspace(&workspace_root, "other-user")
        .expect("other owner");
    assert!(matches!(
        resolve_self_agent_resume_handle(&store, &other_owner, handle),
        Err(SelfAgentResumeTargetError::Unavailable)
    ));
    let other_identity = WorkspaceIdentitySnapshot::for_workspace(&other_workspace, "local-user")
        .expect("other workspace");
    assert!(matches!(
        resolve_self_agent_resume_handle(&store, &other_identity, handle),
        Err(SelfAgentResumeTargetError::Unavailable)
    ));

    let _ = std::fs::remove_dir_all(workspace_root);
    let _ = std::fs::remove_dir_all(other_workspace);
}
