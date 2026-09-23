use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};

/// A workspace whose configuration names one provider and one profile.
///
/// The selection route validates against this configuration, so a test needs a
/// workspace that really has something to select.
struct ConfiguredWorkspace(PathBuf);

impl ConfiguredWorkspace {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchester-model-{label}-{}",
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
        // The loader requires a user-only home and config, and a fresh temp
        // directory is 0755 under the runner's umask: without this the route
        // answered 503 on ubuntu-24.04 while passing on Windows.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).expect("private home");
            fs::set_permissions(
                home.join("orchester.jsonc"),
                fs::Permissions::from_mode(0o600),
            )
            .expect("private config");
        }
        Self(root)
    }

    fn context(&self) -> ServerContext {
        ServerContext::new(
            Some(OrchesterPaths::new(
                self.0.join("home"),
                self.0.join("workspace"),
            )),
            ServerControl::new(),
        )
    }
}

impl Drop for ConfiguredWorkspace {
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

fn select_request(body: &str) -> Request<Body> {
    Request::put("/api/v1/models/selection")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .expect("selection request")
}

fn catalog_request() -> Request<Body> {
    Request::get("/api/v1/models")
        .body(Body::empty())
        .expect("catalog request")
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
    // The loader requires a user-only home and config, and a fresh temp
    // directory is 0755 under the runner's umask: without this the route
    // answered 503 on ubuntu-24.04 while passing on Windows.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).expect("private home");
        fs::set_permissions(
            home.join("orchester.jsonc"),
            fs::Permissions::from_mode(0o600),
        )
        .expect("private config");
    }
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

#[tokio::test]
async fn selection_route_remembers_the_model_the_next_run_will_use() {
    // The runtime's own selection is session state on a host, and a run gets a
    // fresh host: without the server holding the choice, the model a reader
    // picked would be gone before the run that was meant to use it.
    let workspace = ConfiguredWorkspace::new("remember");
    let context = workspace.context();

    let selected = app_router(context.clone())
        .oneshot(select_request(r#"{"provider":"OpenAI","effort":"low"}"#))
        .await
        .expect("selection response");

    assert_eq!(selected.status(), StatusCode::OK);
    let selected = json_body(selected).await;
    assert_eq!(selected["selected_provider"], "OpenAI");
    assert_eq!(selected["active"]["choice"]["reasoning_effort"], "low");

    // And it is what the catalog reports afterwards, which is what the browser
    // draws: the model the next run will actually use.
    let catalog = app_router(context)
        .oneshot(catalog_request())
        .await
        .expect("catalog response");

    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog = json_body(catalog).await;
    assert_eq!(catalog["selected_provider"], "OpenAI");
    assert_eq!(catalog["active"]["choice"]["provider"], "OpenAI");
    assert_eq!(catalog["active"]["choice"]["reasoning_effort"], "low");
}

#[tokio::test]
async fn selection_route_names_a_profile_whole() {
    let workspace = ConfiguredWorkspace::new("profile");
    let context = workspace.context();

    let response = app_router(context)
        .oneshot(select_request(r#"{"profile":"review"}"#))
        .await
        .expect("selection response");

    assert_eq!(response.status(), StatusCode::OK);
    let catalog = json_body(response).await;
    assert_eq!(catalog["active"]["choice"]["profile"], "review");
    assert_eq!(catalog["active"]["choice"]["model"], "gpt-review");
}

#[tokio::test]
async fn selection_route_refuses_a_choice_this_workspace_cannot_apply() {
    let workspace = ConfiguredWorkspace::new("refuse");
    let context = workspace.context();

    let refused = app_router(context.clone())
        .oneshot(select_request(r#"{"provider":"Nowhere"}"#))
        .await
        .expect("selection response");

    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(refused).await, "validation_failed");

    // Refused means not kept: the next run still uses the configuration's model
    // rather than a choice that failed to apply.
    let catalog = app_router(context)
        .oneshot(catalog_request())
        .await
        .expect("catalog response");
    let catalog = json_body(catalog).await;

    assert_eq!(catalog["selected_provider"], Value::Null);
    assert_eq!(catalog["active"]["choice"]["model"], "gpt-default");
}

#[tokio::test]
async fn selection_route_refuses_a_field_it_cannot_believe() {
    let workspace = ConfiguredWorkspace::new("bounds");
    let response = app_router(workspace.context())
        .oneshot(select_request(r#"{"effort":"\u0000high"}"#))
        .await
        .expect("selection response");

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(response).await, "validation_failed");
}

#[tokio::test]
async fn selection_route_requires_a_selected_workspace() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
        .oneshot(select_request(r#"{"provider":"OpenAI"}"#))
        .await
        .expect("selection response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error_code(response).await, "unavailable");
}
