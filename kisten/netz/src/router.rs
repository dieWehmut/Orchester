use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};

use crate::{
    bootstrap::{bootstrap_response, BootstrapDto},
    health::{health_handler, no_store_headers},
    ServerContext,
};

pub fn app_router(context: ServerContext) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/bootstrap", get(bootstrap_handler))
        .with_state(context)
}

async fn bootstrap_handler(
    State(context): State<ServerContext>,
) -> (HeaderMap, Json<BootstrapDto>) {
    (no_store_headers(), Json(bootstrap_response(&context)))
}
