use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(
    context: &ServerContext,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app_router(context.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header("content-type", "application/json")
                .body(
                    body.map(|v| Body::from(v.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

#[tokio::test]
async fn schedule_crud_persists_and_runs_through_the_real_runtime() {
    let root = std::env::temp_dir().join(format!(
        "schedule-api-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    let paths = OrchesterPaths::new(root.join("home"), &root);
    let context = ServerContext::new(Some(paths.clone()), ServerControl::new());
    let input = json!({"name":"Morning check","prompt":"Review this workspace","enabled":false,"first_run_at":time::OffsetDateTime::now_utc().unix_timestamp()+3600,"interval_seconds":3600});
    let (status, task) = request(&context, "POST", "/schedules", Some(input.clone())).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = task["id"].as_str().unwrap();
    let reloaded = ServerContext::new(Some(paths.clone()), ServerControl::new());
    let (_, tasks) = request(&reloaded, "GET", "/schedules", None).await;
    assert_eq!(tasks["schedules"][0]["name"], "Morning check");
    let mut invalid = input.clone();
    invalid["interval_seconds"] = json!(1);
    assert_eq!(
        request(&context, "PUT", &format!("/schedules/{id}"), Some(invalid))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut updated = input.clone();
    updated["name"] = json!("Updated check");
    assert_eq!(
        request(&context, "PUT", &format!("/schedules/{id}"), Some(updated))
            .await
            .1["name"],
        "Updated check"
    );
    let (status, execution) =
        request(&context, "POST", &format!("/schedules/{id}/runs"), None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let run_id = execution["run_id"].as_str().unwrap();
    // An unconfigured model must produce a terminal failure, never a fake success.
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let (_, response) = request(&context, "GET", "/schedules", None).await;
            let history = &response["schedules"][0]["history"][0];
            if history["status"] == "failed" {
                assert!(!history["finished_at"].is_null());
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let (_, snapshot) = request(&context, "GET", &format!("/runs/{run_id}"), None).await;
    assert_eq!(snapshot["state"], "failed");
    assert_eq!(
        request(&context, "DELETE", &format!("/schedules/{id}"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&context, "DELETE", &format!("/schedules/{id}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        request(&context, "GET", "/schedules", None).await.1["schedules"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    drop((context, reloaded));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn a_due_once_task_is_dispatched_without_an_http_submission() {
    let root = std::env::temp_dir().join(format!(
        "schedule-tick-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    let control = ServerControl::new();
    control.start().unwrap();
    let context = ServerContext::new(
        Some(OrchesterPaths::new(root.join("home"), &root)),
        control.clone(),
    );
    let input = json!({"name":"Due task","prompt":"No external model configured","enabled":true,"first_run_at":time::OffsetDateTime::now_utc().unix_timestamp()-10,"interval_seconds":null});
    assert_eq!(
        request(&context, "POST", "/schedules", Some(input)).await.0,
        StatusCode::CREATED
    );
    assert!(context.start_scheduler());
    assert!(!context.start_scheduler());
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let (_, response) = request(&context, "GET", "/schedules", None).await;
            let task = &response["schedules"][0];
            if task["history"][0]["status"] == "failed" {
                assert_eq!(task["enabled"], false);
                assert_eq!(task["history"].as_array().unwrap().len(), 1);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    control.request_shutdown().unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    drop(context);
    std::fs::remove_dir_all(root).unwrap();
}
