use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use orchester_laufzeit::harness::credentials::{InMemoryCredentialStore, provider_secret};
use orchester_laufzeit::harness::provider::responses::{
    ResponsesLanguageModel, ResponsesModelError, ResponsesRequestOptions,
};
use orchester_laufzeit::harness::provider::{
    HttpRequest, HttpResponse, HttpTransport, HttpTransportError,
};
use orchester_modell::{
    LanguageModel, ModelError, ModelItem, ModelMessage, ModelRequest, ModelRole,
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

const SECRET_CANARY: &str = "sk-responses-model-secret";
const HOST_CANARY: &str = "provider-sensitive.example";
const BODY_CANARY: &str = "provider-body-must-not-leak";

#[derive(Clone, Default)]
struct FakeTransport {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    responses: VecDeque<Result<HttpResponse, HttpTransportError>>,
    requests: Vec<CapturedRequest>,
}

struct CapturedRequest {
    endpoint: String,
    body: Value,
    authorization: Option<String>,
}

impl FakeTransport {
    fn with_responses(
        responses: impl IntoIterator<Item = Result<HttpResponse, HttpTransportError>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                responses: responses.into_iter().collect(),
                requests: Vec::new(),
            })),
        }
    }

    fn requests(&self) -> Vec<(String, Value, Option<String>)> {
        self.state
            .lock()
            .expect("fake lock")
            .requests
            .iter()
            .map(|request| {
                (
                    request.endpoint.clone(),
                    request.body.clone(),
                    request.authorization.clone(),
                )
            })
            .collect()
    }
}

#[async_trait]
impl HttpTransport for FakeTransport {
    async fn send(
        &self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        if cancel.is_cancelled() {
            return Err(HttpTransportError::Cancelled);
        }
        let captured = CapturedRequest {
            endpoint: request.endpoint().as_str().to_owned(),
            body: serde_json::from_slice(request.body())
                .map_err(|_| HttpTransportError::InvalidRequest)?,
            authorization: request
                .authorization()
                .map(|secret| secret.expose_for_provider().to_owned()),
        };
        let mut state = self
            .state
            .lock()
            .map_err(|_| HttpTransportError::Transport)?;
        state.requests.push(captured);
        state
            .responses
            .pop_front()
            .unwrap_or(Err(HttpTransportError::Transport))
    }
}

fn request(prompt: &str) -> ModelRequest {
    ModelRequest {
        model: "gpt-test".into(),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            items: vec![ModelItem::Text(prompt.into())],
        }],
        tools: Vec::new(),
        store: false,
    }
}

fn success(text: &str) -> HttpResponse {
    HttpResponse::new(
        200,
        None,
        serde_json::to_vec(&json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}]
            }],
            "usage": {"input_tokens": 2, "output_tokens": 1}
        }))
        .expect("fixture JSON"),
    )
    .expect("bounded fixture")
}

fn authenticated_model(
    base_url: &str,
    transport: FakeTransport,
) -> ResponsesLanguageModel<FakeTransport> {
    let store = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let secret = provider_secret(&store, "OpenAI")
        .expect("credential lookup")
        .expect("credential present");
    ResponsesLanguageModel::new(
        base_url,
        transport,
        Some(secret),
        ResponsesRequestOptions {
            reasoning_effort: Some("ultra".into()),
            service_tier: Some("default".into()),
        },
    )
    .expect("valid model")
}

#[tokio::test]
async fn performs_reusable_authenticated_responses_calls() {
    let transport = FakeTransport::with_responses([Ok(success("first")), Ok(success("second"))]);
    let model = authenticated_model("https://example.test/v1/", transport.clone());

    let first = model
        .complete(request("one"), CancellationToken::new())
        .await
        .expect("first response");
    let second = model
        .complete(request("two"), CancellationToken::new())
        .await
        .expect("second response");
    assert_eq!(first.assistant_text, "first");
    assert_eq!(second.assistant_text, "second");

    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    for (endpoint, body, authorization) in requests {
        assert_eq!(endpoint, "https://example.test/v1/responses");
        assert_eq!(authorization.as_deref(), Some(SECRET_CANARY));
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["reasoning"]["effort"], "ultra");
        assert_eq!(body["service_tier"], "default");
        assert_eq!(body["store"], false);
    }
}

#[tokio::test]
async fn classifies_http_and_transport_failures_without_provider_bodies() {
    let cases = [
        (
            Ok(HttpResponse::new(401, None, BODY_CANARY.into()).unwrap()),
            ModelError::Authentication,
        ),
        (
            Ok(HttpResponse::new(400, None, BODY_CANARY.into()).unwrap()),
            ModelError::Protocol,
        ),
        (
            Ok(HttpResponse::new(503, None, BODY_CANARY.into()).unwrap()),
            ModelError::Transport,
        ),
        (Err(HttpTransportError::Timeout), ModelError::Transport),
        (Err(HttpTransportError::Cancelled), ModelError::Cancelled),
        (
            Err(HttpTransportError::ResponseTooLarge),
            ModelError::Protocol,
        ),
    ];

    for (result, expected) in cases {
        let model = ResponsesLanguageModel::new(
            "http://127.0.0.1:1234/v1",
            FakeTransport::with_responses([result]),
            None,
            ResponsesRequestOptions::default(),
        )
        .expect("loopback model");
        let error = model
            .complete(request("prompt"), CancellationToken::new())
            .await
            .expect_err("case should fail");
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains(BODY_CANARY));
    }
}

#[tokio::test]
async fn maps_rate_limits_to_capped_retry_metadata() {
    let response = HttpResponse::new(429, Some(Duration::from_secs(900)), BODY_CANARY.into())
        .expect("bounded response");
    let model = ResponsesLanguageModel::new(
        "http://localhost:1234",
        FakeTransport::with_responses([Ok(response)]),
        None,
        ResponsesRequestOptions::default(),
    )
    .expect("loopback model");

    let error = model
        .complete(request("prompt"), CancellationToken::new())
        .await
        .expect_err("rate limit should fail");
    assert!(matches!(error, ModelError::RateLimited { .. }));
    assert_eq!(
        error.retry_metadata().retry_after(),
        Some(Duration::from_secs(300))
    );
}

#[tokio::test]
async fn maps_invalid_success_payloads_to_protocol_errors() {
    let response = HttpResponse::new(200, None, BODY_CANARY.into()).expect("bounded response");
    let model = ResponsesLanguageModel::new(
        "http://[::1]:1234/api/v1",
        FakeTransport::with_responses([Ok(response)]),
        None,
        ResponsesRequestOptions::default(),
    )
    .expect("loopback model");
    assert_eq!(
        model
            .complete(request("prompt"), CancellationToken::new())
            .await
            .unwrap_err(),
        ModelError::Protocol
    );
}

#[test]
fn rejects_unsafe_endpoints_and_redacts_model_debug_output() {
    for base_url in [
        "http://example.test/v1",
        "https://user:password@example.test/v1",
        "https://example.test/v1?token=secret",
    ] {
        assert!(matches!(
            ResponsesLanguageModel::new(
                base_url,
                FakeTransport::default(),
                None,
                ResponsesRequestOptions::default()
            ),
            Err(ResponsesModelError::InvalidEndpoint)
        ));
    }

    let model = authenticated_model(
        &format!("https://{HOST_CANARY}/v1"),
        FakeTransport::default(),
    );
    let rendered = format!("{model:?}");
    assert!(!rendered.contains(HOST_CANARY));
    assert!(!rendered.contains(SECRET_CANARY));
    assert!(rendered.contains("authorization_present: true"));
}
