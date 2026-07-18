//! Provider-specific model adapters and their injectable transport boundary.

mod http;
pub mod responses;

pub use http::{
    HttpRequest, HttpResponse, HttpTransport, HttpTransportError, MAX_HTTP_REQUEST_BYTES,
    MAX_HTTP_RESPONSE_BYTES, MAX_HTTP_TIMEOUT, ReqwestHttpTransport,
};
