use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, Request, StatusCode},
    Router,
};
use orchester_anwendung::OrchesterPaths;
use orchester_laufzeit::harness::config::{ConfigLoader, ConfigValue};
use orchester_netz::{app_router, ServerContext, ServerControl};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
use tower::ServiceExt;

struct Workspace(PathBuf);

#[derive(Clone)]
struct TestClient {
    router: Router,
    headers: HeaderMap,
}

impl TestClient {
    async fn new(context: ServerContext) -> Self {
        let router = app_router(context);
        let session = router
            .clone()
            .oneshot(Request::get("/api/v1/session").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(session.status(), StatusCode::OK);
        let cookie = session.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let csrf = json_body(session).await["csrf_token"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:8123".parse().unwrap());
        headers.insert(header::ORIGIN, "http://127.0.0.1:8123".parse().unwrap());
        headers.insert(header::COOKIE, cookie.parse().unwrap());
        headers.insert("x-csrf-token", csrf.parse().unwrap());
        Self { router, headers }
    }

    async fn oneshot(
        self,
        mut request: Request<Body>,
    ) -> Result<axum::response::Response, std::convert::Infallible> {
        request.headers_mut().extend(self.headers);
        self.router.oneshot(request).await
    }
}

impl Workspace {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchester-provider-{:x}",
            u128::from_ne_bytes(random)
        ));
        fs::create_dir_all(root.join("workspace")).unwrap();
        fs::create_dir_all(root.join("home")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root.join("home"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(root)
    }

    fn config(&self) -> PathBuf {
        self.0.join("home/orchester.jsonc")
    }

    async fn router(&self) -> TestClient {
        TestClient::new(ServerContext::new(
            Some(OrchesterPaths::new(
                self.0.join("home"),
                self.0.join("workspace"),
            )),
            ServerControl::new(),
        ))
        .await
    }

    fn write(&self, contents: &str) {
        ConfigLoader::for_user_path(self.config())
            .edit_user_config(&[(
                vec!["model".to_owned()],
                ConfigValue::String("test-model".to_owned()),
            )])
            .unwrap();
        fs::write(self.config(), contents).unwrap();
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft() -> Value {
    // No key is passed: this exercises the existing CLI workflow where a key
    // may have been stored separately, without touching the OS credential store.
    json!({"provider": "relay", "name": "Test relay", "base_url": "https://example.com/v1", "wire_api": "responses", "model": "test-model"})
}

fn request(body: String) -> Request<Body> {
    Request::post("/api/v1/models/providers")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-request-id", "provider-test")
        .body(Body::from(body))
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn adding_a_provider_persists_only_a_reference_and_matches_the_next_catalog() {
    let workspace = Workspace::new();
    let router = workspace.router().await;
    let response = router
        .clone()
        .oneshot(request(draft().to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let saved = json_body(response).await;
    assert_eq!(saved["active"]["state"], "configured");
    assert_eq!(saved["active"]["choice"]["provider"], "relay");
    assert_eq!(saved["active"]["choice"]["model"], "test-model");
    let text = fs::read_to_string(workspace.config()).unwrap();
    assert!(text.contains("${secret:relay}"));
    assert!(!saved.to_string().contains("api_key"));
    assert!(!saved.to_string().contains("secret:"));
    let next = router
        .oneshot(Request::get("/api/v1/models").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(saved, json_body(next).await);
}

#[cfg(windows)]
#[tokio::test]
async fn a_key_is_saved_in_the_windows_credential_store_and_never_returned_or_written_to_config() {
    use orchester_laufzeit::harness::credentials::{CredentialStore, KeyringCredentialStore};
    use secrecy::ExposeSecret;

    struct TestCredential(String);
    impl Drop for TestCredential {
        fn drop(&mut self) {
            let _ = KeyringCredentialStore::new().clear(&self.0);
        }
    }
    let workspace = Workspace::new();
    let mut random = [0; 16];
    getrandom::fill(&mut random).unwrap();
    let credential = TestCredential(format!(
        "orchester-provider-test-{:x}",
        u128::from_ne_bytes(random)
    ));
    let store = KeyringCredentialStore::new();
    assert!(!store.present(&credential.0).unwrap());
    let mut input = draft();
    input["provider"] = json!(credential.0);
    input["api_key"] = json!("fake-provider-route-test-key");
    let response = workspace
        .router()
        .await
        .oneshot(request(input.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let saved = json_body(response).await;
    assert!(!saved.to_string().contains("fake-provider-route-test-key"));
    assert!(!fs::read_to_string(workspace.config())
        .unwrap()
        .contains("fake-provider-route-test-key"));
    assert_eq!(
        store.get(&credential.0).unwrap().unwrap().expose_secret(),
        "fake-provider-route-test-key"
    );
    store.clear(&credential.0).unwrap();
    assert!(!store.present(&credential.0).unwrap());
}

#[tokio::test]
async fn saving_a_provider_keeps_comments_and_clears_the_previous_session_override() {
    let workspace = Workspace::new();
    workspace.write(r#"{
      // retain this comment
      "model_provider": "previous", "model": "old-model",
      "model_providers": {"previous": {"base_url": "https://example.com/v1", "wire_api": "responses"}}
    }"#);
    let router = workspace.router().await;
    let selected = router
        .clone()
        .oneshot(
            Request::put("/api/v1/models/selection")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"provider":"previous","effort":"high"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selected.status(), StatusCode::OK);
    let response = router
        .clone()
        .oneshot(request(draft().to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let saved = json_body(response).await;
    assert_eq!(saved["active"]["choice"]["provider"], "relay");
    assert_eq!(saved["active"]["choice"]["reasoning_effort"], Value::Null);
    assert!(fs::read_to_string(workspace.config())
        .unwrap()
        .contains("// retain this comment"));
    let next = router
        .oneshot(Request::get("/api/v1/models").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(saved, json_body(next).await);
}

#[tokio::test]
async fn duplicate_provider_is_refused_without_replacing_configuration_or_echoing_a_key() {
    let workspace = Workspace::new();
    let router = workspace.router().await;
    assert_eq!(
        router
            .clone()
            .oneshot(request(draft().to_string()))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let before = fs::read(workspace.config()).unwrap();
    let mut duplicate = draft();
    duplicate["api_key"] = json!("private-canary-key");
    let response = router
        .oneshot(request(duplicate.to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = json_body(response).await;
    assert_eq!(error["code"], "conflict");
    assert_eq!(error["request_id"], "provider-test");
    assert!(!error.to_string().contains("private-canary-key"));
    assert_eq!(before, fs::read(workspace.config()).unwrap());
}

#[tokio::test]
async fn invalid_drafts_leave_both_configuration_and_credentials_untouched() {
    let workspace = Workspace::new();
    for (field, value) in [
        ("provider", json!("../outside")),
        ("wire_api", json!("unknown")),
        ("model", json!("")),
        ("base_url", json!("http://external.example/v1")),
        ("api_key", json!("x".repeat(4097))),
    ] {
        let mut invalid = draft();
        invalid["api_key"] = json!("private-canary-key");
        invalid[field] = value;
        let response = workspace
            .router()
            .await
            .oneshot(request(invalid.to_string()))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{field}"
        );
        let error = json_body(response).await;
        assert_eq!(error["code"], "validation_failed");
        assert!(!error.to_string().contains("private-canary-key"));
        assert!(!workspace.config().exists());
    }
}

#[tokio::test]
async fn malformed_unknown_and_oversized_bodies_are_rejected_without_echoing_input() {
    let workspace = Workspace::new();
    let mut unknown = draft();
    unknown["private-canary-key"] = json!(true);
    for body in [
        "{private-canary-key".to_owned(),
        unknown.to_string(),
        "private-canary-key".repeat(3000),
    ] {
        let response = workspace
            .router()
            .await
            .oneshot(request(body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!json_body(response)
            .await
            .to_string()
            .contains("private-canary-key"));
        assert!(!workspace.config().exists());
    }
}

#[tokio::test]
async fn missing_workspace_and_uneditable_configuration_return_retryable_unavailable_errors() {
    let router = TestClient::new(ServerContext::new(None, ServerControl::new())).await;
    let response = router.oneshot(request(draft().to_string())).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json_body(response).await["retryable"], true);
    let workspace = Workspace::new();
    workspace.write("{invalid private-config-canary}");
    let response = workspace
        .router()
        .await
        .oneshot(request(draft().to_string()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!json_body(response)
        .await
        .to_string()
        .contains("private-config-canary"));
}

#[tokio::test]
async fn simultaneous_duplicate_additions_are_serialized() {
    let workspace = Workspace::new();
    let router = workspace.router().await;
    let (first, second) = tokio::join!(
        router.clone().oneshot(request(draft().to_string())),
        router.oneshot(request(draft().to_string()))
    );
    let mut statuses = [
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
}

#[tokio::test]
async fn provider_setup_refuses_untrusted_host_origin_and_missing_session_or_csrf_without_outer_middleware(
) {
    let workspace = Workspace::new();
    let client = workspace.router().await;
    for (header, value, status) in [
        ("host", None, StatusCode::FORBIDDEN),
        ("host", Some("attacker.example:8123"), StatusCode::FORBIDDEN),
        ("origin", None, StatusCode::FORBIDDEN),
        (
            "origin",
            Some("https://attacker.example"),
            StatusCode::FORBIDDEN,
        ),
        ("cookie", None, StatusCode::UNAUTHORIZED),
        ("x-csrf-token", None, StatusCode::FORBIDDEN),
        ("x-csrf-token", Some("incorrect"), StatusCode::FORBIDDEN),
        ("sec-fetch-site", Some("cross-site"), StatusCode::FORBIDDEN),
    ] {
        let mut request = request(draft().to_string());
        request.headers_mut().extend(client.headers.clone());
        if let Some(value) = value {
            request.headers_mut().insert(header, value.parse().unwrap());
        } else {
            request.headers_mut().remove(header);
        }
        let response = client.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), status, "{header}");
        assert!(!workspace.config().exists());
    }
}
