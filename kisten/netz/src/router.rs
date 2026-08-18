use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

use crate::{
    bootstrap::{bootstrap_response, BootstrapDto},
    health::{health_handler, no_store_headers},
    ServerContext,
};

pub fn app_router(context: ServerContext) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/bootstrap", get(bootstrap_handler))
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(context)
}

async fn bootstrap_handler(
    State(context): State<ServerContext>,
) -> (HeaderMap, Json<BootstrapDto>) {
    (no_store_headers(), Json(bootstrap_response(&context)))
}
