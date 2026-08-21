use orchester_netz::{
    agent_status_response, AgentProcessSnapshot, AgentRuntimeStatusError, AgentRuntimeStatusStore,
    AgentRuntimeStatusUpdate,
};
use orchester_protokoll::{AgentActivityState, AgentFleetStreamFrameDto, AgentWindowCountSource};
use orchester_verzeichnis::Registry;

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register_builtins();
    registry
}

fn running_codex() -> AgentRuntimeStatusUpdate {
    AgentRuntimeStatusUpdate {
        agent_id: "codex".to_owned(),
        activity: AgentActivityState::Running,
        active_windows: 2,
        active_sessions: 3,
        active_runs: 2,
        active_subagents: 1,
        window_count_source: AgentWindowCountSource::ManagedSessions,
        last_heartbeat_at: Some("2026-08-20T12:00:00Z".to_owned()),
        last_error: None,
        updated_at: "2026-08-20T12:00:01Z".to_owned(),
    }
}

#[test]
fn runtime_store_updates_one_agent_and_advances_the_snapshot_sequence() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry()))
        .expect("valid initial fleet");

    let sequence = store.update(running_codex()).expect("update codex");
    assert_eq!(sequence, 2);

    let snapshot = store.snapshot().expect("runtime snapshot");
    assert_eq!(snapshot.sequence, 2);
    assert_eq!(snapshot.generated_at, "2026-08-20T12:00:01Z");
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.agent_id == "codex")
        .expect("codex status");
    assert_eq!(codex.activity, AgentActivityState::Running);
    assert_eq!(codex.active_windows, 2);
    assert_eq!(codex.active_sessions, 3);
    assert_eq!(codex.active_runs, 2);
    assert_eq!(codex.active_subagents, 1);
    assert_eq!(
        codex.last_heartbeat_at.as_deref(),
        Some("2026-08-20T12:00:00Z")
    );
}

#[test]
fn runtime_store_rejects_unknown_agents_without_mutating_the_snapshot() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry()))
        .expect("valid initial fleet");
    let mut update = running_codex();
    update.agent_id = "not-registered".to_owned();

    assert_eq!(
        store.update(update),
        Err(AgentRuntimeStatusError::UnknownAgent)
    );
    assert_eq!(store.snapshot().unwrap().sequence, 1);
}

#[test]
fn runtime_store_redacts_failure_paths_before_the_snapshot_leaves_the_server() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry()))
        .expect("valid initial fleet");
    let mut update = running_codex();
    update.activity = AgentActivityState::Error;
    update.last_error = Some(r"failed path=C:\Users\alice\project\transcript.json".to_owned());
    store.update(update).expect("record failure");

    let wire = serde_json::to_string(&store.snapshot().unwrap()).expect("serialize snapshot");
    assert!(!wire.contains(r"C:\\Users"));
    assert!(!wire.contains("alice"));
    assert!(wire.contains("[ROOT]/project/transcript.json"));
}

#[test]
fn runtime_store_broadcasts_snapshots_and_heartbeats_to_subscribers() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry()))
        .expect("valid initial fleet");
    let mut receiver = store.subscribe();

    store.update(running_codex()).expect("record update");
    assert!(matches!(
        receiver.try_recv().expect("snapshot frame"),
        AgentFleetStreamFrameDto::Snapshot { snapshot } if snapshot.sequence == 2
    ));

    store
        .heartbeat("2026-08-20T12:00:02Z")
        .expect("record heartbeat");
    assert!(matches!(
        receiver.try_recv().expect("heartbeat frame"),
        AgentFleetStreamFrameDto::Heartbeat { sequence: 2, .. }
    ));
}

#[test]
fn runtime_store_reconciles_external_processes_without_losing_managed_counts() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry()))
        .expect("valid initial fleet");
    store
        .update(running_codex())
        .expect("managed runtime update");
    let mut receiver = store.subscribe();

    let processes =
        AgentProcessSnapshot::from_process_names(["codex.exe", "codex.exe", "codex.exe"]);
    assert!(store
        .reconcile_external_processes(&processes, "2026-08-22T08:00:00Z")
        .expect("reconcile process snapshot"));

    let snapshot = store.snapshot().expect("runtime snapshot");
    assert_eq!(snapshot.sequence, 3);
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.agent_id == "codex")
        .expect("codex status");
    assert_eq!(codex.activity, AgentActivityState::Running);
    assert_eq!(codex.active_windows, 3);
    assert_eq!(codex.active_sessions, 3);
    assert_eq!(codex.active_runs, 2);
    assert_eq!(codex.active_subagents, 1);
    assert_eq!(
        codex.window_count_source,
        AgentWindowCountSource::ExternalProcesses
    );
    assert!(matches!(
        receiver.try_recv().expect("external process frame"),
        AgentFleetStreamFrameDto::Snapshot { snapshot } if snapshot.sequence == 3
    ));

    assert!(!store
        .reconcile_external_processes(&processes, "2026-08-22T08:00:01Z")
        .expect("ignore unchanged process snapshot"));
    assert_eq!(store.snapshot().unwrap().sequence, 3);
    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    assert!(store
        .reconcile_external_processes(&AgentProcessSnapshot::default(), "2026-08-22T08:00:02Z",)
        .expect("record process exit"));
    let snapshot = store.snapshot().expect("runtime snapshot after exit");
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.agent_id == "codex")
        .expect("codex status");
    assert_eq!(snapshot.sequence, 4);
    assert_eq!(codex.activity, AgentActivityState::Running);
    assert_eq!(codex.active_windows, 0);
    assert_eq!(codex.active_sessions, 3);
    assert_eq!(codex.active_runs, 2);
    assert_eq!(codex.active_subagents, 1);
}

#[test]
fn process_monitor_start_is_safe_without_a_tokio_runtime() {
    let context = orchester_netz::ServerContext::new(None, orchester_netz::ServerControl::new());
    assert!(!context.start_agent_process_monitor());
}
