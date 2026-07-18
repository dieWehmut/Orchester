use async_trait::async_trait;
use orchester_laufzeit::harness::credentials::{InMemoryCredentialStore, provider_secret};
use orchester_laufzeit::harness::provider::{
    HttpRequest, HttpResponse, HttpTransport, HttpTransportError, MAX_HTTP_REQUEST_BYTES,
    MAX_HTTP_RESPONSE_BYTES, MAX_HTTP_TIMEOUT,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use url::Url;

const URL_CANARY: &str = "sensitive-host.example";
const SECRET_CANARY: &str = "sk-sensitive-http-boundary";
const BODY_CANARY: &str = "sensitive prompt body";

struct FakeTransport;

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
        HttpResponse::new(
            200,
            Some(Duration::from_secs(900)),
            request.body().len().to_string().into_bytes(),
        )
    }
}

fn authenticated_request() -> HttpRequest {
    let store = InMemoryCredentialStore::with("OpenAI", SECRET_CANARY);
    let secret = provider_secret(&store, "OpenAI")
        .expect("credential lookup")
        .expect("credential is present");
    HttpRequest::new(
        Url::parse(&format!("https://{URL_CANARY}/v1/responses")).expect("valid URL"),
        BODY_CANARY.as_bytes().to_vec(),
        Some(secret),
    )
    .expect("valid bounded request")
}

fn loopback_request() -> HttpRequest {
    HttpRequest::new(
        Url::parse("http://127.0.0.1/v1/responses").expect("valid URL"),
        Vec::new(),
        None,
    )
    .expect("loopback HTTP is allowed")
}

#[tokio::test]
async fn custom_transport_receives_the_bounded_request_contract() {
    let response = FakeTransport
        .send(authenticated_request(), CancellationToken::new())
        .await
        .expect("fake response");

    assert_eq!(response.status(), 200);
    assert_eq!(response.body(), BODY_CANARY.len().to_string().as_bytes());
    assert_eq!(response.retry_after(), Some(Duration::from_secs(300)));
}

#[test]
fn request_and_response_debug_views_redact_sensitive_material() {
    let request = authenticated_request();
    let request_debug = format!("{request:?}");
    assert!(request_debug.contains("authorization_present: true"));
    assert!(request_debug.contains(&format!("body_bytes: {}", BODY_CANARY.len())));
    for canary in [URL_CANARY, SECRET_CANARY, BODY_CANARY] {
        assert!(!request_debug.contains(canary));
    }

    let response =
        HttpResponse::new(200, None, BODY_CANARY.as_bytes().to_vec()).expect("bounded response");
    let response_debug = format!("{response:?}");
    assert!(response_debug.contains(&format!("body_bytes: {}", BODY_CANARY.len())));
    assert!(!response_debug.contains(BODY_CANARY));
}

#[test]
fn request_validation_rejects_unsafe_urls_and_unbounded_inputs() {
    let remote_http = HttpRequest::new(
        Url::parse("http://example.test/v1/responses").expect("valid URL"),
        Vec::new(),
        None,
    );
    assert_eq!(remote_http.unwrap_err(), HttpTransportError::InvalidRequest);

    let oversized = HttpRequest::new(
        Url::parse("https://example.test/v1/responses").expect("valid URL"),
        vec![0; MAX_HTTP_REQUEST_BYTES + 1],
        None,
    );
    assert_eq!(oversized.unwrap_err(), HttpTransportError::InvalidRequest);

    assert_eq!(
        loopback_request().with_timeout(Duration::ZERO).unwrap_err(),
        HttpTransportError::InvalidRequest,
    );
    assert_eq!(
        loopback_request()
            .with_timeout(MAX_HTTP_TIMEOUT + Duration::from_millis(1))
            .unwrap_err(),
        HttpTransportError::InvalidRequest,
    );
    assert_eq!(
        loopback_request()
            .with_response_limit(MAX_HTTP_RESPONSE_BYTES + 1)
            .unwrap_err(),
        HttpTransportError::InvalidRequest,
    );
}

#[test]
fn typed_errors_never_include_request_material() {
    for error in [
        HttpTransportError::InvalidRequest,
        HttpTransportError::InvalidResponse,
        HttpTransportError::Cancelled,
        HttpTransportError::Timeout,
        HttpTransportError::Transport,
        HttpTransportError::ResponseTooLarge,
    ] {
        let rendered = format!("{error:?} {error}");
        for canary in [URL_CANARY, SECRET_CANARY, BODY_CANARY] {
            assert!(!rendered.contains(canary));
        }
    }
}
