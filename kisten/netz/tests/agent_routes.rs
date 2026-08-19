use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_netz::{app_router, ServerContext, ServerControl};

#[tokio::test]
async fn agent_catalog_route_returns_sorted_path_free_registry_data() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
        .oneshot(
            Request::get("/api/v1/agents")
                .body(Body::empty())
                .expect("agent catalog request"),
        )
        .await
        .expect("agent catalog response");

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
        .expect("agent catalog body");
    let json: Value = serde_json::from_slice(&body).expect("agent catalog JSON");
    assert_eq!(json["schema_version"], 1);
    let ids: Vec<_> = json["agents"]
        .as_array()
        .expect("agent array")
        .iter()
        .map(|agent| agent["id"].as_str().expect("agent id"))
        .collect();
    assert_eq!(ids, vec!["claude", "codex", "mock", "opencode"]);
    assert_eq!(json["agents"][2]["task_kinds"], serde_json::json!(["chat"]));
    assert_eq!(json["agents"][2]["availability"], "available");

    let wire = String::from_utf8(body.to_vec()).expect("UTF-8 response");
    assert!(!wire.contains("command"));
    assert!(!wire.contains("PATH"));
}
