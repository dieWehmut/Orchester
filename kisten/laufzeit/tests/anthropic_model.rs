use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use orchester_laufzeit::harness::credentials::{provider_secret, InMemoryCredentialStore};
use orchester_laufzeit::harness::provider::anthropic::{
    AnthropicLanguageModel, AnthropicModelError, AnthropicRequestOptions, DEFAULT_MAX_OUTPUT_TOKENS,
};
use orchester_laufzeit::harness::provider::{
    CredentialHeader, HttpRequest, HttpResponse, HttpTransport, HttpTransportError,
};
use orchester_modell::{
    LanguageModel, ModelError, ModelEventSink, ModelItem, ModelMessage, ModelRequest, ModelRole,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

const SECRET_CANARY: &str = "sk-anthropic-model-secret";
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

#[derive(Clone)]
struct CapturedRequest {
    endpoint: String,
    body: Value,
    authorization: Option<String>,
    credential_header: CredentialHeader,
    protocol_headers: Vec<(&'static str, &'static str)>,
}

struct CollectingSink(Arc<Mutex<Vec<String>>>);

impl ModelEventSink for CollectingSink {
    fn response_started(&self) {
        self.0
            .lock()
            .expect("sink lock")
            .push("<started>".to_owned());
    }

    fn text_delta(&self, delta: &str) {
        self.0.lock().expect("sink lock").push(delta.to_owned());
    }

    fn response_completed(&self) {
        self.0
            .lock()
            .expect("sink lock")
            .push("<completed>".to_owned());
    }
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

    fn requests(&self) -> Vec<CapturedRequest> {
        self.state.lock().expect("fake lock").requests.clone()
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
            credential_header: request.credential_header(),
            protocol_headers: request.protocol_headers().to_vec(),
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
        model: "claude-test".into(),
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
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": text}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 3, "output_tokens": 2}
        }))
        .expect("fixture JSON"),
    )
    .expect("bounded fixture")
}

/// Assemble a Messages event stream from named frames, so the fixture stays
/// readable instead of a hand-escaped byte string.
fn event_stream(frames: &[(&str, Value)]) -> HttpResponse {
    let mut body = String::new();
    for (name, data) in frames {
        body.push_str(&format!("event: {name}\ndata: {data}\n\n"));
    }
    HttpResponse::new(200, None, body.into_bytes()).expect("bounded SSE fixture")
}

