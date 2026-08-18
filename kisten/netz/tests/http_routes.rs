use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};

fn test_context() -> ServerContext {
    ServerContext::new(None, ServerControl::new())
}

#[tokio::test]
async fn health_route_returns_uncached_json_with_the_typed_payload() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("health body");
    let json: Value = serde_json::from_slice(&body).expect("health JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "orchester");
    assert_eq!(json["schema_version"], 1);
}

#[tokio::test]
async fn unknown_api_route_returns_not_found() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/does-not-exist")
                .body(Body::empty())
                .expect("unknown request"),
        )
        .await
        .expect("unknown response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bootstrap_route_returns_the_safe_context_snapshot() {
    let context = ServerContext::new(
        Some(OrchesterPaths::new("private-home", "visible-project")),
        ServerControl::new(),
    );
    let response = app_router(context)
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .expect("bootstrap request"),
        )
        .await
        .expect("bootstrap response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("bootstrap body");
    let json: Value = serde_json::from_slice(&body).expect("bootstrap JSON");
    assert_eq!(
        json["workspace"],
        serde_json::json!({
            "selected": true,
            "name": "visible-project",
        })
    );
    assert!(!json.to_string().contains("private-home"));
}
