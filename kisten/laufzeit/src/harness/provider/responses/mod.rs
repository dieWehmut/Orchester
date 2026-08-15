//! Explicit OpenAI Responses wire mappings.

mod factory;
mod json;
mod model;
mod request;
mod response;

pub use factory::{
    build_responses_model, build_responses_model_with_transport, ConfiguredResponsesModel,
    ResponsesModelBuildError,
};
pub use model::{ResponsesLanguageModel, ResponsesModelError};
pub use request::{
    encode_responses_request, encode_responses_stream_request, ResponsesRequestError,
    ResponsesRequestOptions,
};
pub use response::{
    decode_responses_event_stream, decode_responses_response, ResponsesResponseError,
};
