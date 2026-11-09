use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::ServerContext,
    health::no_store_headers,
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    Json,
};
use orchester_anwendung::library::{library_get, library_put, LibraryError, LibraryStateDto};
fn error(error: LibraryError, headers: &HeaderMap) -> ApiErrorResponse {
    api_error_response(
        match error {
            LibraryError::Invalid => ApiErrorCode::BadRequest,
            LibraryError::Conflict => ApiErrorCode::Conflict,
            LibraryError::Unavailable => ApiErrorCode::Unavailable,
        },
        request_id_from_headers(headers),
    )
}
pub(crate) async fn get(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<LibraryStateDto>), ApiErrorResponse> {
    let paths = context
        .paths()
        .ok_or_else(|| error(LibraryError::Unavailable, &headers))?;
    Ok((
        no_store_headers(),
        Json(library_get(paths).map_err(|e| error(e, &headers))?),
    ))
}
pub(crate) async fn put(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    body: Result<Json<LibraryStateDto>, JsonRejection>,
) -> Result<(HeaderMap, Json<LibraryStateDto>), ApiErrorResponse> {
    let paths = context
        .paths()
        .ok_or_else(|| error(LibraryError::Unavailable, &headers))?;
    let authenticated = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|value| {
            cookie::Cookie::split_parse(value)
                .filter_map(Result::ok)
                .find(|c| c.name() == crate::session::SESSION_COOKIE_NAME)
        })
        .zip(headers.get("x-csrf-token").and_then(|v| v.to_str().ok()))
        .is_some_and(|(cookie, csrf)| context.sessions().validate(cookie.value(), csrf));
    if !authenticated {
        return Err(api_error_response(
            ApiErrorCode::Forbidden,
            request_id_from_headers(&headers),
        ));
    }
    let Json(state) = body.map_err(|_| error(LibraryError::Invalid, &headers))?;
    Ok((
        no_store_headers(),
        Json(library_put(paths, state).map_err(|e| error(e, &headers))?),
    ))
}

#[derive(serde::Serialize)]
pub(crate) struct WorkspaceDto {
    path: String,
}
pub(crate) async fn workspace(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<WorkspaceDto>), ApiErrorResponse> {
    let paths = context
        .paths()
        .ok_or_else(|| error(LibraryError::Unavailable, &headers))?;
    let path = std::fs::canonicalize(paths.workspace())
        .map_err(|_| error(LibraryError::Unavailable, &headers))?;
    let path = path
        .to_str()
        .ok_or_else(|| error(LibraryError::Unavailable, &headers))?
        .to_owned();
    Ok((no_store_headers(), Json(WorkspaceDto { path })))
}
