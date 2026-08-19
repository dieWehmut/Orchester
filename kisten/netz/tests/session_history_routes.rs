use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use orchester_anwendung::OrchesterPaths;
use orchester_laufzeit::{SessionRecord, SessionStore};
use orchester_netz::{app_router, ServerContext, ServerControl};
use orchester_protokoll::{Outcome, Usage};
use serde_json::Value;
use tower::ServiceExt;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("orchester-session-routes-{nonce}"));
        fs::create_dir_all(root.join("workspace")).expect("temp workspace");
        Self(root)
    }

    fn paths(&self) -> OrchesterPaths {
        OrchesterPaths::new(self.0.join("home"), self.0.join("workspace"))
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(index: u64, prompt: &str) -> SessionRecord {
    SessionRecord {
        recorded_at_unix: 1_800_000_000 + index,
        agent: "codex".to_owned(),
        session_id: Some(format!("native-{index}")),
        prompt: prompt.to_owned(),
        cwd: PathBuf::from(r"C:\private\workspace"),
        model: Some("gpt-5.6".to_owned()),
        outcome: Outcome::Success,
        final_text: format!("result {index}"),
        usage: Usage::default(),
    }
}

async fn json_response(context: ServerContext, uri: &str) -> (StatusCode, Value) {
    let response = app_router(context)
        .oneshot(
            Request::get(uri)
                .body(Body::empty())
                .expect("session history request"),
        )
        .await
        .expect("session history response");
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("session history body");
    let value = serde_json::from_slice(&body).expect("session history JSON");
    (status, value)
}

#[tokio::test]
async fn session_list_route_pages_newest_first_with_opaque_cursors() {
    let root = TempRoot::new();
    let paths = root.paths();
    let store = SessionStore::new(paths.session_log());
    store.append(&record(1, "first")).unwrap();
    store.append(&record(2, "second")).unwrap();
    store.append(&record(3, "third")).unwrap();

    let (status, first) = json_response(
        ServerContext::new(Some(paths.clone()), ServerControl::new()),
        "/api/v1/sessions?limit=2",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["schema_version"], 1);
    assert_eq!(first["items"][0]["title"], "third");
    assert_eq!(first["items"][1]["title"], "second");
    assert_eq!(first["items"][0]["source"], "delegate");
    assert!(first["items"][0]["id"].as_str().unwrap().starts_with("s-"));
    let cursor = first["next_cursor"].as_str().expect("next cursor");

    let (status, second) = json_response(
        ServerContext::new(Some(paths), ServerControl::new()),
        &format!("/api/v1/sessions?limit=2&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["items"][0]["title"], "first");
    assert!(second["next_cursor"].is_null());
    assert!(!second.to_string().contains("native-"));
    assert!(!second.to_string().contains("private"));
}

#[tokio::test]
async fn session_list_route_returns_typed_errors_for_unavailable_or_invalid_queries() {
    let (status, unavailable) = json_response(
        ServerContext::new(None, ServerControl::new()),
        "/api/v1/sessions",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(unavailable["code"], "unavailable");

    let root = TempRoot::new();
    for uri in [
        "/api/v1/sessions?limit=0",
        "/api/v1/sessions?limit=101",
        "/api/v1/sessions?limit=invalid",
        "/api/v1/sessions?cursor=s-00000000000000000000000000000000",
    ] {
        let (status, invalid) = json_response(
            ServerContext::new(Some(root.paths()), ServerControl::new()),
            uri,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(invalid["code"], "bad_request", "{uri}");
    }
}

#[tokio::test]
async fn session_list_route_redacts_corrupt_history_storage_errors() {
    let root = TempRoot::new();
    let paths = root.paths();
    fs::create_dir_all(paths.home()).expect("history home");
    fs::write(paths.session_log(), r#"{"prompt":"private-broken-record"}"#)
        .expect("corrupt history");

    let (status, error) = json_response(
        ServerContext::new(Some(paths.clone()), ServerControl::new()),
        "/api/v1/sessions",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["code"], "unavailable");
    let wire = error.to_string();
    assert!(!wire.contains("private-broken-record"));
    assert!(!wire.contains(paths.home().to_string_lossy().as_ref()));
}

#[tokio::test]
async fn session_detail_route_returns_the_flat_opaque_history_record() {
    let root = TempRoot::new();
    let paths = root.paths();
    SessionStore::new(paths.session_log())
        .append(&record(7, "inspect the selected workspace"))
        .unwrap();
    let context = ServerContext::new(Some(paths), ServerControl::new());
    let (status, page) = json_response(context.clone(), "/api/v1/sessions").await;
    assert_eq!(status, StatusCode::OK);
    let id = page["items"][0]["id"].as_str().expect("session id");

    let (status, detail) = json_response(context, &format!("/api/v1/sessions/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["schema_version"], 1);
    assert_eq!(detail["id"], id);
    assert_eq!(detail["source"], "delegate");
    assert_eq!(detail["prompt"], "inspect the selected workspace");
    assert_eq!(detail["final_text"], "result 7");
    assert!(detail.get("summary").is_none());
    assert!(detail.get("cwd").is_none());
    assert!(detail.get("session_id").is_none());
    assert!(!detail.to_string().contains("native-7"));
}

#[tokio::test]
async fn session_detail_route_hides_lookup_and_workspace_failures() {
    let root = TempRoot::new();
    for id in [
        "not-a-session",
        "s-00000000000000000000000000000000",
        "s-zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    ] {
        let (status, missing) = json_response(
            ServerContext::new(Some(root.paths()), ServerControl::new()),
            &format!("/api/v1/sessions/{id}"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
        assert_eq!(missing["code"], "not_found", "{id}");
        assert!(!missing.to_string().contains(id), "{id}");
    }

    let (status, unavailable) = json_response(
        ServerContext::new(None, ServerControl::new()),
        "/api/v1/sessions/s-00000000000000000000000000000000",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(unavailable["code"], "unavailable");
}
