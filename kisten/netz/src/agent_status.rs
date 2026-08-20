//! Browser-safe projection of the agents known to the local registry.
//!
//! Registry discovery can establish whether an adapter is installed and
//! launchable, but it cannot inspect arbitrary provider windows. Until the
//! runtime manager reports live handles, activity and managed-session counts
//! therefore remain explicit zeroes rather than guessed process data.

use axum::{extract::State, http::HeaderMap, Json};
use orchester_protokoll::{
    AgentActivityState, AgentAvailabilityState, AgentFleetSnapshotDto, AgentRuntimeSummaryDto,
    AgentWindowCountSource, Capability, TaskKind, AGENT_STATUS_SCHEMA_VERSION,
};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};
use orchester_verzeichnis::Registry;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{bootstrap::ServerContext, health::no_store_headers};

pub const AGENT_STATUS_ROUTE_SCHEMA_VERSION: u8 = AGENT_STATUS_SCHEMA_VERSION;

/// Build one redaction-safe snapshot from the registry.
pub fn agent_status_response(registry: &Registry) -> AgentFleetSnapshotDto {
    let now = now_rfc3339();
    let capabilities = registry.list();
    let availability = registry.availability();
    let agents = capabilities
        .into_iter()
        .map(|capability| {
            let check = availability
                .iter()
                .find(|item| item.name == capability.name);
            runtime_summary(capability, check, &now)
        })
        .collect();

    AgentFleetSnapshotDto {
        schema_version: AGENT_STATUS_ROUTE_SCHEMA_VERSION,
        sequence: 1,
        generated_at: now.clone(),
        agents,
    }
}

pub(crate) async fn agent_status_handler(
    State(context): State<ServerContext>,
) -> (HeaderMap, Json<AgentFleetSnapshotDto>) {
    (
        no_store_headers(),
        Json(agent_status_response(context.registry())),
    )
}

fn runtime_summary(
    capability: Capability,
    availability: Option<&AdapterAvailability>,
    now: &str,
) -> AgentRuntimeSummaryDto {
    let status = availability.map(|item| item.status);
    let (availability_state, activity, installed, configured, authenticated, last_error) =
        match status {
            Some(AvailabilityStatus::Available) => (
                AgentAvailabilityState::Available,
                AgentActivityState::Idle,
                true,
                true,
                capability.name == "mock",
                None,
            ),
            Some(AvailabilityStatus::Missing) => (
                AgentAvailabilityState::Unavailable,
                AgentActivityState::Offline,
                false,
                true,
                false,
                Some("Agent executable is unavailable".to_owned()),
            ),
            Some(AvailabilityStatus::Unknown) | None => (
                AgentAvailabilityState::Error,
                AgentActivityState::Offline,
                true,
                false,
                false,
                Some("Agent availability is unknown".to_owned()),
            ),
        };

    AgentRuntimeSummaryDto {
        agent_id: capability.name.clone(),
        provider: capability.name.clone(),
        display_name: display_name(&capability.name),
        icon_key: icon_key(&capability.name),
        availability: availability_state,
        activity,
        installed,
        configured,
        authenticated,
        active_windows: 0,
        active_sessions: 0,
        active_runs: 0,
        active_subagents: 0,
        window_count_source: AgentWindowCountSource::ManagedSessions,
        last_heartbeat_at: None,
        last_error,
        capabilities: capability_labels(&capability),
        updated_at: now.to_owned(),
    }
}

fn capability_labels(capability: &Capability) -> Vec<String> {
    let mut labels = capability
        .kinds
        .iter()
        .map(task_kind_name)
        .collect::<Vec<_>>();
    if capability.streaming {
        labels.push("streaming".to_owned());
    }
    if capability.supports_resume {
        labels.push("resume".to_owned());
    }
    labels
}

fn task_kind_name(kind: &TaskKind) -> String {
    match kind {
        TaskKind::Code => "code".to_owned(),
        TaskKind::Review => "review".to_owned(),
        TaskKind::Chat => "chat".to_owned(),
        TaskKind::Browser => "browser".to_owned(),
        TaskKind::Custom(_) => "custom".to_owned(),
    }
}

fn icon_key(provider: &str) -> String {
    let normalized = provider.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return "generic".to_owned();
    }
    if normalized.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || character == '-'
            || character == '_'
    }) {
        normalized
    } else {
        "generic".to_owned()
    }
}

fn display_name(provider: &str) -> String {
    match provider {
        "claude" => "Claude Code".to_owned(),
        "codex" => "Codex".to_owned(),
        "deepseek" => "DeepSeek".to_owned(),
        "mock" => "Mock".to_owned(),
        "opencode" => "OpenCode".to_owned(),
        other => other
            .split(['-', '_'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
