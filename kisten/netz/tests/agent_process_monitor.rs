use std::sync::{Arc, RwLock};
use std::time::Duration;

use orchester_netz::{AgentProcessSnapshot, AgentProcessSource, ServerContext, ServerControl};
use orchester_protokoll::{AgentActivityState, AgentFleetStreamFrameDto, AgentWindowCountSource};

#[derive(Clone, Default)]
struct MutableProcessSource {
    names: Arc<RwLock<Vec<&'static str>>>,
}

impl MutableProcessSource {
    fn replace(&self, names: Vec<&'static str>) {
        *self.names.write().expect("process source lock") = names;
    }
}

impl AgentProcessSource for MutableProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        AgentProcessSnapshot::from_process_names(
            self.names
                .read()
                .expect("process source lock")
                .iter()
                .copied(),
        )
    }
}

#[tokio::test(start_paused = true)]
async fn process_monitor_publishes_changes_once_and_stops_with_the_server() {
    let control = ServerControl::new();
    control.start().expect("start server lifecycle");
    let source = MutableProcessSource::default();
    source.replace(vec!["codex.exe", "codex.exe"]);
    let context =
        ServerContext::with_agent_process_source(None, control.clone(), Arc::new(source.clone()));
    let mut receiver = context.agent_status_store().subscribe();

    assert!(context.start_agent_process_monitor());
    assert!(!context.start_agent_process_monitor());
    let frame = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("startup process refresh timeout")
        .expect("startup process snapshot");
    assert!(matches!(
        frame,
        AgentFleetStreamFrameDto::Snapshot { ref snapshot } if snapshot.sequence == 2
    ));

    let snapshot = context.agent_status_store().snapshot().expect("snapshot");
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.agent_id == "codex")
        .expect("codex status");
    assert_eq!(snapshot.sequence, 2);
    assert_eq!(codex.activity, AgentActivityState::Running);
    assert_eq!(codex.active_windows, 2);
    assert_eq!(
        codex.window_count_source,
        AgentWindowCountSource::ExternalProcesses
    );
    source.replace(vec!["codex.exe", "codex.exe", "codex.exe"]);
    tokio::time::advance(Duration::from_secs(2)).await;
    let frame = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("scheduled process refresh timeout")
        .expect("scheduled process snapshot");
    assert!(matches!(
        frame,
        AgentFleetStreamFrameDto::Snapshot { ref snapshot }
            if snapshot.sequence == 3
                && snapshot.agents.iter().any(|agent| {
                    agent.agent_id == "codex" && agent.active_windows == 3
                })
    ));

    assert!(control.request_shutdown().expect("request shutdown"));
    source.replace(vec!["codex.exe", "codex.exe", "codex.exe", "codex.exe"]);
    tokio::time::advance(Duration::from_secs(4)).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), receiver.recv())
            .await
            .is_err(),
        "shutdown monitor published another process snapshot"
    );
    assert_eq!(context.agent_status_store().snapshot().unwrap().sequence, 3);
}
