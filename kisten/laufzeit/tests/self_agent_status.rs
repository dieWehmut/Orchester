use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::config::ConfigLoader;
use orchester_laufzeit::harness::credentials::InMemoryCredentialStore;
use orchester_laufzeit::harness::run_store::{NewRun, RunStore, SqliteRunStore};
use orchester_laufzeit::harness::service::{load_self_agent_status, SelfAgentStatusError};

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

fn temp_workspace(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "orchester-self-status-{label}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("workspace");
    root
}

fn configured() -> orchester_laufzeit::harness::config::UserConfig {
    ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "Loopback",
                "model": "gpt-status",
                "model_reasoning_effort": "high",
                "plan_mode_reasoning_effort": "ultra",
                "disable_response_storage": true,
                "service_tier": "default",
                "model_providers": {
                    "Loopback": {
                        "name": "Local Responses",
                        "base_url": "http://127.0.0.1:4567/v1",
                        "wire_api": "responses",
                        "requires_openai_auth": false
                    }
                },
                "governance": {
                    "approval_reviewer": "user",
                    "tool_network": "ask",
                    "out_of_workspace": "deny",
                    "shell_interpreters": "deny",
                    "approval_ttl_seconds": 900
                },
                "limits": {
                    "max_steps": 12,
                    "max_minutes": 20,
                    "max_same_failure": 2,
                    "max_observation_bytes": 32768
                }
            }"#,
        )
        .expect("config")
}

fn state_database(workspace: &Path) -> PathBuf {
    workspace.join("state/runs.db")
}

#[test]
fn reports_effective_model_governance_and_limits_without_creating_state() {
    let workspace = temp_workspace("configured");
    let state = state_database(&workspace);

    let status = load_self_agent_status(
        &configured(),
        &InMemoryCredentialStore::default(),
        &workspace,
        &state,
        "local-user",
    )
    .expect("status");

    let model = status.model.as_ref().expect("configured model");
    assert_eq!(model.provider, "Loopback");
    assert_eq!(model.provider_name, "Local Responses");
    assert_eq!(model.model, "gpt-status");
    assert_eq!(model.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(model.plan_reasoning_effort.as_deref(), Some("ultra"));
    assert!(!model.store_responses);
    assert_eq!(model.service_tier.as_deref(), Some("default"));
    assert_eq!(status.governance.approval_reviewer, "user");
    assert_eq!(status.governance.approval_ttl_seconds, 900);
    assert_eq!(status.limits.max_steps, 12);
    assert_eq!(status.limits.max_observation_bytes, 32768);
    assert!(!status.durable.database_present);
    assert_eq!(status.durable.resumable_runs, 0);
    assert!(!state.exists(), "status must not create an empty database");
    assert!(!format!("{status:?}").contains(&workspace.display().to_string()));
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn summarizes_real_resumable_runs_for_the_current_workspace() {
    let workspace = temp_workspace("durable");
    let state = state_database(&workspace);
    let config = ConfigLoader::test().load_user("{}").expect("config");
    let empty = load_self_agent_status(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace,
        &state,
        "local-user",
    )
    .expect("empty status");
    let store = SqliteRunStore::open_with_terminal_secrets(&state, Vec::new()).expect("store");
    store
        .create_run(NewRun {
            run_id: "run-status-1".into(),
            project_id: empty.workspace.project_id.clone(),
            owner_actor_id: "local-user".into(),
            canonical_root: empty.workspace.canonical_root.clone(),
            workspace_identity: empty.workspace.workspace_identity.clone(),
            policy_snapshot_hash: "policy-status".into(),
            config_snapshot_hash: "config-status".into(),
            max_steps: 4,
            occurred_at: "2026-07-26T00:00:00Z".into(),
        })
        .expect("run");
    drop(store);

    let status = load_self_agent_status(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace,
        &state,
        "local-user",
    )
    .expect("durable status");

    assert!(status.model.is_none());
    assert!(status.durable.database_present);
    assert_eq!(status.durable.resumable_runs, 1);
    assert_eq!(status.durable.ready_to_continue, 1);
    assert_eq!(status.durable.awaiting_approval, 0);
    assert_eq!(status.durable.reconciliation_required, 0);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn invalid_active_model_configuration_is_not_reported_as_unconfigured() {
    let workspace = temp_workspace("invalid-model");
    let config = ConfigLoader::test()
        .load_user(r#"{"model_provider":"Missing","model":"gpt-status"}"#)
        .expect("syntactically valid config");
    let error = load_self_agent_status(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace,
        state_database(&workspace),
        "local-user",
    )
    .expect_err("invalid active model");

    assert!(matches!(error, SelfAgentStatusError::Config(_)));
    assert!(!state_database(&workspace).exists());
    let _ = std::fs::remove_dir_all(workspace);
}
