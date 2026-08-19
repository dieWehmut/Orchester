use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

use crate::{
    agent_catalog::agent_catalog_handler,
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::{bootstrap_response, BootstrapDto},
    health::{health_handler, no_store_headers},
    model_catalog::model_catalog_handler,
    session::{fragment_exchange_handler, session_bootstrap_handler, session_revoke_handler},
    session_history::{session_detail_handler, session_list_handler},
    ServerContext,
};

pub fn app_router(context: ServerContext) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/bootstrap", get(bootstrap_handler))
        .route("/api/v1/agents", get(agent_catalog_handler))
        .route("/api/v1/models", get(model_catalog_handler))
        .route("/api/v1/sessions", get(session_list_handler))
        .route("/api/v1/sessions/{id}", get(session_detail_handler))
        .route("/api/v1/session", get(session_bootstrap_handler))
        .route("/api/v1/session/revoke", post(session_revoke_handler))
        .route("/api/v1/auth/fragment", post(fragment_exchange_handler))
        .fallback(not_found_handler)
        .method_not_allowed_fallback(method_not_allowed_handler)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(context)
}

async fn not_found_handler(headers: HeaderMap) -> ApiErrorResponse {
    api_error_response(ApiErrorCode::NotFound, request_id_from_headers(&headers))
}

async fn method_not_allowed_handler(headers: HeaderMap) -> ApiErrorResponse {
    api_error_response(
        ApiErrorCode::MethodNotAllowed,
        request_id_from_headers(&headers),
    )
}

async fn bootstrap_handler(
    State(context): State<ServerContext>,
) -> (HeaderMap, Json<BootstrapDto>) {
    (no_store_headers(), Json(bootstrap_response(&context)))
}
