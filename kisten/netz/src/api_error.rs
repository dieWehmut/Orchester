use axum::http::StatusCode;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    BadRequest,
    MethodNotAllowed,
    NotFound,
    Unauthorized,
    Forbidden,
    Conflict,
    ResyncRequired,
    ValidationFailed,
    RuntimeError,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApiErrorBody {
    pub error: &'static str,
    pub code: &'static str,
    pub request_id: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiErrorResponse {
    pub status: StatusCode,
    pub body: ApiErrorBody,
}

pub fn api_error_response(code: ApiErrorCode, request_id: Option<&str>) -> ApiErrorResponse {
    let (status, code_name, message, retryable) = match code {
        ApiErrorCode::BadRequest => (
            StatusCode::BAD_REQUEST,
            "bad_request",
            "request is invalid",
            false,
        ),
        ApiErrorCode::MethodNotAllowed => (
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "method is not allowed",
            false,
        ),
        ApiErrorCode::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "resource not found",
            false,
        ),
        ApiErrorCode::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "authentication is required",
            false,
        ),
        ApiErrorCode::Forbidden => (
            StatusCode::FORBIDDEN,
            "forbidden",
            "request is forbidden",
            false,
        ),
        ApiErrorCode::Conflict => (
            StatusCode::CONFLICT,
            "conflict",
            "request conflicts with current state",
            false,
        ),
        ApiErrorCode::ResyncRequired => (
            StatusCode::CONFLICT,
            "resync_required",
            "a fresh run snapshot is required",
            false,
        ),
        ApiErrorCode::ValidationFailed => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
            "request validation failed",
            false,
        ),
        ApiErrorCode::RuntimeError => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "runtime_error",
            "runtime operation failed",
            false,
        ),
        ApiErrorCode::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "service is temporarily unavailable",
            true,
        ),
        ApiErrorCode::Internal => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal server error",
            false,
        ),
    };

    ApiErrorResponse {
        status,
        body: ApiErrorBody {
            error: message,
            code: code_name,
            request_id: request_id.map(str::to_owned),
            retryable,
        },
    }
}
