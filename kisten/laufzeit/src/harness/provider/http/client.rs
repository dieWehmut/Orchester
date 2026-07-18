use std::fmt;
use std::time::Duration;

use futures::StreamExt;
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER};
use reqwest::redirect::Policy;
use tokio_util::sync::CancellationToken;

use super::{HttpRequest, HttpResponse, HttpTransport, HttpTransportError};

/// The production HTTP implementation for provider-neutral model requests.
///
/// Redirects are disabled so an authorization header cannot be forwarded to a
/// different origin. Response bytes are consumed incrementally under the
/// caller-selected limit; no provider body is included in an error value.
pub struct ReqwestHttpTransport {
    client: reqwest::Client,
}

impl ReqwestHttpTransport {
    pub fn new() -> Result<Self, HttpTransportError> {
        reqwest::Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map(|client| Self { client })
            .map_err(|_| HttpTransportError::Transport)
    }

    async fn send_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpTransportError> {
        let endpoint = request.endpoint;
        let body = request.body;
        let authorization = request.authorization;
        let timeout = request.timeout;
        let response_limit = request.response_limit;

        let mut builder = self
            .client
            .post(endpoint)
            .timeout(timeout)
            .header(CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(secret) = authorization {
            builder = builder.bearer_auth(secret.expose_for_provider());
        }

        let response = builder.send().await.map_err(map_reqwest_error)?;
        if response
            .content_length()
            .is_some_and(|length| length > response_limit as u64)
        {
            return Err(HttpTransportError::ResponseTooLarge);
        }

        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_reqwest_error)?;
            if chunk.len() > response_limit.saturating_sub(body.len()) {
                return Err(HttpTransportError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        HttpResponse::new(status, retry_after, body)
    }
}

impl fmt::Debug for ReqwestHttpTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReqwestHttpTransport")
            .field("redirects", &"disabled")
            .field("body_policy", &"bounded")
            .finish()
    }
}

#[async_trait::async_trait]
impl HttpTransport for ReqwestHttpTransport {
    async fn send(
        &self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(HttpTransportError::Cancelled),
            result = self.send_inner(request) => result,
        }
    }
}

fn map_reqwest_error(error: reqwest::Error) -> HttpTransportError {
    if error.is_timeout() {
        HttpTransportError::Timeout
    } else if error.is_builder() {
        HttpTransportError::InvalidRequest
    } else {
        HttpTransportError::Transport
    }
}
