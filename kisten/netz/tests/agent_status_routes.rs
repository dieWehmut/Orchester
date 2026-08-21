use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_netz::{
    app_router, AgentProcessSnapshot, AgentProcessSource, AgentRuntimeStatusUpdate, ServerContext,
    ServerControl,
};
use orchester_protokoll::{
    AgentActivityState, AgentWindowCountSource, AGENT_STATUS_SCHEMA_VERSION,
};

#[derive(Clone)]
struct FixedProcessSource(AgentProcessSnapshot);

impl AgentProcessSource for FixedProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        self.0.clone()
    }
}

fn context_with_processes(names: impl IntoIterator<Item = &'static str>) -> ServerContext {
    ServerContext::with_agent_process_source(
        None,
        ServerControl::new(),
        Arc::new(FixedProcessSource(
            AgentProcessSnapshot::from_process_names(names),
        )),
    )
}

#[tokio::test]
async fn agent_status_route_returns_a_redaction_safe_runtime_snapshot() {
    let response = app_router(context_with_processes([]))
        .oneshot(
            Request::get("/api/v1/agents/status")
                .body(Body::empty())
                .expect("agent status request"),
        )
        .await
        .expect("agent status response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    assert!(response.headers().get("x-request-id").is_some());

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("agent status body");
    let json: Value = serde_json::from_slice(&body).expect("agent status JSON");
    assert_eq!(json["schema_version"], AGENT_STATUS_SCHEMA_VERSION);
    assert_eq!(json["sequence"], 1);
    assert!(json["generated_at"].as_str().unwrap().ends_with('Z'));

    let agents = json["agents"].as_array().expect("agent array");
    let ids: Vec<_> = agents
        .iter()
        .map(|agent| agent["agent_id"].as_str().expect("agent id"))
        .collect();
    assert_eq!(ids, vec!["claude", "codex", "mock", "opencode"]);

    let mock = &agents[2];
    assert_eq!(mock["provider"], "mock");
    assert_eq!(mock["display_name"], "Mock");
    assert_eq!(mock["icon_key"], "mock");
    assert_eq!(mock["availability"], "available");
    assert_eq!(mock["activity"], "idle");
    assert_eq!(mock["installed"], true);
    assert_eq!(mock["configured"], true);
    assert_eq!(mock["authenticated"], true);
    assert_eq!(mock["active_windows"], 0);
    assert_eq!(mock["active_sessions"], 0);
    assert_eq!(mock["active_runs"], 0);
    assert_eq!(mock["active_subagents"], 0);
    assert_eq!(mock["window_count_source"], "managed_sessions");
    assert!(mock["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item == "streaming"));

    let wire = String::from_utf8(body.to_vec()).expect("UTF-8 response");
    assert!(!wire.contains("command"));
    assert!(!wire.contains("PATH"));
    assert!(!wire.contains("\\\\"));
}

#[tokio::test]
async fn agent_status_route_refreshes_external_process_counts_before_responding() {
    let context = context_with_processes(["codex.exe", "CODEX", "codex.exe"]);
    let response = app_router(context.clone())
        .oneshot(
            Request::get("/api/v1/agents/status")
                .body(Body::empty())
                .expect("agent status request"),
        )
        .await
        .expect("agent status response");

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("agent status body");
    let json: Value = serde_json::from_slice(&body).expect("agent status JSON");
    assert_eq!(json["sequence"], 2);
    let codex = json["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent_id"] == "codex")
        .expect("codex status");
    assert_eq!(codex["activity"], "running");
    assert_eq!(codex["active_windows"], 3);
    assert_eq!(codex["window_count_source"], "external_processes");

    context
        .refresh_agent_processes()
        .await
        .expect("repeat process refresh");
    assert_eq!(context.agent_status_store().snapshot().unwrap().sequence, 2);
}

#[tokio::test]
async fn agent_status_route_reflects_runtime_updates_from_the_shared_context() {
    let context = context_with_processes([]);
    context
        .agent_status_store()
        .update(AgentRuntimeStatusUpdate {
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
        })
        .expect("update runtime status");

    let response = app_router(context)
        .oneshot(
            Request::get("/api/v1/agents/status")
                .body(Body::empty())
                .expect("agent status request"),
        )
        .await
        .expect("agent status response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("agent status body");
    let json: Value = serde_json::from_slice(&body).expect("agent status JSON");
    assert_eq!(json["sequence"], 2);
    let codex = json["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["agent_id"] == "codex")
        .expect("codex status");
    assert_eq!(codex["activity"], "running");
    assert_eq!(codex["active_windows"], 2);
    assert_eq!(codex["active_sessions"], 3);
    assert_eq!(codex["active_runs"], 2);
    assert_eq!(codex["active_subagents"], 1);
}
