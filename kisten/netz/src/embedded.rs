//! Process-owned desktop server. Only the generated loopback origin can access
//! it, and a one-time launch fragment establishes the existing HTTP session.

use std::{io, net::SocketAddr, path::PathBuf, time::Duration};

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use cookie::Cookie;
use orchester_anwendung::OrchesterPaths;
use tokio::task::JoinHandle;

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode},
    app_router_with_static_assets, bind_listener, wait_for_shutdown, ServerConfig, ServerContext,
    ServerControl, StaticAssets, SESSION_COOKIE_NAME,
};

/// Own this handle for exactly as long as the desktop window is alive.
/// `shutdown` drains active requests; dropping also signals every subscriber.
pub struct EmbeddedServer {
    address: SocketAddr,
    origin: String,
    launch_url: String,
    control: ServerControl,
    task: Option<JoinHandle<io::Result<()>>>,
}

impl std::fmt::Debug for EmbeddedServer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedServer")
            .field("address", &self.address)
            .field("state", &self.control.state())
            .finish_non_exhaustive()
    }
}

impl EmbeddedServer {
    pub async fn start(paths: OrchesterPaths, assets: PathBuf) -> io::Result<Self> {
        let assets = tokio::fs::canonicalize(assets).await?;
        if !assets.join("index.html").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Bundled web/index.html is missing",
            ));
        }
        let listener = bind_listener(&ServerConfig::default()).map_err(|error| {
            io::Error::other(format!("Failed to bind desktop loopback server: {error:?}"))
        })?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let origin = format!("http://{address}");
        let control = ServerControl::new();
        let context = ServerContext::new(Some(paths), control.clone());
        let mut random = [0; 32];
        getrandom::fill(&mut random)
            .map_err(|_| io::Error::other("Failed to generate desktop launch token"))?;
        let token: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        context
            .provision_fragment_token(&token)
            .map_err(io::Error::other)?;
        let launch_url = format!("{origin}/#fragment_token={token}");
        let policy = DesktopPolicy {
            host: address.to_string(),
            origin: origin.clone(),
            csp: HeaderValue::from_str(&format!(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self' ws://{address} ipc: http://ipc.localhost; object-src 'none'; base-uri 'self'; form-action 'self'; frame-src 'none'; frame-ancestors 'none'; navigate-to 'self'"
            )).map_err(io::Error::other)?,
            context: context.clone(),
        };
        let router =
            app_router_with_static_assets(context.clone(), StaticAssets::Directory(assets))
                .layer(middleware::from_fn_with_state(policy, desktop_boundary));
        let listener = tokio::net::TcpListener::from_std(listener)?;
        control.start().map_err(|error| {
            io::Error::other(format!("Failed to start desktop server: {error:?}"))
        })?;
        context.start_agent_process_monitor();
        let shutdown = control.subscribe_shutdown();
        let task_control = control.clone();
        let task = tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(wait_for_shutdown(shutdown))
                .await;
            let _ = task_control.request_shutdown();
            let _ = task_control.complete_shutdown();
            result
        });
        Ok(Self {
            address,
            origin,
            launch_url,
            control,
            task: Some(task),
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Contains a secret; only pass it directly to the first desktop navigation.
    pub fn launch_url(&self) -> &str {
        &self.launch_url
    }

    pub async fn shutdown(&mut self) -> io::Result<()> {
        let _ = self.control.request_shutdown();
        let Some(mut task) = self.task.take() else {
            return Ok(());
        };
        match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
            Ok(result) => result.map_err(io::Error::other)?,
            Err(_) => {
                task.abort();
                let _ = task.await;
                let _ = self.control.complete_shutdown();
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Desktop server shutdown timed out",
                ))
            }
        }
    }
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        let _ = self.control.request_shutdown();
    }
}

#[derive(Clone)]
struct DesktopPolicy {
    host: String,
    origin: String,
    csp: HeaderValue,
    context: ServerContext,
}

async fn desktop_boundary(
    State(policy): State<DesktopPolicy>,
    request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let websocket = headers.contains_key(header::UPGRADE);
    let mutation = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    // Validate Host even when Origin is absent, preventing DNS rebinding. The
    // browser must send our exact Origin on upgrades and state-changing calls.
    let invalid_origin = (headers.contains_key(header::ORIGIN)
        && origin != Some(policy.origin.as_str()))
        || ((websocket || mutation) && origin != Some(policy.origin.as_str()));
    let cross_site = headers
        .get("sec-fetch-site")
        .is_some_and(|value| value == "cross-site" || value == "same-site");
    let mut response = if host != Some(policy.host.as_str()) || invalid_origin || cross_site {
        StatusCode::FORBIDDEN.into_response()
    } else {
        let path = request.uri().path();
        let api = path == "/api/v1" || path.starts_with("/api/v1/");
        let bootstrap = (request.method() == Method::GET
            && matches!(path, "/api/v1/health" | "/api/v1/bootstrap"))
            || (request.method() == Method::POST && path == "/api/v1/auth/fragment");
        let session = headers
            .get(header::COOKIE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                Cookie::split_parse(value)
                    .filter_map(Result::ok)
                    .find(|cookie| cookie.name() == SESSION_COOKIE_NAME)
                    .map(|cookie| cookie.value().to_owned())
            });
        let authenticated = session
            .as_deref()
            .is_some_and(|cookie| policy.context.sessions().validate_cookie(cookie));
        let csrf_valid = session
            .as_deref()
            .zip(
                headers
                    .get("x-csrf-token")
                    .and_then(|value| value.to_str().ok()),
            )
            .is_some_and(|(cookie, csrf)| policy.context.sessions().validate(cookie, csrf));
        if api && !bootstrap && !authenticated {
            api_error_response(ApiErrorCode::Unauthorized, request_id_from_headers(headers))
                .into_response()
        } else if api && !bootstrap && mutation && !csrf_valid {
            api_error_response(ApiErrorCode::Forbidden, request_id_from_headers(headers))
                .into_response()
        } else {
            next.run(request).await
        }
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, policy.csp);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
