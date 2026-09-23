use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    Json,
};
use orchester_anwendung::SelfAgentHost;
use orchester_laufzeit::harness::service::{
    SelfAgentActiveModel, SelfAgentModelCatalog, SelfAgentModelChoice, SelfAgentProviderState,
};
use serde::{Deserialize, Serialize};

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::ServerContext,
    health::no_store_headers,
    model_selection::{ModelSelection, MODEL_SELECTION_FIELD_MAX_CHARS},
};

pub const MODEL_CATALOG_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelChoiceDto {
    pub profile: Option<String>,
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub plan_reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ActiveModelDto {
    Configured { choice: ModelChoiceDto },
    Unresolved { field: String, reason: String },
    NotConfigured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderChoiceStateDto {
    Selectable,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderChoiceDto {
    pub id: String,
    pub name: String,
    pub active: bool,
    pub state: ProviderChoiceStateDto,
    pub model: Option<String>,
    pub wire_api: Option<String>,
    pub field: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelProfileDto {
    pub profile: String,
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub plan_reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelCatalogDto {
    pub schema_version: u8,
    pub active: ActiveModelDto,
    pub selected_provider: Option<String>,
    pub providers: Vec<ProviderChoiceDto>,
    pub profiles: Vec<ModelProfileDto>,
}

pub fn model_catalog_response(catalog: &SelfAgentModelCatalog) -> ModelCatalogDto {
    let active_provider = catalog.selected_provider.as_deref().or_else(|| {
        catalog
            .active
            .choice()
            .map(|choice| choice.provider.as_str())
    });
    let active = match &catalog.active {
        SelfAgentActiveModel::Configured(choice) => ActiveModelDto::Configured {
            choice: model_choice(choice),
        },
        SelfAgentActiveModel::Unresolved { path, message } => ActiveModelDto::Unresolved {
            field: path.clone(),
            reason: message.clone(),
        },
        SelfAgentActiveModel::NotConfigured => ActiveModelDto::NotConfigured,
    };
    let providers = catalog
        .providers
        .iter()
        .map(|provider| {
            let (state, model, wire_api, field, reason) = match &provider.state {
                SelfAgentProviderState::Selectable { model, wire_api } => (
                    ProviderChoiceStateDto::Selectable,
                    Some(model.clone()),
                    Some(wire_api.clone()),
                    None,
                    None,
                ),
                SelfAgentProviderState::Unavailable { path, message } => (
                    ProviderChoiceStateDto::Unavailable,
                    None,
                    None,
                    Some(path.clone()),
                    Some(message.clone()),
                ),
            };
            ProviderChoiceDto {
                id: provider.provider.clone(),
                name: provider.provider_name.clone(),
                active: active_provider == Some(provider.provider.as_str()),
                state,
                model,
                wire_api,
                field,
                reason,
            }
        })
        .collect();
    let profiles = catalog.profiles.iter().filter_map(model_profile).collect();

    ModelCatalogDto {
        schema_version: MODEL_CATALOG_SCHEMA_VERSION,
        active,
        selected_provider: catalog.selected_provider.clone(),
        providers,
        profiles,
    }
}

/// The body of a selection request.
///
/// Three optional pieces rather than one enum, because they are independent: a
/// provider can be switched while keeping the model, a profile names both, and
/// the effort is a session-only override on top of either. `effort: null` means
/// the provider's own default, which is a choice rather than an omission.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSelectionRequestDto {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

impl ModelSelectionRequestDto {
    fn validate(&self) -> Result<(), ()> {
        for field in [&self.provider, &self.profile, &self.effort] {
            let Some(value) = field else { continue };
            if value.trim().is_empty()
                || value.chars().count() > MODEL_SELECTION_FIELD_MAX_CHARS
                || value.chars().any(char::is_control)
            {
                return Err(());
            }
        }
        Ok(())
    }

    fn selection(&self) -> ModelSelection {
        ModelSelection {
            provider: self.provider.clone(),
            profile: self.profile.clone(),
            effort: self.effort.clone(),
        }
    }
}

/// The catalog as the next run would see it.
///
/// A fresh host with the reader's selection applied, which is the host the run
/// route builds: without this the screen would report the configuration file's
/// model while the next run used something else.
fn catalog_for(context: &ServerContext, selection: &ModelSelection) -> Option<SelfAgentModelCatalog> {
    let paths = context.paths()?;
    let mut host = SelfAgentHost::for_paths(paths);
    if !selection.is_empty() && selection.apply(&mut host).is_err() {
        // A stored choice the configuration no longer supports: the route cannot
        // describe the next run, and the run route is where the reader sees why,
        // as an error event on the run rather than a silently different model.
        return None;
    }
    host.model_catalog().ok()
}

pub(crate) async fn model_catalog_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<ModelCatalogDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let selection = context.model_selection().read().await;
    let catalog = catalog_for(&context, &selection)
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;
    Ok((no_store_headers(), Json(model_catalog_response(&catalog))))
}

/// Choose the model the following runs use.
///
/// The choice is validated against this workspace's own configuration before it
/// is kept, and answered with the catalog it produces: a reader who picked a
/// model sees what the next run will use without a second request, and a choice
/// that cannot be applied is refused now rather than failing the run it was
/// meant for.
pub(crate) async fn model_selection_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<ModelSelectionRequestDto>, JsonRejection>,
) -> Result<(HeaderMap, Json<ModelCatalogDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) =
        request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    request
        .validate()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;

    let selection = request.selection();
    let paths = context
        .paths()
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;

    let catalog = {
        let mut host = SelfAgentHost::for_paths(paths);
        selection
            .apply(&mut host)
            .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
        host.model_catalog()
            .map_err(|_| api_error_response(ApiErrorCode::Unavailable, request_id))?
    };

    context.model_selection().write(selection).await;
    Ok((no_store_headers(), Json(model_catalog_response(&catalog))))
}

fn model_choice(choice: &SelfAgentModelChoice) -> ModelChoiceDto {
    ModelChoiceDto {
        profile: choice.profile.clone(),
        provider: choice.provider.clone(),
        provider_name: choice.provider_name.clone(),
        model: choice.model.clone(),
        reasoning_effort: choice.reasoning_effort.clone(),
        plan_reasoning_effort: choice.plan_reasoning_effort.clone(),
        service_tier: choice.service_tier.clone(),
    }
}

fn model_profile(choice: &SelfAgentModelChoice) -> Option<ModelProfileDto> {
    Some(ModelProfileDto {
        profile: choice.profile.clone()?,
        provider: choice.provider.clone(),
        provider_name: choice.provider_name.clone(),
        model: choice.model.clone(),
        reasoning_effort: choice.reasoning_effort.clone(),
        plan_reasoning_effort: choice.plan_reasoning_effort.clone(),
        service_tier: choice.service_tier.clone(),
    })
}
