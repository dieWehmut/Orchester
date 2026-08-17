use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use orchester_modell::{LanguageModel, ModelError, ModelEventSink, ModelRequest, ModelResponse};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::harness::credentials::ProviderSecret;
use crate::harness::provider::retry::send_with_retry;
use crate::harness::provider::{
    CredentialHeader, HttpRequest, HttpResponse, HttpTransport, HttpTransportError,
    MAX_HTTP_RESPONSE_BYTES,
};

use super::{
    decode_anthropic_event_stream, decode_anthropic_response, encode_anthropic_request,
    encode_anthropic_stream_request, AnthropicRequestOptions, AnthropicResponseError,
};

/// The Messages protocol revision every request pins. Anthropic requires the
/// header and treats its absence as an unversioned, rejected call.
const ANTHROPIC_VERSION: (&str, &str) = ("anthropic-version", "2023-06-01");

/// Configuration errors raised while constructing a Messages model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AnthropicModelError {
    #[error("Messages provider endpoint is invalid")]
    InvalidEndpoint,
}

/// A unary Anthropic Messages adapter over an injectable HTTP transport.
pub struct AnthropicLanguageModel<T> {
    transport: T,
    endpoint: Url,
    authorization: Option<Arc<ProviderSecret>>,
    options: AnthropicRequestOptions,
}

impl<T: HttpTransport> AnthropicLanguageModel<T> {
    pub fn new(
        base_url: &str,
        transport: T,
        authorization: Option<ProviderSecret>,
        options: AnthropicRequestOptions,
    ) -> Result<Self, AnthropicModelError> {
        Ok(Self {
            transport,
            endpoint: messages_endpoint(base_url)?,
            authorization: authorization.map(Arc::new),
            options,
        })
    }
}
impl<T> AnthropicLanguageModel<T> {
    /// Wrap an encoded body in the request shape Messages expects: the
    /// credential travels in `x-api-key` and the protocol revision travels
    /// beside it, both written by the transport rather than by this module.
    fn http_request(&self, body: Vec<u8>) -> Result<HttpRequest, ModelError> {
        HttpRequest::new(self.endpoint.clone(), body, None)
            .and_then(|request| request.with_response_limit(MAX_HTTP_RESPONSE_BYTES))
            .and_then(|request| request.with_protocol_headers(vec![ANTHROPIC_VERSION]))
            .map(|request| {
                request
                    .with_credential_header(CredentialHeader::ApiKey)
                    .with_shared_authorization(self.authorization.clone())
            })
            .map_err(map_transport_error)
    }
}

impl<T> fmt::Debug for AnthropicLanguageModel<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnthropicLanguageModel")
            .field("endpoint", &"[REDACTED]")
            .field(
                "authorization_present",
                &self.authorization.as_ref().is_some(),
            )
            .field("options", &self.options)
            .finish()
    }
}

#[async_trait]
impl<T: HttpTransport + 'static> LanguageModel for AnthropicLanguageModel<T> {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        let body =
            encode_anthropic_request(&request, &self.options).map_err(|_| ModelError::Protocol)?;
        let request = self.http_request(body)?;
        let response = send_with_retry(&request, &cancel, |request, cancel| {
            self.transport.send(request, cancel)
        })
        .await
        .map_err(map_transport_error)?;
        decode_http_response(response)
    }

    async fn complete_with_events(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<ModelResponse, ModelError> {
        let Some(events) = events else {
            return self.complete(request, cancel).await;
        };
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        let body = encode_anthropic_stream_request(&request, &self.options)
            .map_err(|_| ModelError::Protocol)?;
        let request = self.http_request(body)?;
        let response = send_with_retry(&request, &cancel, |request, cancel| {
            self.transport.send_stream(request, cancel)
        })
        .await
        .map_err(map_transport_error)?;
        match response.status() {
            200..=299 => {
                events.response_started();
                let decoded =
                    decode_anthropic_event_stream(response, cancel, Some(events.as_ref()))
                        .await
                        .map_err(map_response_error);
                if decoded.is_ok() {
                    events.response_completed();
                }
                decoded
            }
            status => Err(status_error(status, response.retry_after())),
        }
    }
}
fn decode_http_response(response: HttpResponse) -> Result<ModelResponse, ModelError> {
    match response.status() {
        200..=299 => decode_anthropic_response(response.body()).map_err(map_response_error),
        status => Err(status_error(status, response.retry_after())),
    }
}

fn status_error(status: u16, retry_after: Option<std::time::Duration>) -> ModelError {
    match status {
        401 => ModelError::Authentication,
        403 => ModelError::Forbidden,
        429 => ModelError::rate_limited(retry_after),
        408 | 425 | 500..=599 => ModelError::Transport,
        _ => ModelError::Protocol,
    }
}

fn map_transport_error(error: HttpTransportError) -> ModelError {
    match error {
        HttpTransportError::Cancelled => ModelError::Cancelled,
        HttpTransportError::Timeout | HttpTransportError::Transport => ModelError::Transport,
        HttpTransportError::InvalidRequest
        | HttpTransportError::InvalidResponse
        | HttpTransportError::ResponseTooLarge => ModelError::Protocol,
    }
}

fn map_response_error(error: AnthropicResponseError) -> ModelError {
    match error {
        AnthropicResponseError::Cancelled => ModelError::Cancelled,
        AnthropicResponseError::Transport => ModelError::Transport,
        _ => ModelError::Protocol,
    }
}

/// Derive the Messages endpoint from a configured provider base URL.
///
/// A relay may be configured with the bare host, with `/v1`, or with the full
/// path, and all three name the same endpoint.
fn messages_endpoint(base_url: &str) -> Result<Url, AnthropicModelError> {
    if base_url.is_empty()
        || base_url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AnthropicModelError::InvalidEndpoint);
    }
    let mut endpoint = Url::parse(base_url).map_err(|_| AnthropicModelError::InvalidEndpoint)?;
    let base_path = endpoint.path().trim_end_matches('/');
    let path = if base_path.ends_with("/messages") {
        base_path.to_owned()
    } else if base_path.ends_with("/v1") {
        format!("{base_path}/messages")
    } else if base_path.is_empty() {
        "/v1/messages".to_owned()
    } else {
        format!("{base_path}/v1/messages")
    };
    endpoint.set_path(&path);

    HttpRequest::new(endpoint.clone(), Vec::new(), None)
        .map_err(|_| AnthropicModelError::InvalidEndpoint)?;
    Ok(endpoint)
}
