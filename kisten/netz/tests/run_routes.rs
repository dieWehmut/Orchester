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

async fn start_request(
    context: &ServerContext,
    key: Option<&str>,
    payload: Value,
) -> axum::response::Response {
    let mut request = Request::post("/api/v1/runs")
        .header("content-type", "application/json")
        .header("host", "127.0.0.1:43123")
        .header("x-request-id", "idempotency-route-test");
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    app_router(context.clone())
        .oneshot(request.body(Body::from(payload.to_string())).unwrap())
        .await
        .expect("start response")
}

async fn start_success(context: &ServerContext, key: Option<&str>, payload: Value) -> Value {
    let response = start_request(context, key, payload).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    json_body(response).await
}

async fn snapshot(context: &ServerContext, run_id: &str) -> Value {
    let response = app_router(context.clone())
        .oneshot(
            Request::get(format!("/api/v1/runs/{run_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("snapshot response");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
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
async fn start_route_reuses_the_run_for_an_identical_idempotency_key() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let request = || {
        Request::post("/api/v1/runs")
            .header("content-type", "application/json")
            .header("idempotency-key", "submission-1")
            .body(Body::from(r#"{"prompt":"inspect once"}"#))
            .expect("start request")
    };

    let first = app_router(context.clone())
        .oneshot(request())
        .await
        .expect("first start response");
    assert_eq!(first.status(), StatusCode::OK);
    let first_run_id = json_body(first).await["run_id"]
        .as_str()
        .expect("first run id")
        .to_owned();

    let repeated = app_router(context.clone())
        .oneshot(request())
        .await
        .expect("repeated start response");
    assert_eq!(repeated.status(), StatusCode::OK);
    let repeated_run_id = json_body(repeated).await["run_id"]
        .as_str()
        .expect("repeated run id")
        .to_owned();

    assert_eq!(repeated_run_id, first_run_id);
    let snapshot = app_router(context)
        .oneshot(
            Request::get(format!("/api/v1/runs/{first_run_id}"))
                .body(Body::empty())
                .expect("snapshot request"),
        )
        .await
        .expect("snapshot response");
    let snapshot = json_body(snapshot).await;
    let events = snapshot["events"].as_array().expect("event list");
    assert_eq!(
        events
            .iter()
            .filter(|event| event["kind"]["type"] == "user_message")
            .count(),
        1
    );
}

#[tokio::test]
async fn start_route_rejects_a_reused_key_with_a_different_request() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());

    let first = app_router(context.clone())
        .oneshot(
            Request::post("/api/v1/runs")
                .header("content-type", "application/json")
                .header("idempotency-key", "submission-2")
                .body(Body::from(r#"{"prompt":"first request"}"#))
                .expect("first start request"),
        )
        .await
        .expect("first start response");
    assert_eq!(first.status(), StatusCode::OK);

    let conflict = app_router(context)
        .oneshot(
            Request::post("/api/v1/runs")
                .header("content-type", "application/json")
                .header("idempotency-key", "submission-2")
                .body(Body::from(r#"{"prompt":"different request"}"#))
                .expect("conflicting start request"),
        )
        .await
        .expect("conflict response");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(conflict).await, "conflict");
}

#[tokio::test]
async fn start_route_rejects_an_invalid_idempotency_key() {
    let workspace = TempWorkspace::new();
    let response = app_router(ServerContext::new(
        Some(workspace.paths()),
        ServerControl::new(),
    ))
    .oneshot(
        Request::post("/api/v1/runs")
            .header("content-type", "application/json")
            .header("idempotency-key", " ")
            .body(Body::from(r#"{"prompt":"inspect workspace"}"#))
            .expect("invalid-key request"),
    )
    .await
    .expect("invalid-key response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(response).await, "validation_failed");
}

#[tokio::test]
async fn start_route_compares_the_exact_prompt_and_resume() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let original = serde_json::json!({"prompt": "inspect", "resume": "session-a"});
    let first = start_success(&context, Some("resume-key"), original.clone()).await;
    for changed in [
        serde_json::json!({"prompt": "inspect ", "resume": "session-a"}),
        serde_json::json!({"prompt": "inspect", "resume": "session-b"}),
        serde_json::json!({"prompt": "inspect"}),
    ] {
        let response = start_request(&context, Some("resume-key"), changed).await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let error = json_body(response).await;
        assert_eq!(error["code"], "conflict");
        assert_eq!(error["retryable"], false);
        assert_eq!(error["request_id"], "idempotency-route-test");
    }
    assert_eq!(
        start_success(&context, Some("resume-key"), original).await,
        first
    );
}

#[tokio::test]
async fn start_route_validates_key_boundaries_and_duplicate_headers() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    for key in [
        "".to_owned(),
        " ".to_owned(),
        "bad key".to_owned(),
        "bad\tkey".to_owned(),
        "x".repeat(129),
    ] {
        let response = start_request(
            &context,
            Some(&key),
            serde_json::json!({"prompt": "inspect"}),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "key {key:?}"
        );
        let error = json_body(response).await;
        assert_eq!(error["code"], "validation_failed");
        assert_eq!(error["retryable"], false);
    }
    let mut request = Request::post("/api/v1/runs")
        .header("content-type", "application/json")
        .header("idempotency-key", "duplicate")
        .body(Body::from(r#"{"prompt":"inspect"}"#))
        .unwrap();
    request
        .headers_mut()
        .append("idempotency-key", "duplicate".parse().unwrap());
    let duplicate = app_router(context.clone()).oneshot(request).await.unwrap();
    assert_eq!(duplicate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(duplicate).await, "validation_failed");

    // The maximum length is accepted without truncation, and visible punctuation
    // is part of the opaque key rather than a separator for multiple values.
    for key in [
        "!".to_owned(),
        "x".repeat(128),
        "key,with:punctuation".to_owned(),
    ] {
        start_success(
            &context,
            Some(&key),
            serde_json::json!({"prompt": "inspect"}),
        )
        .await;
    }
}

#[tokio::test]
async fn rejected_payloads_do_not_reserve_the_key() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    for invalid in [
        serde_json::json!({"prompt": ""}),
        serde_json::json!({"prompt": "inspect", "resume": ""}),
        serde_json::json!({"prompt": "inspect", "unexpected": true}),
    ] {
        let response = start_request(&context, Some("valid-after-rejection"), invalid).await;
        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
    }
    let payload = serde_json::json!({"prompt": "accepted after validation failures"});
    let first = start_success(&context, Some("valid-after-rejection"), payload.clone()).await;
    assert_eq!(
        start_success(&context, Some("valid-after-rejection"), payload).await,
        first
    );
}

#[tokio::test]
async fn unkeyed_and_differently_keyed_submissions_remain_independent() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let payload = serde_json::json!({"prompt": "the same prompt"});
    let mut ids = std::collections::HashSet::new();
    for key in [None, None, Some("first-key"), Some("second-key")] {
        let response = start_success(&context, key, payload.clone()).await;
        assert!(ids.insert(response["run_id"].as_str().unwrap().to_owned()));
    }
    let other = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let response = start_success(&other, Some("first-key"), payload).await;
    assert!(ids.insert(response["run_id"].as_str().unwrap().to_owned()));
}

#[tokio::test]
async fn a_retry_after_cancellation_keeps_the_terminal_run() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let payload = serde_json::json!({"prompt": "inspect once"});
    let first = start_success(&context, Some("cancelled-key"), payload.clone()).await;
    let run_id = first["run_id"].as_str().unwrap();
    let cancelled = app_router(context.clone())
        .oneshot(
            Request::post(format!("/api/v1/runs/{run_id}/cancel"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let before = snapshot(&context, run_id).await;
    assert_eq!(before["state"], "cancelled");
    assert_eq!(
        start_success(&context, Some("cancelled-key"), payload).await,
        first
    );
    assert_eq!(snapshot(&context, run_id).await, before);
}

#[tokio::test]
async fn equivalent_json_and_null_resume_share_a_run_and_the_routed_events_url() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let first = start_success(
        &context,
        Some("body-key"),
        serde_json::json!({"prompt": "你好"}),
    )
    .await;
    let repeated = start_success(
        &context,
        Some("body-key"),
        serde_json::json!({"resume": null, "prompt": "你好"}),
    )
    .await;
    assert_eq!(repeated, first);
    assert_eq!(
        first["events_url"],
        format!(
            "ws://127.0.0.1:43123/api/v1/runs/{}/events",
            first["run_id"].as_str().unwrap()
        )
    );
}

#[tokio::test]
async fn an_unconfigured_model_finishes_once_with_a_static_public_error() {
    let workspace = TempWorkspace::new();
    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());
    let key = "private-idempotency-fixture";
    let payload = serde_json::json!({"prompt": "private-prompt-fixture"});
    let first = start_success(&context, Some(key), payload.clone()).await;
    let run_id = first["run_id"].as_str().unwrap();
    let completed = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let current = snapshot(&context, run_id).await;
            if current["state"] == "failed"
                && current["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event["kind"]["type"] == "run_stopped")
            {
                break current;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("an unavailable model must stop, rather than stay running");
    let events = completed["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["kind"]["type"], "user_message");
    assert_eq!(events[1]["kind"]["type"], "error");
    assert_eq!(events[1]["kind"]["code"], "model_unavailable");
    assert_eq!(
        events[1]["kind"]["message"],
        "The selected model is unavailable."
    );
    assert_eq!(events[2]["kind"]["type"], "run_stopped");
    assert_eq!(events[2]["kind"]["reason"], "failed");
    let error_text = events[1]["kind"].to_string();
    for private in [key, "private-prompt-fixture", workspace.0.to_str().unwrap()] {
        assert!(!error_text.contains(private));
    }
    assert_eq!(start_success(&context, Some(key), payload).await, first);
    assert_eq!(snapshot(&context, run_id).await, completed);
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
