//! Explicit OpenAI Responses wire mappings.

mod model;
mod request;
mod response;

pub use model::{ResponsesLanguageModel, ResponsesModelError};
pub use request::{
    encode_responses_request, encode_responses_stream_request, ResponsesRequestError,
    ResponsesRequestOptions,
};
pub use response::{
    decode_responses_event_stream, decode_responses_response, ResponsesResponseError,
};
