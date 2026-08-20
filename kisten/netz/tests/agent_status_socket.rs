use std::net::SocketAddr;

use futures::StreamExt;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use orchester_netz::{app_router, AgentRuntimeStatusUpdate, ServerContext, ServerControl};
use orchester_protokoll::{AgentActivityState, AgentWindowCountSource};

async fn read_json<S>(stream: &mut tokio_tungstenite::WebSocketStream<S>) -> Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    match stream
        .next()
        .await
        .expect("socket frame")
        .expect("socket message")
    {
        Message::Text(text) => serde_json::from_str(&text).expect("JSON frame"),
        other => panic!("expected text frame, got {other:?}"),
    }
}

#[tokio::test]
async fn agent_status_socket_streams_snapshot_updates_and_heartbeats() {
    let context = ServerContext::new(None, ServerControl::new());
    let updates = context.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind socket");
    let address: SocketAddr = listener.local_addr().expect("socket address");
    let server = tokio::spawn(async move {
        axum::serve(listener, app_router(context))
            .await
            .expect("serve test router");
    });

    let (mut socket, _) = connect_async(format!("ws://{address}/api/v1/agents/status/ws"))
        .await
        .expect("connect agent status socket");
    let initial = read_json(&mut socket).await;
    assert_eq!(initial["type"], "snapshot");
    assert_eq!(initial["snapshot"]["sequence"], 1);

    updates
        .agent_status_store()
        .update(runtime_update())
        .expect("update codex");
    let update = read_json(&mut socket).await;
    assert_eq!(update["type"], "snapshot");
    assert_eq!(update["snapshot"]["sequence"], 2);
    let codex = update["snapshot"]["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent_id"] == "codex")
        .expect("codex status");
    assert_eq!(codex["active_windows"], 2);
    assert_eq!(codex["active_subagents"], 1);

    updates
        .agent_status_store()
        .heartbeat("2026-08-20T12:00:02Z")
        .expect("broadcast heartbeat");
    let heartbeat = read_json(&mut socket).await;
    assert_eq!(heartbeat["type"], "heartbeat");
    assert_eq!(heartbeat["sequence"], 2);

    socket.close(None).await.expect("close socket");
    server.abort();
}

fn runtime_update() -> AgentRuntimeStatusUpdate {
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
