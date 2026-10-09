//! Local, receipt-owned plugin management. Package output never crosses HTTP.
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use orchester_verzeichnis::{
    management::{self, InstallError, RemoveError},
    PluginOrigin,
};
use serde::{Deserialize, Serialize};

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    ServerContext,
};

const CATALOG: [(&str, &str); 3] = [
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("opencode", "OpenCode"),
];

#[derive(Serialize)]
pub struct PluginDto {
    name: String,
    display_name: String,
    package_name: String,
    version: String,
    installed: bool,
    origin: Option<&'static str>,
    removable: bool,
}

#[derive(Serialize)]
pub struct PluginCatalogDto {
    schema_version: u8,
    plugins: Vec<PluginDto>,
}

fn catalog(context: &ServerContext) -> PluginCatalogDto {
    // Include changes made by the CLI since the last request.
    context.refresh_registry();
    let registry = context.registry();
    let mut entries = std::collections::BTreeMap::new();
    for (name, display) in CATALOG {
        entries.insert(
            name.to_owned(),
            PluginDto {
                name: name.into(),
                display_name: display.into(),
                package_name: format!("@orchester/{name}"),
                version: env!("CARGO_PKG_VERSION").into(),
                installed: false,
                origin: None,
                removable: false,
            },
        );
    }
    for plugin in registry.plugins() {
        let info = plugin.info();
        let managed = plugin.origin() == PluginOrigin::Managed;
        entries.insert(
            info.name().into(),
            PluginDto {
                name: info.name().into(),
                display_name: info.display_name().into(),
                package_name: info.package_name().into(),
                version: info.version().into(),
                installed: true,
                origin: Some(if managed { "managed" } else { "project" }),
                removable: managed,
            },
        );
    }
    PluginCatalogDto {
        schema_version: 1,
        plugins: entries.into_values().collect(),
    }
}

pub(crate) async fn list(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<PluginCatalogDto>), ApiErrorResponse> {
    if context.paths().is_none() {
        return Err(api_error_response(
            ApiErrorCode::Unavailable,
            request_id_from_headers(&headers),
        ));
    }
    let result = tokio::task::spawn_blocking(move || catalog(&context))
        .await
        .map_err(|_| {
            api_error_response(ApiErrorCode::Internal, request_id_from_headers(&headers))
        })?;
    Ok((no_store_headers(), Json(result)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstallRequest {
    name: String,
}

pub(crate) async fn install(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<InstallRequest>, JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<PluginCatalogDto>), ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(&headers));
    let Json(request) = request.map_err(|_| fail(ApiErrorCode::BadRequest))?;
    if !CATALOG.iter().any(|(name, _)| *name == request.name) {
        return Err(fail(ApiErrorCode::ValidationFailed));
    }
    let home = context
        .paths()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?
        .home()
        .to_owned();
    let guard = context
        .plugin_mutation()
        .try_lock_owned()
        .map_err(|_| fail(ApiErrorCode::Conflict))?;
    let result = tokio::task::spawn_blocking(move || {
        let _guard = guard;
        management::install(&home, &request.name)
    })
    .await
    .map_err(|_| fail(ApiErrorCode::Internal))?;
    result.map_err(|error| {
        fail(match error {
            InstallError::AlreadyInstalled => ApiErrorCode::Conflict,
            InstallError::InvalidName => ApiErrorCode::ValidationFailed,
            InstallError::NpmUnavailable | InstallError::NpmTimedOut | InstallError::NpmFailed => {
                ApiErrorCode::Unavailable
            }
            _ => ApiErrorCode::RuntimeError,
        })
    })?;
    let (_, response) = list(State(context), headers).await?;
    Ok((StatusCode::CREATED, no_store_headers(), response))
}

pub(crate) async fn remove(
    State(context): State<ServerContext>,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<PluginCatalogDto>), ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(&headers));
    let home = context
        .paths()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?
        .home()
        .to_owned();
    let guard = context
        .plugin_mutation()
        .try_lock_owned()
        .map_err(|_| fail(ApiErrorCode::Conflict))?;
    // A project override is read-only even if a hidden managed copy also exists.
    context.refresh_registry();
    let registry = context.registry();
    if registry
        .plugins()
        .iter()
        .any(|p| p.info().name() == name && p.origin() == PluginOrigin::Project)
    {
        return Err(fail(ApiErrorCode::Conflict));
    }
    tokio::task::spawn_blocking(move || {
        let _guard = guard;
        management::remove(&home, &name)
    })
    .await
    .map_err(|_| fail(ApiErrorCode::Internal))?
    .map_err(|error| {
        fail(match error {
            RemoveError::InvalidName => ApiErrorCode::ValidationFailed,
            RemoveError::OwnershipUnverified => ApiErrorCode::Conflict,
            _ => ApiErrorCode::RuntimeError,
        })
    })?;
    list(State(context), headers).await
}
