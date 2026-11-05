use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use orchester_laufzeit::harness::config::ConfigLoader;
use orchester_laufzeit::harness::credentials::InMemoryCredentialStore;
use orchester_laufzeit::harness::provider::{
    build_wire_model, build_wire_model_with_transport, HttpRequest, HttpResponse, HttpTransport,
    HttpTransportError, WireModelBuildError,
};
use orchester_modell::{
    LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest, ModelRole,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

const SECRET_CANARY: &str = "sk-configured-responses-secret";

#[derive(Clone, Default)]
struct CaptureTransport {
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}

struct CapturedRequest {
    endpoint: String,
    body: Value,
    authorization: Option<String>,
}

impl CaptureTransport {
    fn request_count(&self) -> usize {
        self.requests.lock().expect("capture lock").len()
    }

    fn first_request(&self) -> (String, Value, Option<String>) {
        let requests = self.requests.lock().expect("capture lock");
        let request = requests.first().expect("captured request");
        (
            request.endpoint.clone(),
            request.body.clone(),
            request.authorization.clone(),
        )
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
                body: serde_json::from_slice(request.body())
                    .map_err(|_| HttpTransportError::InvalidRequest)?,
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
                    "content": [{"type": "output_text", "text": "configured"}]
                }]
            }))
            .expect("fixture JSON"),
        )
    }
}

fn configured_user() -> orchester_laufzeit::harness::config::UserConfig {
    ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "OpenAI",
                "model": "gpt-configured",
                "model_reasoning_effort": "ultra",
                "disable_response_storage": true,
                "service_tier": "default",
                "model_providers": {
                    "OpenAI": {
                        "name": "OpenAI Compatible",
                        "base_url": "http://127.0.0.1:4567/v1",
                        "api_key": "${secret:OpenAI}",
                        "wire_api": "responses"
                    }
                }
            }"#,
        )
        .expect("valid config")
}

fn request(model: &str, store: bool) -> ModelRequest {
    ModelRequest {
        model: model.into(),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            items: vec![ModelItem::Text("prompt".into())],
        }],
        tools: Vec::new(),
        store,
    }
}

#[tokio::test]
async fn builds_a_profile_bound_model_with_resolved_authentication() {
    let config = configured_user();
    let credentials = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let transport = CaptureTransport::default();
    let model = build_wire_model_with_transport(&config, &credentials, transport.clone())
        .expect("configured model");

    assert_eq!(model.profile().provider, "OpenAI");
    assert_eq!(model.profile().provider_name, "OpenAI Compatible");
    assert_eq!(model.profile().model, "gpt-configured");
    assert!(!model.profile().store);
    let response = model
        .complete(request("gpt-configured", false), CancellationToken::new())
        .await
        .expect("configured response");
    assert_eq!(response.assistant_text, "configured");

    let (endpoint, body, authorization) = transport.first_request();
    assert_eq!(endpoint, "http://127.0.0.1:4567/v1/responses");
    assert_eq!(authorization.as_deref(), Some(SECRET_CANARY));
    assert_eq!(body["model"], "gpt-configured");
    assert_eq!(body["reasoning"]["effort"], "ultra");
    assert_eq!(body["service_tier"], "default");
    assert_eq!(body["store"], false);

    let debug = format!("{model:?}");
    assert!(!debug.contains(SECRET_CANARY));
    assert!(!debug.contains("127.0.0.1"));
}

#[tokio::test]
async fn rejects_model_or_storage_drift_before_transport() {
    let config = configured_user();
    let credentials = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let transport = CaptureTransport::default();
    let model = build_wire_model_with_transport(&config, &credentials, transport.clone())
        .expect("configured model");

    for request in [
        request("different-model", false),
        request("gpt-configured", true),
    ] {
        assert_eq!(
            model
                .complete(request, CancellationToken::new())
                .await
                .unwrap_err(),
            ModelError::Protocol
        );
    }
    assert_eq!(transport.request_count(), 0);
}

#[test]
fn required_credentials_fail_before_model_construction_without_leaking() {
    let error = build_wire_model_with_transport(
        &configured_user(),
        &InMemoryCredentialStore::default(),
        CaptureTransport::default(),
    )
    .expect_err("missing credential should fail");
    assert!(matches!(error, WireModelBuildError::Config(_)));
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(SECRET_CANARY));
}

#[test]
fn unauthenticated_profiles_build_without_a_credential() {
    let config = ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "Local",
                "model": "local-model",
                "model_providers": {
                    "Local": {
                        "base_url": "http://localhost:4567/v1",
                        "wire_api": "responses"
                    }
                }
            }"#,
        )
        .expect("valid local config");
    let model = build_wire_model_with_transport(
        &config,
        &InMemoryCredentialStore::default(),
        CaptureTransport::default(),
    )
    .expect("no credential required");
    assert!(!format!("{model:?}").contains("authorization_present: true"));

    let production = build_wire_model(&config, &InMemoryCredentialStore::default())
        .expect("production transport should construct without connecting");
    assert_eq!(production.profile().model, "local-model");
}

