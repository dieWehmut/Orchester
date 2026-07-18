use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orchester_laufzeit::harness::config::ConfigLoader;
use orchester_laufzeit::harness::credentials::{CredentialStore, InMemoryCredentialStore};
use orchester_laufzeit::harness::provider::{
    HttpRequest, HttpResponse, HttpTransport, HttpTransportError,
};
use orchester_laufzeit::harness::service::{
    build_self_agent_service, build_self_agent_service_with_transport, SelfAgentBuildError,
};
use secrecy::SecretString;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const PROVIDER_SECRET: &str = "sk-builder-provider-secret";
const WORKSPACE_SECRET: &str = "workspace-tool-output-secret";
static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn temp_paths(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "orchester-self-builder-{label}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace");
    (workspace, root.join("state").join("runs.db"))
}

fn cleanup(path: &Path) {
    let root = path
        .parent()
        .and_then(Path::parent)
        .expect("temporary root");
    let _ = std::fs::remove_dir_all(root);
}

fn configured_user() -> orchester_laufzeit::harness::config::UserConfig {
    ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "OpenAI",
                "model": "gpt-configured",
                "model_reasoning_effort": "high",
                "disable_response_storage": true,
                "env": { "WORKSPACE_TOKEN": "${secret:Workspace}" },
                "model_providers": {
                    "OpenAI": {
                        "base_url": "http://127.0.0.1:4567/v1",
                        "api_key": "${secret:OpenAI}",
                        "wire_api": "responses",
                        "requires_openai_auth": true
                    }
                },
                "limits": { "max_steps": 5, "max_observation_bytes": 65536 }
            }"#,
        )
        .expect("valid config")
}

fn credentials() -> InMemoryCredentialStore {
    let store = InMemoryCredentialStore::with("OpenAI", PROVIDER_SECRET);
    store
        .set(
            "Workspace",
            SecretString::new(WORKSPACE_SECRET.to_owned().into()),
        )
        .expect("workspace credential");
    store
}

#[derive(Clone, Default)]
struct CaptureTransport {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

struct CapturedRequest {
    endpoint: String,
    authorization: Option<String>,
}

impl CaptureTransport {
    fn request_count(&self) -> usize {
        self.requests.lock().expect("capture lock").len()
    }
}

#[async_trait]
impl HttpTransport for CaptureTransport {
    async fn send(
        &self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        if cancel.is_cancelled() {
            return Err(HttpTransportError::Cancelled);
        }
        self.requests
            .lock()
            .map_err(|_| HttpTransportError::Transport)?
            .push(CapturedRequest {
                endpoint: request.endpoint().as_str().to_owned(),
                authorization: request
                    .authorization()
                    .map(|secret| secret.expose_for_provider().to_owned()),
            });
        HttpResponse::new(
            200,
            None,
            serde_json::to_vec(&json!({
                "status": "completed",
                "output": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type":"output_text", "text":"ready"}]
                }]
            }))
            .expect("fixture"),
        )
    }
}

#[tokio::test]
async fn builds_and_runs_a_configured_durable_service_offline() {
    let (workspace, state_db) = temp_paths("configured");
    let transport = CaptureTransport::default();
    let service = build_self_agent_service_with_transport(
        &configured_user(),
        &credentials(),
        transport.clone(),
        &workspace,
        &state_db,
        "local-user",
    )
    .expect("configured service");

    assert_eq!(service.model().profile().model, "gpt-configured");
    assert!(!service.model().profile().store);
    let turn = service
        .start("say hello", CancellationToken::new())
        .await
        .expect("turn");
    assert_eq!(turn.text(), Some("ready"));
    assert!(state_db.is_file());
    {
        let requests = transport.requests.lock().expect("capture lock");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].endpoint, "http://127.0.0.1:4567/v1/responses");
        assert_eq!(requests[0].authorization.as_deref(), Some(PROVIDER_SECRET));
    }

    let error = service
        .start(format!("echo {WORKSPACE_SECRET}"), CancellationToken::new())
        .await
        .expect_err("configured secret must be rejected before transport");
    assert_eq!(transport.request_count(), 1);
    let rendered = format!("{service:?} {error:?} {error}");
    assert!(!rendered.contains(PROVIDER_SECRET));
    assert!(!rendered.contains(WORKSPACE_SECRET));
    drop(service);
    cleanup(&state_db);
}

#[test]
fn build_failures_do_not_create_durable_state() {
    let (workspace, missing_secret_db) = temp_paths("missing-secret");
    let missing_workspace = InMemoryCredentialStore::with("OpenAI", PROVIDER_SECRET);
    let error = build_self_agent_service_with_transport(
        &configured_user(),
        &missing_workspace,
        CaptureTransport::default(),
        &workspace,
        &missing_secret_db,
        "local-user",
    )
    .expect_err("missing configured secret");
    assert!(matches!(error, SelfAgentBuildError::Config(_)));
    assert!(!missing_secret_db.exists());
    cleanup(&missing_secret_db);

    let (workspace, invalid_limit_db) = temp_paths("invalid-limit");
    let invalid = ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider":"Local",
                "model":"local",
                "model_providers":{"Local":{"base_url":"http://localhost:4567/v1"}},
                "limits":{"max_steps":0}
            }"#,
        )
        .expect("schema-valid config");
    let error = build_self_agent_service_with_transport(
        &invalid,
        &InMemoryCredentialStore::default(),
        CaptureTransport::default(),
        &workspace,
        &invalid_limit_db,
        "local-user",
    )
    .expect_err("invalid loop limit");
    assert!(matches!(error, SelfAgentBuildError::Loop(_)));
    assert!(!invalid_limit_db.exists());
    cleanup(&invalid_limit_db);
}

#[test]
fn production_builder_constructs_without_connecting() {
    let (workspace, state_db) = temp_paths("production");
    let config = ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider":"Local",
                "model":"local",
                "disable_response_storage":true,
                "model_providers":{
                    "Local":{
                        "base_url":"http://localhost:4567/v1",
                        "wire_api":"responses",
                        "requires_openai_auth":false
                    }
                }
            }"#,
        )
        .expect("local config");
    let service = build_self_agent_service(
        &config,
        &InMemoryCredentialStore::default(),
        &workspace,
        &state_db,
        "local-user",
    )
    .expect("production service should construct without network I/O");
    assert_eq!(service.model().profile().provider, "Local");
    assert!(state_db.is_file());
    drop(service);
    cleanup(&state_db);
}
