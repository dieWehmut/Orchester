use orchester_netz::{
    agent_status_response, AgentProcessSnapshot, AgentProcessSource, AgentRuntimeStatusError,
    AgentRuntimeStatusStore, AgentRuntimeStatusUpdate, ServerContext, ServerControl,
};
use orchester_protokoll::{AgentActivityState, AgentFleetStreamFrameDto, AgentWindowCountSource};
use orchester_protokoll::{
    AgentAvailabilityState, AgentProcessEnvironment, AgentProcessSourceState,
};
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
fn incomplete_first_observation_is_unknown_and_partial_counts_remain_running() {
    let mut initial = agent_status_response(&registry());
    let codex = initial
        .agents
        .iter_mut()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    codex.availability = AgentAvailabilityState::Available;
    codex.activity = AgentActivityState::Idle;
    let store = AgentRuntimeStatusStore::new(initial).unwrap();
    let mut partial = AgentProcessSnapshot::from_source(
        AgentProcessEnvironment::Windows,
        [] as [&str; 0],
        "2026-11-10T05:00:00Z",
    );
    partial.extend(AgentProcessSnapshot::unavailable(
        AgentProcessEnvironment::Wsl,
    ));
    assert!(store
        .reconcile_external_processes(&partial, "2026-11-10T05:00:01Z")
        .unwrap());
    let snapshot = store.snapshot().unwrap();
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    assert_eq!(codex.activity, AgentActivityState::Unknown);
    assert_eq!(codex.active_windows, 0);
    assert_eq!(
        codex.process_sources.as_ref().unwrap()[1].status,
        AgentProcessSourceState::Unavailable
    );

    let mut partial = AgentProcessSnapshot::from_source(
        AgentProcessEnvironment::Windows,
        ["codex", "codex", "codex"],
        "2026-11-10T05:00:02Z",
    );
    partial.extend(AgentProcessSnapshot::unavailable(
        AgentProcessEnvironment::Wsl,
    ));
    assert!(store
        .reconcile_external_processes(&partial, "2026-11-10T05:00:03Z")
        .unwrap());
    let snapshot = store.snapshot().unwrap();
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    assert_eq!(codex.activity, AgentActivityState::Running);
    assert_eq!(codex.active_windows, 3);
    assert_eq!(codex.process_sources.as_ref().unwrap()[0].count, Some(3));
}

#[test]
fn source_quality_changes_publish_atomically_and_recovery_confirms_zero() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry())).unwrap();
    let successful = AgentProcessSnapshot::from_source(
        AgentProcessEnvironment::Wsl,
        [] as [&str; 0],
        "2026-11-10T05:00:00Z",
    );
    store
        .reconcile_external_processes(&successful, "2026-11-10T05:00:01Z")
        .unwrap();
    let before = store.snapshot().unwrap();
    let mut subscriber = store.subscribe();
    assert!(store
        .reconcile_external_processes(
            &AgentProcessSnapshot::unavailable(AgentProcessEnvironment::Wsl),
            "2026-11-10T05:00:02Z"
        )
        .unwrap());
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.sequence, before.sequence + 1);
    assert_eq!(
        subscriber.try_recv().unwrap(),
        AgentFleetStreamFrameDto::Snapshot {
            snapshot: snapshot.clone()
        }
    );
    assert!(snapshot
        .agents
        .iter()
        .all(|agent| agent.process_sources.as_ref().unwrap()[0].count.is_none()));
    store
        .reconcile_external_processes(&successful, "2026-11-10T05:00:03Z")
        .unwrap();
    let recovered = store.snapshot().unwrap();
    let codex = recovered
        .agents
        .iter()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    assert_ne!(codex.activity, AgentActivityState::Unknown);
    assert_eq!(codex.process_sources.as_ref().unwrap()[0].count, Some(0));
}

#[test]
fn unavailable_external_sources_preserve_managed_approval_and_counts() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry())).unwrap();
    let mut update = running_codex();
    update.activity = AgentActivityState::WaitingApproval;
    store.update(update).unwrap();
    store
        .reconcile_external_processes(
            &AgentProcessSnapshot::unavailable(AgentProcessEnvironment::Wsl),
            "2026-11-10T05:00:01Z",
        )
        .unwrap();
    let snapshot = store.snapshot().unwrap();
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    assert_eq!(codex.activity, AgentActivityState::WaitingApproval);
    assert_eq!(codex.active_windows, 2);
    assert_eq!(codex.active_runs, 2);
    assert_eq!(
        codex.process_sources.as_ref().unwrap()[0].status,
        AgentProcessSourceState::Unavailable
    );
}

