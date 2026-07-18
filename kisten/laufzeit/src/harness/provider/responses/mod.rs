//! Explicit OpenAI Responses wire mappings.

mod json;
mod request;
mod response;

pub use request::{ResponsesRequestError, ResponsesRequestOptions, encode_responses_request};
pub use response::{ResponsesResponseError, decode_responses_response};
