//! HTTP boundary for local runs.
//!
//! The first route slice deliberately reports an explicit availability error
//! until a context has a runtime manager.  Keeping the handlers in place makes
//! the wire contract observable and gives later runtime work a stable seam.

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::HeaderMap,
    Json,
};

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    run_contract::{RunReplayRequestDto, StartRunRequest},
    ServerContext,
};

pub(crate) async fn start_run_handler(
    State(_context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<StartRunRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) = request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    request
        .validate()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
    Err(api_error_response(ApiErrorCode::Unavailable, request_id))
}

pub(crate) async fn snapshot_run_handler(
    State(_context): State<ServerContext>,
    headers: HeaderMap,
    Path(_run_id): Path<String>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiErrorResponse> {
    Err(api_error_response(
        ApiErrorCode::Unavailable,
        request_id_from_headers(&headers),
    ))
}

pub(crate) async fn replay_run_handler(
    State(_context): State<ServerContext>,
    headers: HeaderMap,
    Path(_run_id): Path<String>,
    request: Result<Json<RunReplayRequestDto>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) = request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    request
        .bounded_limit()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
    Err(api_error_response(ApiErrorCode::Unavailable, request_id))
}

pub(crate) async fn cancel_run_handler(
    State(_context): State<ServerContext>,
    headers: HeaderMap,
    Path(_run_id): Path<String>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiErrorResponse> {
    Err(api_error_response(
        ApiErrorCode::Unavailable,
        request_id_from_headers(&headers),
    ))
}
