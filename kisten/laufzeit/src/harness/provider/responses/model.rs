use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use orchester_modell::{LanguageModel, ModelError, ModelRequest, ModelResponse};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::harness::credentials::ProviderSecret;
use crate::harness::provider::{
    HttpRequest, HttpResponse, HttpTransport, HttpTransportError, MAX_HTTP_RESPONSE_BYTES,
};

use super::{decode_responses_response, encode_responses_request, ResponsesRequestOptions};

const MAX_TRANSIENT_TRANSPORT_ATTEMPTS: usize = 2;
const TRANSIENT_TRANSPORT_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Configuration errors raised while constructing a Responses model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ResponsesModelError {
    #[error("Responses provider endpoint is invalid")]
    InvalidEndpoint,
}

/// A unary OpenAI Responses adapter over an injectable HTTP transport.
pub struct ResponsesLanguageModel<T> {
    transport: T,
    endpoint: Url,
    authorization: Option<Arc<ProviderSecret>>,
    options: ResponsesRequestOptions,
}

impl<T: HttpTransport> ResponsesLanguageModel<T> {
    pub fn new(
        base_url: &str,
        transport: T,
        authorization: Option<ProviderSecret>,
        options: ResponsesRequestOptions,
    ) -> Result<Self, ResponsesModelError> {
        Ok(Self {
            transport,
            endpoint: responses_endpoint(base_url)?,
            authorization: authorization.map(Arc::new),
            options,
        })
    }
}

impl<T> fmt::Debug for ResponsesLanguageModel<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponsesLanguageModel")
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
impl<T: HttpTransport + 'static> LanguageModel for ResponsesLanguageModel<T> {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        let body =
            encode_responses_request(&request, &self.options).map_err(|_| ModelError::Protocol)?;
        let request = HttpRequest::new(self.endpoint.clone(), body, None)
            .map_err(map_transport_error)?
            .with_response_limit(MAX_HTTP_RESPONSE_BYTES)
            .map_err(map_transport_error)?
            .with_shared_authorization(self.authorization.clone());
        let mut attempt = 1;
        loop {
            match self.transport.send(request.clone(), cancel.clone()).await {
                Ok(response) => return decode_http_response(response),
                Err(HttpTransportError::Transport)
                    if attempt < MAX_TRANSIENT_TRANSPORT_ATTEMPTS =>
                {
                    attempt += 1;
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return Err(ModelError::Cancelled),
                        _ = tokio::time::sleep(TRANSIENT_TRANSPORT_RETRY_DELAY) => {}
                    }
                }
                Err(error) => return Err(map_transport_error(error)),
            }
        }
    }
}

fn decode_http_response(response: HttpResponse) -> Result<ModelResponse, ModelError> {
    match response.status() {
        200..=299 => decode_responses_response(response.body()).map_err(|_| ModelError::Protocol),
        401 => Err(ModelError::Authentication),
        403 => Err(ModelError::Forbidden),
        429 => Err(ModelError::rate_limited(response.retry_after())),
        408 | 425 | 500..=599 => Err(ModelError::Transport),
        _ => Err(ModelError::Protocol),
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

fn responses_endpoint(base_url: &str) -> Result<Url, ResponsesModelError> {
    if base_url.is_empty()
        || base_url
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(ResponsesModelError::InvalidEndpoint);
    }
    let mut endpoint = Url::parse(base_url).map_err(|_| ResponsesModelError::InvalidEndpoint)?;
    let base_path = endpoint.path().trim_end_matches('/');
    let path = if base_path.ends_with("/responses") {
        base_path.to_owned()
    } else if base_path.ends_with("/v1") {
        format!("{base_path}/responses")
    } else if base_path.is_empty() {
        "/v1/responses".to_owned()
    } else {
        format!("{base_path}/v1/responses")
    };
    endpoint.set_path(&path);

    HttpRequest::new(endpoint.clone(), Vec::new(), None)
        .map_err(|_| ResponsesModelError::InvalidEndpoint)?;
    Ok(endpoint)
}