#[tokio::test]
async fn failed_process_task_publishes_unavailable_instead_of_retaining_a_confirmed_zero() {
    struct PanickingSource;
    impl AgentProcessSource for PanickingSource {
        fn snapshot(&self) -> AgentProcessSnapshot {
            panic!("owned process-source fixture");
        }
    }
    let context = ServerContext::with_agent_process_source(
        None,
        ServerControl::new(),
        std::sync::Arc::new(PanickingSource),
    );
    let successful = AgentProcessSnapshot::from_source(
        AgentProcessEnvironment::Local,
        [] as [&str; 0],
        "2026-11-10T05:00:00Z",
    );
    context
        .agent_status_store()
        .reconcile_external_processes(&successful, "2026-11-10T05:00:01Z")
        .unwrap();
    let before = context.agent_status_store().snapshot().unwrap().sequence;
    assert_eq!(
        context.refresh_agent_processes().await,
        Err(AgentRuntimeStatusError::ProcessRefreshFailed)
    );
    let failed = context.agent_status_store().snapshot().unwrap();
    assert_eq!(failed.sequence, before + 1);
    assert!(failed
        .agents
        .iter()
        .all(|agent| agent.activity == AgentActivityState::Unknown));
    assert!(failed
        .agents
        .iter()
        .all(|agent| agent
            .process_sources
            .as_ref()
            .unwrap()
            .iter()
            .all(
                |source| source.status == AgentProcessSourceState::Unavailable
                    && source.count.is_none()
            )));
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

#[test]
fn additional_providers_have_monitor_only_rows_without_runnable_adapters() {
    let registry = registry();
    let snapshot = agent_status_response(&registry);
    for provider in ["gemini", "antigravity", "cursor", "deepseek"] {
        assert!(!registry
            .list()
            .iter()
            .any(|capability| capability.name == provider));
        let rows = snapshot
            .agents
            .iter()
            .filter(|agent| agent.provider == provider)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "one always-visible row for {provider}");
        let agent = rows[0];
        assert!(agent
            .capabilities
            .iter()
            .any(|label| label == "monitor_only"));
        assert!(agent
            .capabilities
            .iter()
            .any(|label| label == "process_presence"));
        assert_eq!(agent.activity, AgentActivityState::Unknown);
        assert!(!agent.installed && !agent.configured && !agent.authenticated);
        assert!(agent.last_error.is_none());
    }
}

#[test]
fn monitor_only_rows_observe_presence_absence_and_unknown_sources() {
    let store = AgentRuntimeStatusStore::new(agent_status_response(&registry())).unwrap();
    store
        .reconcile_external_processes(
            &AgentProcessSnapshot::from_process_names(["deepseek"]),
            "2026-11-10T05:00:01Z",
        )
        .unwrap();
    let snapshot = store.snapshot().unwrap();
    for agent in snapshot.agents.iter().filter(|agent| {
        agent
            .capabilities
            .iter()
            .any(|label| label == "monitor_only")
    }) {
        assert_eq!(
            agent.activity,
            if agent.provider == "deepseek" {
                AgentActivityState::Running
            } else {
                AgentActivityState::Offline
            }
        );
        assert_eq!(
            agent.active_windows,
            u64::from(agent.provider == "deepseek")
        );
        assert_eq!(agent.active_runs, 0);
    }
    store
        .reconcile_external_processes(
            &AgentProcessSnapshot::unavailable(AgentProcessEnvironment::Local),
            "2026-11-10T05:00:02Z",
        )
        .unwrap();
    for agent in store.snapshot().unwrap().agents.iter().filter(|agent| {
        agent
            .capabilities
            .iter()
            .any(|label| label == "monitor_only")
    }) {
        assert_eq!(agent.activity, AgentActivityState::Unknown);
        assert_eq!(agent.process_sources.as_ref().unwrap()[0].count, None);
    }
}
