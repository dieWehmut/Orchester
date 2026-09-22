use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};

fn test_context() -> ServerContext {
    ServerContext::new(None, ServerControl::new())
}

/// A workspace the run handler will accept, discarded with the test.
struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("orchester-netz-run-{nonce}"));
        fs::create_dir_all(workspace.join("manifeste")).expect("workspace");
        Self(workspace)
    }

    fn paths(&self) -> OrchesterPaths {
        OrchesterPaths::new(self.0.join("home"), &self.0)
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

async fn json_body(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&body).expect("response JSON")
}

async fn error_code(response: axum::response::Response) -> String {
    json_body(response).await["code"]
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
async fn start_route_journals_the_readers_own_turn_first() {
    // The journal is what the browser replays, and the transcript opens with the
    // reader's bubble because of this event: the runtime never reports the
    // prompt back, since from its side the prompt is the request, not an event.
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());

    let started = app_router(context.clone())
        .oneshot(
            Request::post("/api/v1/runs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"prompt":"inspect the runtime"}"#))
                .expect("start request"),
        )
        .await
        .expect("start response");

    assert_eq!(started.status(), StatusCode::OK);
    let run_id = json_body(started).await["run_id"]
        .as_str()
        .expect("run id")
        .to_owned();

    let snapshot = app_router(context)
        .oneshot(
            Request::get(format!("/api/v1/runs/{run_id}"))
                .body(Body::empty())
                .expect("snapshot request"),
        )
        .await
        .expect("snapshot response");

    let snapshot = json_body(snapshot).await;
    assert_eq!(snapshot["events"][0]["kind"]["type"], "user_message");
    assert_eq!(snapshot["events"][0]["kind"]["text"], "inspect the runtime");
    assert_eq!(snapshot["events"][0]["sequence"], 1);
}

#[tokio::test]
async fn cancel_route_reports_not_found_without_a_bound_run() {
    let response = app_router(test_context())
        .oneshot(
            Request::post("/api/v1/runs/run-1/cancel")
                .body(Body::empty())
                .expect("cancel request"),
        )
        .await
        .expect("cancel response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(error_code(response).await, "not_found");
}