#[test]
fn an_explicit_false_keeps_a_keyed_provider_unauthenticated() {
    let config = ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "Local",
                "model": "local-model",
                "model_providers": {
                    "Local": {
                        "base_url": "http://localhost:4567/v1",
                        "wire_api": "responses",
                        "api_key": "${secret:Local}",
                        "requires_openai_auth": false
                    }
                }
            }"#,
        )
        .expect("valid local config");

    let model = build_wire_model_with_transport(
        &config,
        &InMemoryCredentialStore::default(),
        CaptureTransport::default(),
    )
    .expect("explicit false must not resolve a credential");

    assert!(!model.profile().requires_auth);
}

#[tokio::test]
async fn only_the_selected_provider_secret_is_resolved() {
    let config = ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "Router",
                "model": "router-model",
                "model_providers": {
                    "Unused": {
                        "base_url": "https://unused.example/v1",
                        "wire_api": "responses",
                        "api_key": "${secret:Unused}"
                    },
                    "Router": {
                        "base_url": "http://127.0.0.1:4567/v1",
                        "wire_api": "responses",
                        "api_key": "${secret:Router}"
                    }
                }
            }"#,
        )
        .expect("valid selected provider config");
    let credentials = InMemoryCredentialStore::with("Router", SECRET_CANARY);
    let transport = CaptureTransport::default();
    let model = build_wire_model_with_transport(&config, &credentials, transport.clone())
        .expect("only the selected secret is required");

    model
        .complete(request("router-model", true), CancellationToken::new())
        .await
        .expect("captured response");

    assert_eq!(transport.first_request().2.as_deref(), Some(SECRET_CANARY));
}

#[tokio::test]
async fn model_catalog_controls_wire_reasoning_and_output_cap() {
    for (levels, effort, expected) in [
        (
            json!([{"level":"max","api_value":"high"}]),
            "max",
            Some("high"),
        ),
        (json!([{"level":"off","api_value":null}]), "off", None),
        (json!([]), "high", None),
    ] {
        let mut profile:orchester_laufzeit::harness::config::ModelProfileConfig=serde_json::from_value(json!({"model_provider":"OpenAI","model":"gpt-configured","max_output_tokens":8192,"reasoning_levels":levels})).unwrap();
        profile.model_reasoning_effort = Some(effort.into());
        let config = configured_user()
            .with_model_profile_entry("OpenAI.custom", Some(profile))
            .unwrap()
            .with_model_profile("OpenAI.custom")
            .unwrap();
        let transport = CaptureTransport::default();
        let credentials = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
        let model =
            build_wire_model_with_transport(&config, &credentials, transport.clone()).unwrap();
        assert_eq!(
            model.profile().reasoning_effort.as_deref(),
            if effort == "high" { None } else { Some(effort) }
        );
        model
            .complete(request("gpt-configured", false), CancellationToken::new())
            .await
            .unwrap();
        let (_, body, _) = transport.first_request();
        assert_eq!(body["max_output_tokens"], 8192);
        assert_eq!(
            body.get("reasoning").and_then(|r| r["effort"].as_str()),
            expected
        );
    }
}

#[test]
fn explicit_alias_controls_metadata_for_duplicate_model_ids() {
    let mut config = configured_user();
    for (name, cap) in [("OpenAI.first", 1000), ("OpenAI.second", 2000)] {
        let profile = serde_json::from_value(
            json!({"model_provider":"OpenAI","model":"gpt-configured","max_output_tokens":cap}),
        )
        .unwrap();
        config = config
            .with_model_profile_entry(name, Some(profile))
            .unwrap();
    }
    let selected = config
        .with_model_profile("OpenAI.second")
        .unwrap()
        .resolve_model_profile()
        .unwrap();
    assert_eq!(selected.metadata.unwrap().max_output_tokens, Some(2000));
}

#[tokio::test]
async fn raw_default_effort_uses_valid_named_profile_default_after_catalog_edit() {
    let profile=serde_json::from_value(json!({"model_provider":"OpenAI","model":"gpt-configured","model_reasoning_effort":"careful","reasoning_levels":[{"level":"careful","api_value":"high"}]})).unwrap();
    let config = configured_user()
        .with_model_profile_entry("OpenAI.custom", Some(profile))
        .unwrap();
    let transport = CaptureTransport::default();
    let credentials = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let model = build_wire_model_with_transport(&config, &credentials, transport.clone()).unwrap();
    assert_eq!(model.profile().reasoning_effort.as_deref(), Some("careful"));
    model
        .complete(request("gpt-configured", false), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(transport.first_request().1["reasoning"]["effort"], "high");
}
