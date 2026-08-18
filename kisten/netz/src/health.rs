use axum::{
    http::{header, HeaderMap},
    routing::get,
    Json, Router,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthDto {
    pub status: String,
    pub service: String,
    pub version: String,
    pub schema_version: u8,
}

pub fn health_response() -> HealthDto {
    HealthDto {
        status: "ok".to_owned(),
        service: "orchester".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: 1,
    }
}

pub async fn health_handler() -> (HeaderMap, Json<HealthDto>) {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header"),
    );
    (headers, Json(health_response()))
}

pub fn app_router() -> Router {
    Router::new().route("/api/v1/health", get(health_handler))
}
