use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

use orchester_netz::{app_router, ServerContext, ServerControl};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct EnvironmentRestore {
    previous: Option<std::ffi::OsString>,
}

impl Drop for EnvironmentRestore {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var("ORCHESTER_HOME", previous);
        } else {
            std::env::remove_var("ORCHESTER_HOME");
        }
    }
}

#[tokio::test]
async fn model_catalog_without_workspace_returns_a_typed_unavailable_error() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
        .oneshot(
            Request::get("/api/v1/models")
                .body(Body::empty())
                .expect("model request"),
        )
        .await
        .expect("model response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("model error body");
    let json: Value = serde_json::from_slice(&body).expect("model error JSON");
    assert_eq!(json["code"], "unavailable");
    assert_eq!(json["retryable"], true);
}

#[tokio::test]
async fn model_catalog_route_does_not_echo_configuration_failures() {
    let root = std::env::temp_dir().join(format!(
        "orchester-model-route-invalid-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(root.join(".orchester")).expect("invalid workspace");
    fs::write(root.join(".orchester/project.jsonc"), "{not-json").expect("invalid project config");
    let response = app_router(ServerContext::new(
        Some(orchester_anwendung::OrchesterPaths::new(
            root.join("home"),
            &root,
        )),
        ServerControl::new(),
    ))
    .oneshot(
        Request::get("/api/v1/models")
            .body(Body::empty())
            .expect("model request"),
    )
    .await
    .expect("model response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("model error body");
    let wire = String::from_utf8(body.to_vec()).expect("UTF-8 error");
    assert!(!wire.contains("orchester-model-route-invalid"));
    assert!(!wire.contains("not-json"));
    assert!(!wire.contains("base_url"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn model_catalog_route_projects_a_configured_workspace_model() {
    let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let root = std::env::temp_dir().join(format!(
        "orchester-model-route-valid-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let home = root.join("home");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    fs::create_dir_all(&home).expect("home");
    fs::write(
        home.join("orchester.jsonc"),
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_providers": {
                "OpenAI": {
                    "name": "OpenAI API",
                    "base_url": "https://private.example/v1",
                    "wire_api": "responses"
                }
            },
            "model_profiles": {
                "review": {
                    "model_provider": "OpenAI",
                    "model": "gpt-review"
                }
            }
        }"#,
    )
    .expect("model config");
    let previous = std::env::var_os("ORCHESTER_HOME");
    std::env::set_var("ORCHESTER_HOME", &home);
    let _restore = EnvironmentRestore { previous };

    let response = app_router(ServerContext::new(
        Some(orchester_anwendung::OrchesterPaths::new(&home, &workspace)),
        ServerControl::new(),
    ))
    .oneshot(
        Request::get("/api/v1/models")
            .body(Body::empty())
            .expect("model request"),
    )
    .await
    .expect("model response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("model body");
    let json: Value = serde_json::from_slice(&body).expect("model JSON");
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["active"]["state"], "configured");
    assert_eq!(json["active"]["choice"]["model"], "gpt-default");
    assert_eq!(json["profiles"][0]["profile"], "review");
    let wire = String::from_utf8(body.to_vec()).expect("UTF-8 model response");
    assert!(!wire.contains("private.example"));

    let _ = fs::remove_dir_all(root);
}
