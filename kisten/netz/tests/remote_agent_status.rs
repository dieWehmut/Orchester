use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{
    AgentProcessSnapshot, AgentProcessSource, RemoteControl, ServerContext, ServerControl,
};
use orchester_protokoll::{AgentFleetSnapshotDto, AGENT_STATUS_SCHEMA_VERSION};
use serde_json::{json, Value};
use tower::ServiceExt;

#[derive(Clone, Default)]
struct CountingProcessSource(Arc<AtomicUsize>);

impl AgentProcessSource for CountingProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        self.0.fetch_add(1, Ordering::SeqCst);
        AgentProcessSnapshot::from_process_names(["codex.exe", "codex.exe"])
    }
}

fn remote(root: &std::path::Path, source: CountingProcessSource) -> (RemoteControl, ServerContext) {
    let context = ServerContext::with_agent_process_source(
        Some(OrchesterPaths::new(root.join("home"), root)),
        ServerControl::new(),
        Arc::new(source),
    );
    (
        RemoteControl::new(context.clone(), root.to_path_buf()).unwrap(),
        context,
    )
}

async fn request(
    remote: &RemoteControl,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if !token.is_empty() {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = remote
        .router()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
    )
}

async fn pair(remote: &RemoteControl) -> String {
    let (status, _, body) = request(
        remote,
        "POST",
        "/remote/v1/pair",
        "",
        json!({"code":remote.pairing_code(),"device_name":"Status fixture"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn unauthenticated_status_requests_do_not_sample_host_processes() {
    let root = tempfile::tempdir().unwrap();
    let source = CountingProcessSource::default();
    let (remote, _) = remote(root.path(), source.clone());
    for token in ["", "invalid-device-fixture"] {
        let (status, headers, _) = request(
            &remote,
            "GET",
            "/remote/v1/agents/status",
            token,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    }
    assert_eq!(source.0.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn paired_status_returns_the_same_typed_redacted_host_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let source = CountingProcessSource::default();
    let (remote, context) = remote(root.path(), source.clone());
    let token = pair(&remote).await;
    let (status, headers, body) = request(
        &remote,
        "GET",
        "/remote/v1/agents/status",
        &token,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let snapshot: AgentFleetSnapshotDto = serde_json::from_value(body.clone()).unwrap();
    snapshot.validate().unwrap();
    assert_eq!(snapshot.schema_version, AGENT_STATUS_SCHEMA_VERSION);
    assert_eq!(snapshot, context.agent_status_store().snapshot().unwrap());
    let codex = snapshot
        .agents
        .iter()
        .find(|agent| agent.provider == "codex")
        .unwrap();
    assert_eq!(codex.active_windows, 2);
    assert_eq!(codex.process_sources.as_ref().unwrap()[0].count, Some(2));
    for provider in ["qwen", "workbuddy", "workbuddy_global", "openclaw"] {
        let agent = snapshot
            .agents
            .iter()
            .find(|agent| agent.provider == provider)
            .unwrap();
        assert!(agent
            .capabilities
            .iter()
            .any(|capability| capability == "monitor_only"));
        assert_eq!(agent.active_runs, 0);
        assert_eq!(agent.active_subagents, 0);
    }
    let wire = body.to_string();
    for private_key in [
        "\"pid\":",
        "\"argv\":",
        "\"command\":",
        "\"token\":",
        "\"path\":",
    ] {
        assert!(!wire.contains(private_key));
    }
    assert_eq!(source.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn revoked_device_cannot_read_status_or_trigger_another_sample() {
    let root = tempfile::tempdir().unwrap();
    let source = CountingProcessSource::default();
    let (remote, _) = remote(root.path(), source.clone());
    let token = pair(&remote).await;
    assert_eq!(
        request(
            &remote,
            "GET",
            "/remote/v1/agents/status",
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    let device_id = remote.devices()[0].device_id.clone();
    assert!(remote.revoke_device(&device_id).unwrap());
    assert_eq!(
        request(
            &remote,
            "GET",
            "/remote/v1/agents/status",
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(source.0.load(Ordering::SeqCst), 1);
}
