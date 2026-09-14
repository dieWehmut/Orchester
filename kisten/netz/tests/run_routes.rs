use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_netz::{app_router, ServerContext, ServerControl};

fn test_context() -> ServerContext {
    ServerContext::new(None, ServerControl::new())
}

async fn error_code(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("error body");
    serde_json::from_slice::<Value>(&body).expect("error JSON")["code"]
        .as_str()
        .expect("error code")
        .to_owned()
}

#[tokio::test]
async fn start_route_reports_unavailable_without_a_selected_workspace() {
    let response = app_router(test_context())
        .oneshot(
            Request::post("/api/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"prompt":"inspect workspace"}"#))
                .expect("start request"),
        )
        .await
        .expect("start response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error_code(response).await, "unavailable");
}

#[tokio::test]
async fn snapshot_route_reports_not_found_without_a_bound_run() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/runs/run-1")
                .body(Body::empty())
                .expect("snapshot request"),
        )
        .await
        .expect("snapshot response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(error_code(response).await, "not_found");
}

#[tokio::test]
async fn replay_route_reports_not_found_without_a_bound_run() {
    let response = app_router(test_context())
        .oneshot(
            Request::post("/api/v1/runs/run-1/replay")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"after_sequence":0}"#))
                .expect("replay request"),
        )
        .await
        .expect("replay response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(error_code(response).await, "not_found");
}

#[tokio::test]
async fn cancel_route_reports_unavailable_without_a_bound_run() {
    let response = app_router(test_context())
        .oneshot(
            Request::post("/api/v1/runs/run-1/cancel")
                .body(Body::empty())
                .expect("cancel request"),
        )
        .await
        .expect("cancel response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error_code(response).await, "unavailable");
}
