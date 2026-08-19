use orchester_laufzeit::harness::service::{
    SelfAgentActiveModel, SelfAgentModelCatalog, SelfAgentModelChoice, SelfAgentProviderState,
};
use serde::Serialize;

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
