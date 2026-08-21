use axum::{
    extract::State,
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use tower::ServiceBuilder;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};

#[cfg(feature = "static-files")]
use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
#[cfg(feature = "static-files")]
use std::{
    convert::Infallible,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
#[cfg(feature = "static-files")]
use tower::Service;

use crate::{
    agent_catalog::agent_catalog_handler,
    agent_status::{agent_status_handler, agent_status_socket_handler},
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::{bootstrap_response, BootstrapDto},
    config::StaticAssets,
    health::{health_handler, no_store_headers},
    model_catalog::model_catalog_handler,
    session::{fragment_exchange_handler, session_bootstrap_handler, session_revoke_handler},
    session_history::{session_detail_handler, session_list_handler},
    ServerContext,
};

pub fn app_router(context: ServerContext) -> Router {
    app_router_with_static_assets(context, StaticAssets::Disabled)
}

pub fn app_router_with_static_assets(
    context: ServerContext,
    static_assets: StaticAssets,
) -> Router {
    let router = Router::new()
        .nest("/api/v1", api_router())
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .with_state(context);

    match static_assets {
        StaticAssets::Disabled => router.fallback(not_found_handler),
        #[cfg(feature = "static-files")]
        StaticAssets::Directory(directory) => {
            router.fallback_service(static_web_service(directory))
        }
        #[cfg(not(feature = "static-files"))]
        StaticAssets::Directory(_) => router.fallback(not_found_handler),
    }
}

fn api_router() -> Router<ServerContext> {
    Router::new()
        .route("/health", get(health_handler))
        .route("/bootstrap", get(bootstrap_handler))
        .route("/agents", get(agent_catalog_handler))
        .route("/agents/status", get(agent_status_handler))
        .route("/agents/status/ws", get(agent_status_socket_handler))
        .route("/models", get(model_catalog_handler))
        .route("/sessions", get(session_list_handler))
        .route("/sessions/{id}", get(session_detail_handler))
        .route("/session", get(session_bootstrap_handler))
        .route("/session/revoke", post(session_revoke_handler))
        .route("/auth/fragment", post(fragment_exchange_handler))
        .fallback(not_found_handler)
        .method_not_allowed_fallback(method_not_allowed_handler)
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

#[cfg(feature = "static-files")]
#[derive(Clone)]
struct StaticWebService {
    root: Arc<PathBuf>,
}

#[cfg(feature = "static-files")]
fn static_web_service(directory: PathBuf) -> StaticWebService {
    StaticWebService {
        root: Arc::new(directory),
    }
}

#[cfg(feature = "static-files")]
impl Service<Request> for StaticWebService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let root = Arc::clone(&self.root);
        Box::pin(async move { Ok(serve_static_request(root, request).await) })
    }
}

#[cfg(feature = "static-files")]
async fn serve_static_request(root: Arc<PathBuf>, request: Request) -> Response {
    use axum::http::{header, Method};

    let request_path = request.uri().path();
    if request_path.starts_with("/api/v1") {
        return api_error_response(
            ApiErrorCode::NotFound,
            request_id_from_headers(request.headers()),
        )
        .into_response();
    }

    if request.method() != Method::GET && request.method() != Method::HEAD {
        let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
        response
            .headers_mut()
            .insert(header::ALLOW, header::HeaderValue::from_static("GET, HEAD"));
        return with_static_cache_policy(response);
    }

    let Some(relative_path) = safe_relative_path(request_path) else {
        return static_not_found_response();
    };
    let Ok(canonical_root) = tokio::fs::canonicalize(root.as_ref()).await else {
        return static_not_found_response();
    };

    let mut candidate = canonical_root.join(relative_path);
    match tokio::fs::metadata(&candidate).await {
        Ok(metadata) if metadata.is_dir() => {
            candidate.push("index.html");
        }
        Ok(_) => {}
        Err(_) => {
            candidate = canonical_root.join("index.html");
        }
    }

    let Ok(canonical_candidate) = tokio::fs::canonicalize(&candidate).await else {
        return static_not_found_response();
    };
    if !canonical_candidate.starts_with(&canonical_root) {
        return static_not_found_response();
    }
    let Ok(metadata) = tokio::fs::metadata(&canonical_candidate).await else {
        return static_not_found_response();
    };
    if metadata.is_dir() {
        return static_not_found_response();
    }

    let content_type = mime_guess::from_path(&canonical_candidate)
        .first_or_octet_stream()
        .to_string();
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        match tokio::fs::read(&canonical_candidate).await {
            Ok(bytes) => Body::from(bytes),
            Err(_) => return static_not_found_response(),
        }
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .body(body)
        .expect("static response headers are valid")
        .into_response();
    with_static_cache_policy(response)
}

#[cfg(feature = "static-files")]
fn safe_relative_path(path: &str) -> Option<PathBuf> {
    let decoded = decode_uri_path(path)?;
    // Do not reinterpret a second layer of percent encoding. A path such as
    // `%252e%252e/secret` must not become a traversal after a later proxy or
    // filesystem layer decodes it again.
    if decoded.contains('%') {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in decoded.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".."
            || component.contains('\\')
            || component.contains('\0')
            || component.contains(':')
        {
            return None;
        }
        relative.push(component);
    }
    Some(relative)
}

#[cfg(feature = "static-files")]
fn decode_uri_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(feature = "static-files")]
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(feature = "static-files")]
fn static_not_found_response() -> Response {
    with_static_cache_policy(StatusCode::NOT_FOUND.into_response())
}

#[cfg(feature = "static-files")]
fn with_static_cache_policy(mut response: Response) -> Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response
}
