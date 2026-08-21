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

async fn wait_for_sequence(context: &ServerContext, expected: u64) {
    for _ in 0..100 {
        if context
            .agent_status_store()
            .snapshot()
            .expect("runtime snapshot")
            .sequence
            >= expected
        {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("agent process monitor did not reach sequence {expected}");
}

#[tokio::test(start_paused = true)]
async fn process_monitor_publishes_changes_once_and_stops_with_the_server() {
    let control = ServerControl::new();
    control.start().expect("start server lifecycle");
    let source = MutableProcessSource::default();
    let context =
        ServerContext::with_agent_process_source(None, control.clone(), Arc::new(source.clone()));
    let mut receiver = context.agent_status_store().subscribe();

    assert!(context.start_agent_process_monitor());
    assert!(!context.start_agent_process_monitor());
    tokio::task::yield_now().await;

    source.replace(vec!["codex.exe", "codex.exe"]);
    tokio::time::advance(Duration::from_secs(2)).await;
    wait_for_sequence(&context, 2).await;

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
    assert!(matches!(
        receiver.try_recv().expect("monitor snapshot"),
        AgentFleetStreamFrameDto::Snapshot { snapshot } if snapshot.sequence == 2
    ));

    tokio::time::advance(Duration::from_secs(2)).await;
    tokio::task::yield_now().await;
    assert_eq!(context.agent_status_store().snapshot().unwrap().sequence, 2);

    assert!(control.request_shutdown().expect("request shutdown"));
    tokio::task::yield_now().await;
    source.replace(vec!["codex.exe", "codex.exe", "codex.exe"]);
    tokio::time::advance(Duration::from_secs(4)).await;
    tokio::task::yield_now().await;
    assert_eq!(context.agent_status_store().snapshot().unwrap().sequence, 2);
}