fn streamed_text_and_tool_call() -> HttpResponse {
    event_stream(&[
        (
            "message_start",
            json!({"message": {"usage": {"input_tokens": 11, "output_tokens": 0}}}),
        ),
        (
            "content_block_start",
            json!({"index": 0, "content_block": {"type": "text", "text": ""}}),
        ),
        ("ping", json!({})),
        (
            "content_block_delta",
            json!({"index": 0, "delta": {"type": "text_delta", "text": "reading "}}),
        ),
        (
            "content_block_delta",
            json!({"index": 0, "delta": {"type": "text_delta", "text": "the file"}}),
        ),
        ("content_block_stop", json!({"index": 0})),
        (
            "content_block_start",
            json!({
                "index": 1,
                "content_block": {"type": "tool_use", "id": "toolu-1", "name": "read_file"}
            }),
        ),
        (
            "content_block_delta",
            json!({"index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":"}}),
        ),
        (
            "content_block_delta",
            json!({
                "index": 1,
                "delta": {"type": "input_json_delta", "partial_json": "\"src/lib.rs\"}"}
            }),
        ),
        ("content_block_stop", json!({"index": 1})),
        (
            "message_delta",
            json!({"delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 9}}),
        ),
        ("message_stop", json!({})),
    ])
}

fn authenticated_model(
    base_url: &str,
    transport: FakeTransport,
) -> AnthropicLanguageModel<FakeTransport> {
    let store = InMemoryCredentialStore::with("Anthropic", SECRET_CANARY);
    let secret = provider_secret(&store, "Anthropic")
        .expect("credential lookup")
        .expect("credential present");
    AnthropicLanguageModel::new(
        base_url,
        transport,
        Some(secret),
        AnthropicRequestOptions {
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
            service_tier: Some("standard_only".into()),
        },
    )
    .expect("valid model")
}

#[tokio::test]
async fn sends_the_credential_as_an_api_key_beside_the_pinned_protocol_version() {
    let transport = FakeTransport::with_responses([Ok(success("done"))]);
    let model = authenticated_model("https://example.test/", transport.clone());

    let response = model
        .complete(request("hello"), CancellationToken::new())
        .await
        .expect("response");

    assert_eq!(response.assistant_text, "done");
    assert_eq!(response.usage.input_tokens, 3);
    assert_eq!(response.usage.output_tokens, 2);
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    let sent = &requests[0];
    // Messages rejects a bearer token, so the wire must select `x-api-key` and
    // still leave the secret for the transport to write.
    assert_eq!(sent.credential_header, CredentialHeader::ApiKey);
    assert_eq!(sent.authorization.as_deref(), Some(SECRET_CANARY));
    assert_eq!(sent.protocol_headers, [("anthropic-version", "2023-06-01")]);
    assert_eq!(sent.body["model"], "claude-test");
    assert_eq!(sent.body["max_tokens"], DEFAULT_MAX_OUTPUT_TOKENS);
    assert_eq!(sent.body["service_tier"], "standard_only");
    assert_eq!(sent.body["stream"], false);
    // Extended thinking is not round-tripped by the agent loop, so it is never
    // requested.
    assert!(sent.body.get("thinking").is_none());
}

#[tokio::test]
async fn derives_one_messages_endpoint_from_every_configured_base_path() {
    for (base_url, expected) in [
        ("https://example.test", "https://example.test/v1/messages"),
        ("https://example.test/", "https://example.test/v1/messages"),
        (
            "https://example.test/v1",
            "https://example.test/v1/messages",
        ),
        (
            "https://example.test/v1/messages",
            "https://example.test/v1/messages",
        ),
        (
            "https://example.test/relay/",
            "https://example.test/relay/v1/messages",
        ),
    ] {
        let transport = FakeTransport::with_responses([Ok(success("done"))]);
        let model = authenticated_model(base_url, transport.clone());
        model
            .complete(request("hello"), CancellationToken::new())
            .await
            .expect("response");
        assert_eq!(transport.requests()[0].endpoint, expected);
    }
}

#[tokio::test]
async fn forwards_streamed_text_deltas_and_assembles_a_streamed_tool_call() {
    let transport = FakeTransport::with_responses([Ok(streamed_text_and_tool_call())]);
    let model = authenticated_model("https://example.test/v1", transport.clone());
    let deltas = Arc::new(Mutex::new(Vec::new()));

    let response = model
        .complete_with_events(
            request("stream me"),
            CancellationToken::new(),
            Some(Arc::new(CollectingSink(Arc::clone(&deltas)))),
        )
        .await
        .expect("streamed response");

    assert_eq!(response.assistant_text, "reading the file");
    let call = response.tool_call.expect("streamed tool call");
    assert_eq!(call.call_id.0, "toolu-1");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments_json, r#"{"path":"src/lib.rs"}"#);
    assert_eq!(response.usage.input_tokens, 11);
    assert_eq!(response.usage.output_tokens, 9);
    assert_eq!(
        *deltas.lock().expect("deltas lock"),
        ["<started>", "reading ", "the file", "<completed>"]
    );
    assert_eq!(transport.requests()[0].body["stream"], true);
}

#[tokio::test]
async fn rejects_a_stream_that_ends_before_the_message_does() {
    let truncated = event_stream(&[
        (
            "content_block_start",
            json!({"index": 0, "content_block": {"type": "text", "text": ""}}),
        ),
        (
            "content_block_delta",
            json!({"index": 0, "delta": {"type": "text_delta", "text": "partial"}}),
        ),
    ]);
    let model = authenticated_model(
        "https://example.test/v1",
        FakeTransport::with_responses([Ok(truncated)]),
    );
    let events = Arc::new(Mutex::new(Vec::new()));

    let error = model
        .complete_with_events(
            request("incomplete"),
            CancellationToken::new(),
            Some(Arc::new(CollectingSink(Arc::clone(&events)))),
        )
        .await
        .expect_err("an unterminated stream should fail");

    assert_eq!(error, ModelError::Protocol);
    assert_eq!(
        *events.lock().expect("events lock"),
        ["<started>", "partial"]
    );
}

#[tokio::test]
async fn retries_a_transient_failure_once_before_succeeding() {
    let transport = FakeTransport::with_responses([
        Ok(HttpResponse::new(503, None, BODY_CANARY.into()).expect("bounded response")),
        Ok(success("recovered")),
    ]);
    let model = authenticated_model("https://example.test/v1", transport.clone());

    let response = model
        .complete(request("retry me"), CancellationToken::new())
        .await
        .expect("a transient status should be retried");

    assert_eq!(response.assistant_text, "recovered");
    assert_eq!(transport.requests().len(), 2);
}

#[tokio::test]
async fn classifies_http_and_transport_failures_without_provider_bodies() {
    let cases = [
        (
            Ok(HttpResponse::new(401, None, BODY_CANARY.into()).unwrap()),
            ModelError::Authentication,
        ),
        (
            Ok(HttpResponse::new(403, None, BODY_CANARY.into()).unwrap()),
            ModelError::Forbidden,
        ),
        (
            Ok(HttpResponse::new(400, None, BODY_CANARY.into()).unwrap()),
            ModelError::Protocol,
        ),
        (
            Ok(HttpResponse::new(200, None, BODY_CANARY.into()).unwrap()),
            ModelError::Protocol,
        ),
        (Err(HttpTransportError::Cancelled), ModelError::Cancelled),
        (
            Err(HttpTransportError::ResponseTooLarge),
            ModelError::Protocol,
        ),
    ];

    for (result, expected) in cases {
        let model = AnthropicLanguageModel::new(
            "http://127.0.0.1:1234/v1",
            FakeTransport::with_responses([result]),
            None,
            AnthropicRequestOptions::default(),
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
async fn caps_the_retry_delay_a_rate_limit_asks_for() {
    let response = HttpResponse::new(429, Some(Duration::from_secs(900)), BODY_CANARY.into())
        .expect("bounded response");
    let model = AnthropicLanguageModel::new(
        "http://localhost:1234",
        FakeTransport::with_responses([Ok(response)]),
        None,
        AnthropicRequestOptions::default(),
    )
    .expect("loopback model");

    let error = model
        .complete(request("prompt"), CancellationToken::new())
        .await
        .expect_err("a rate limit should fail");
    assert!(matches!(error, ModelError::RateLimited { .. }));
    assert_eq!(
        error.retry_metadata().retry_after(),
        Some(Duration::from_secs(300))
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
            AnthropicLanguageModel::new(
                base_url,
                FakeTransport::default(),
                None,
                AnthropicRequestOptions::default()
            ),
            Err(AnthropicModelError::InvalidEndpoint)
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
