use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use url::{Host, Url};

use crate::harness::credentials::ProviderSecret;

mod client;

pub use client::ReqwestHttpTransport;

pub const MAX_HTTP_REQUEST_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HTTP_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_HTTP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

const DEFAULT_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HttpTransportError {
    #[error("model HTTP request is invalid")]
    InvalidRequest,
    #[error("model HTTP response is invalid")]
    InvalidResponse,
    #[error("model HTTP request was cancelled")]
    Cancelled,
    #[error("model HTTP request timed out")]
    Timeout,
    #[error("model HTTP transport failed")]
    Transport,
    #[error("model HTTP response exceeds the configured limit")]
    ResponseTooLarge,
}

pub struct HttpRequest {
    endpoint: Url,
    body: Vec<u8>,
    authorization: Option<Arc<ProviderSecret>>,
    timeout: Duration,
    response_limit: usize,
}

impl HttpRequest {
    pub fn new(
        endpoint: Url,
        body: Vec<u8>,
        authorization: Option<ProviderSecret>,
    ) -> Result<Self, HttpTransportError> {
        if !valid_endpoint(&endpoint) || body.len() > MAX_HTTP_REQUEST_BYTES {
            return Err(HttpTransportError::InvalidRequest);
        }
        Ok(Self {
            endpoint,
            body,
            authorization: authorization.map(Arc::new),
            timeout: DEFAULT_HTTP_TIMEOUT,
            response_limit: DEFAULT_HTTP_RESPONSE_BYTES,
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, HttpTransportError> {
        if timeout.is_zero() || timeout > MAX_HTTP_TIMEOUT {
            return Err(HttpTransportError::InvalidRequest);
        }
        self.timeout = timeout;
        Ok(self)
    }

    pub fn with_response_limit(mut self, limit: usize) -> Result<Self, HttpTransportError> {
        if limit == 0 || limit > MAX_HTTP_RESPONSE_BYTES {
            return Err(HttpTransportError::InvalidRequest);
        }
        self.response_limit = limit;
        Ok(self)
    }

    pub fn endpoint(&self) -> &Url {
        &self.endpoint
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn authorization(&self) -> Option<&ProviderSecret> {
        self.authorization.as_deref()
    }

    pub(crate) fn with_shared_authorization(
        mut self,
        authorization: Option<Arc<ProviderSecret>>,
    ) -> Self {
        self.authorization = authorization;
        self
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn response_limit(&self) -> usize {
        self.response_limit
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("endpoint", &"[REDACTED]")
            .field("body_bytes", &self.body.len())
            .field("authorization_present", &self.authorization.is_some())
            .field("timeout", &self.timeout)
            .field("response_limit", &self.response_limit)
            .finish()
    }
}

pub struct HttpResponse {
    status: u16,
    retry_after: Option<Duration>,
    body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(
        status: u16,
        retry_after: Option<Duration>,
        body: Vec<u8>,
    ) -> Result<Self, HttpTransportError> {
        if !(100..=599).contains(&status) {
            return Err(HttpTransportError::InvalidResponse);
        }
        if body.len() > MAX_HTTP_RESPONSE_BYTES {
            return Err(HttpTransportError::ResponseTooLarge);
        }
        Ok(Self {
            status,
            retry_after: retry_after.map(|delay| delay.min(MAX_RETRY_AFTER)),
            body,
        })
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("retry_after", &self.retry_after)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn send(
        &self,
        request: HttpRequest,
        cancel: CancellationToken,
    ) -> Result<HttpResponse, HttpTransportError>;
}

fn valid_endpoint(endpoint: &Url) -> bool {
    if endpoint.host().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return false;
    }
    match (endpoint.scheme(), endpoint.host()) {
        ("https", Some(_)) => true,
        ("http", Some(host)) => is_loopback(host),
        _ => false,
    }
}

fn is_loopback(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain
            .trim_end_matches('.')
            .eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}
