use axum::http::StatusCode;
use orchester_netz::{api_error_response, ApiErrorCode};

#[test]
fn api_error_mapping_returns_stable_status_and_safe_json() {
    let response = api_error_response(ApiErrorCode::Unauthorized, Some("request-123"));

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.body.code, "unauthorized");
    assert_eq!(response.body.error, "authentication is required");
    assert_eq!(response.body.request_id.as_deref(), Some("request-123"));
    assert!(!response.body.retryable);
}

#[test]
fn internal_error_mapping_does_not_echo_internal_details() {
    let response = api_error_response(ApiErrorCode::Internal, None);

    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.body.error, "internal server error");
    assert_eq!(response.body.request_id, None);
    assert!(!response.body.retryable);
}
