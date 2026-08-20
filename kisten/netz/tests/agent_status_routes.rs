use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_netz::{app_router, ServerContext, ServerControl};

#[tokio::test]
async fn agent_status_route_returns_a_redaction_safe_runtime_snapshot() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
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
    assert_eq!(json["schema_version"], 1);
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
    assert!(mock["capabilities"].as_array().unwrap().iter().any(|item| item == "streaming"));

    let wire = String::from_utf8(body.to_vec()).expect("UTF-8 response");
    assert!(!wire.contains("command"));
    assert!(!wire.contains("PATH"));
    assert!(!wire.contains("\\\\"));
}
