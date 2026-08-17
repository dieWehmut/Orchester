//! Provider-specific model adapters and their injectable transport boundary.

mod http;
mod json;
pub mod responses;
mod sse;

pub use http::{
    CredentialHeader, HttpRequest, HttpResponse, HttpResponseStream, HttpTransport,
    HttpTransportError, ReqwestHttpTransport, MAX_HTTP_PROTOCOL_HEADERS, MAX_HTTP_REQUEST_BYTES,
    MAX_HTTP_RESPONSE_BYTES, MAX_HTTP_TIMEOUT,
};
