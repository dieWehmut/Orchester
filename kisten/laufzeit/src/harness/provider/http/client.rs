use std::fmt;
use std::time::Duration;

use futures::StreamExt;
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER, USER_AGENT};
use reqwest::redirect::Policy;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{
    CredentialHeader, HttpRequest, HttpResponse, HttpResponseStream, HttpTransport,
    HttpTransportError,
};

const DEFAULT_USER_AGENT: &str = concat!("orchester/", env!("CARGO_PKG_VERSION"));

const AGENTROUTER_RESPONSES_COMPAT_USER_AGENT: &str = concat!(
    "codex_cli_rs/",
    env!("CARGO_PKG_VERSION"),
    " orchester/",
    env!("CARGO_PKG_VERSION")
);

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
            // A default agent belongs on the client, not on each call site: a
            // provider fronted by a bot filter answers 403 to a POST that names
            // no agent, and that reaches the operator as an unexplained refusal.
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .map(|client| Self { client })
            .map_err(|_| HttpTransportError::Transport)
    }

    async fn send_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpTransportError> {
        let response_limit = request.response_limit;
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        if response
            .content_length()
            .is_some_and(|length| length > response_limit as u64)
        {
            return Err(HttpTransportError::ResponseTooLarge);
        }

        let (status, retry_after) = response_metadata(&response);
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

    async fn send_stream_inner(
        &self,
        request: HttpRequest,
    ) -> Result<HttpResponseStream, HttpTransportError> {
        let response_limit = request.response_limit;
        let response = self
            .request_builder(&request)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        if response
            .content_length()
            .is_some_and(|length| length > response_limit as u64)
        {
            return Err(HttpTransportError::ResponseTooLarge);
        }

        let (status, retry_after) = response_metadata(&response);
        let chunks = futures::stream::try_unfold(
            (response.bytes_stream(), 0usize),
            move |(mut body, total)| async move {
                let Some(chunk) = body.next().await else {
                    return Ok(None);
                };
                let chunk = chunk.map_err(map_reqwest_error)?;
                if chunk.len() > response_limit.saturating_sub(total) {
                    return Err(HttpTransportError::ResponseTooLarge);
                }
                let next_total = total + chunk.len();
                Ok(Some((chunk.to_vec(), (body, next_total))))
            },
        );
        HttpResponseStream::new(status, retry_after, Box::pin(chunks))
    }

    fn request_builder(&self, request: &HttpRequest) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .post(request.endpoint.clone())
            .timeout(request.timeout)
            .header(CONTENT_TYPE, "application/json")
            .body(request.body.clone());
        if let Some(user_agent) = compatibility_user_agent(&request.endpoint) {
            builder = builder.header(USER_AGENT, user_agent);
        }
        for (name, value) in request.protocol_headers() {
            builder = builder.header(*name, *value);
        }
        if let Some(secret) = &request.authorization {
            builder = match request.credential_header() {
                CredentialHeader::Bearer => builder.bearer_auth(secret.expose_for_provider()),
                CredentialHeader::ApiKey => builder.header(
                    CredentialHeader::ApiKey.header_name(),
                    secret.expose_for_provider(),
                ),
            };
        }
        builder
    }
}

fn response_metadata(response: &reqwest::Response) -> (u16, Option<Duration>) {
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    (response.status().as_u16(), retry_after)
}

fn compatibility_user_agent(endpoint: &Url) -> Option<&'static str> {
    endpoint
        .host_str()
        .filter(|host| host.eq_ignore_ascii_case("agentrouter.org"))
        .map(|_| AGENTROUTER_RESPONSES_COMPAT_USER_AGENT)
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

    async fn send_stream(
        &self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Result<HttpResponseStream, HttpTransportError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(HttpTransportError::Cancelled),
            result = self.send_stream_inner(request) => result,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_user_agent_is_limited_to_agentrouter() {
        let agentrouter = Url::parse("https://agentrouter.org/v1/responses").unwrap();
        let ordinary = Url::parse("https://api.example.test/v1/responses").unwrap();
        let lookalike = Url::parse("https://agentrouter.org.example.test/v1/responses").unwrap();

        assert_eq!(
            compatibility_user_agent(&agentrouter),
            Some(AGENTROUTER_RESPONSES_COMPAT_USER_AGENT)
        );
        assert_eq!(compatibility_user_agent(&ordinary), None);
        assert_eq!(compatibility_user_agent(&lookalike), None);
    }
}
