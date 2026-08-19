use axum::{extract::State, http::HeaderMap, Json};
use orchester_protokoll::TaskKind;
use orchester_verzeichnis::Registry;
use serde::Serialize;

use crate::{bootstrap::ServerContext, health::no_store_headers};

pub const AGENT_CATALOG_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentAvailabilityDto {
    Available,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentSummaryDto {
    pub id: String,
    pub name: String,
    pub task_kinds: Vec<String>,
    pub supports_resume: bool,
    pub streaming: bool,
    pub availability: AgentAvailabilityDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentCatalogDto {
    pub schema_version: u8,
    pub agents: Vec<AgentSummaryDto>,
}

pub fn agent_catalog_response(registry: &Registry) -> AgentCatalogDto {
    let capabilities = registry.list();
    let availability = registry.availability();
    let agents = capabilities
        .into_iter()
        .map(|capability| AgentSummaryDto {
            id: capability.name.clone(),
            name: capability.name.clone(),
            task_kinds: capability.kinds.iter().map(task_kind_name).collect(),
            supports_resume: capability.supports_resume,
            streaming: capability.streaming,
            availability: availability
                .iter()
                .find(|check| check.name == capability.name)
                .map(|check| match check.status {
                    orchester_vertrag::AvailabilityStatus::Available => {
                        AgentAvailabilityDto::Available
                    }
                    orchester_vertrag::AvailabilityStatus::Missing => AgentAvailabilityDto::Missing,
                    orchester_vertrag::AvailabilityStatus::Unknown => AgentAvailabilityDto::Unknown,
                })
                .unwrap_or(AgentAvailabilityDto::Unknown),
        })
        .collect();

    AgentCatalogDto {
        schema_version: AGENT_CATALOG_SCHEMA_VERSION,
        agents,
    }
}

pub(crate) async fn agent_catalog_handler(
    State(context): State<ServerContext>,
) -> (HeaderMap, Json<AgentCatalogDto>) {
    (
        no_store_headers(),
        Json(agent_catalog_response(context.registry())),
    )
}

fn task_kind_name(kind: &TaskKind) -> String {
    match kind {
        TaskKind::Code => "code".to_owned(),
        TaskKind::Review => "review".to_owned(),
        TaskKind::Chat => "chat".to_owned(),
        TaskKind::Browser => "browser".to_owned(),
        TaskKind::Custom(value) => format!("custom:{value}"),
    }
}
