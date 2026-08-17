//! Provider-specific model adapters and their injectable transport boundary.

pub mod anthropic;
mod http;
mod json;
pub mod responses;
mod retry;
mod sse;
mod wire;

pub use http::{
    CredentialHeader, HttpRequest, HttpResponse, HttpResponseStream, HttpTransport,
    HttpTransportError, ReqwestHttpTransport, MAX_HTTP_PROTOCOL_HEADERS, MAX_HTTP_REQUEST_BYTES,
    MAX_HTTP_RESPONSE_BYTES, MAX_HTTP_TIMEOUT,
};
pub use wire::{
    build_wire_model, build_wire_model_with_transport, ConfiguredWireModel, WireModelBuildError,
};
