//! Explicit OpenAI Responses wire mappings.

mod json;
mod request;

pub use request::{ResponsesRequestError, ResponsesRequestOptions, encode_responses_request};
