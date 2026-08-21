//! Redaction-safe runtime status for the browser and desktop clients.
//!
//! This is a projection, not a process inspection API. Providers may report
//! availability independently from activity, while window counts identify the
//! source that produced them (`managed_sessions` in a browser,
//! `tauri_windows` once the desktop registry is available, and
//! `external_processes` for redaction-safe operating-system process counts).

use std::{collections::BTreeSet, fmt};

use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::redact_ui_text;

pub const AGENT_STATUS_SCHEMA_VERSION: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentAvailabilityState {
    Available,
    Unavailable,
    AuthRequired,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActivityState {
    Offline,
    Idle,
    Starting,
    Running,
    WaitingApproval,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWindowCountSource {
    ManagedSessions,
    TauriWindows,
    ExternalProcesses,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeSummaryDto {
    pub agent_id: String,
    pub provider: String,
    pub display_name: String,
    pub icon_key: String,
    pub availability: AgentAvailabilityState,
    pub activity: AgentActivityState,
    pub installed: bool,
    pub configured: bool,
    pub authenticated: bool,
    pub active_windows: u64,
    pub active_sessions: u64,
    pub active_runs: u64,
    pub active_subagents: u64,
    pub window_count_source: AgentWindowCountSource,
    pub last_heartbeat_at: Option<String>,
    pub last_error: Option<String>,
    pub capabilities: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFleetSnapshotDto {
    pub schema_version: u8,
    pub sequence: u64,
    pub generated_at: String,
    pub agents: Vec<AgentRuntimeSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentFleetStreamFrameDto {
    Snapshot { snapshot: AgentFleetSnapshotDto },
    Heartbeat { sequence: u64, sent_at: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatusValidationError {
    UnsupportedSchemaVersion { found: u8, expected: u8 },
    InvalidSequence,
    InvalidField(&'static str),
    DuplicateAgentId(String),
}

impl fmt::Display for AgentStatusValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, expected } => write!(
                formatter,
                "unsupported agent status schema version {found}; expected {expected}"
            ),
            Self::InvalidSequence => {
                formatter.write_str("agent status sequence must be greater than zero")
            }
            Self::InvalidField(field) => write!(formatter, "invalid agent status field {field}"),
            Self::DuplicateAgentId(id) => write!(formatter, "duplicate agent id {id}"),
        }
    }
}

impl std::error::Error for AgentStatusValidationError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRuntimeSummaryWire {
    agent_id: String,
    provider: String,
    display_name: String,
    icon_key: String,
    availability: AgentAvailabilityState,
    activity: AgentActivityState,
    installed: bool,
    configured: bool,
    authenticated: bool,
    active_windows: u64,
    active_sessions: u64,
    active_runs: u64,
    active_subagents: u64,
    window_count_source: AgentWindowCountSource,
    last_heartbeat_at: Option<String>,
    last_error: Option<String>,
    capabilities: Vec<String>,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentFleetSnapshotWire {
    schema_version: u8,
    sequence: u64,
    generated_at: String,
    agents: Vec<AgentRuntimeSummaryWire>,
}

fn validate_text(value: &str, field: &'static str) -> Result<(), AgentStatusValidationError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(AgentStatusValidationError::InvalidField(field));
    }
    Ok(())
}

fn validate_icon_key(value: &str) -> Result<(), AgentStatusValidationError> {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(AgentStatusValidationError::InvalidField("icon_key"));
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit())
        || !chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
    {
        return Err(AgentStatusValidationError::InvalidField("icon_key"));
    }
    Ok(())
}

impl TryFrom<AgentRuntimeSummaryWire> for AgentRuntimeSummaryDto {
    type Error = AgentStatusValidationError;

    fn try_from(value: AgentRuntimeSummaryWire) -> Result<Self, Self::Error> {
        let last_error = value.last_error.map(|error| redact_ui_text(&error));
        let capabilities = value
            .capabilities
            .into_iter()
            .map(|capability| redact_ui_text(&capability))
            .collect();

        let agent = Self {
            agent_id: value.agent_id,
            provider: value.provider,
            display_name: value.display_name,
            icon_key: value.icon_key,
            availability: value.availability,
            activity: value.activity,
            installed: value.installed,
            configured: value.configured,
            authenticated: value.authenticated,
            active_windows: value.active_windows,
            active_sessions: value.active_sessions,
            active_runs: value.active_runs,
            active_subagents: value.active_subagents,
            window_count_source: value.window_count_source,
            last_heartbeat_at: value.last_heartbeat_at,
            last_error,
            capabilities,
            updated_at: value.updated_at,
        };
        agent.validate()?;
        Ok(agent)
    }
}

impl From<&AgentRuntimeSummaryDto> for AgentRuntimeSummaryWire {
    fn from(value: &AgentRuntimeSummaryDto) -> Self {
        Self {
            agent_id: value.agent_id.clone(),
            provider: value.provider.clone(),
            display_name: value.display_name.clone(),
            icon_key: value.icon_key.clone(),
            availability: value.availability,
            activity: value.activity,
            installed: value.installed,
            configured: value.configured,
            authenticated: value.authenticated,
            active_windows: value.active_windows,
            active_sessions: value.active_sessions,
            active_runs: value.active_runs,
            active_subagents: value.active_subagents,
            window_count_source: value.window_count_source,
            last_heartbeat_at: value.last_heartbeat_at.clone(),
            last_error: value.last_error.as_deref().map(redact_ui_text),
            capabilities: value
                .capabilities
                .iter()
                .map(|item| redact_ui_text(item))
                .collect(),
            updated_at: value.updated_at.clone(),
        }
    }
}

impl Serialize for AgentRuntimeSummaryDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(S::Error::custom)?;
        AgentRuntimeSummaryWire::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentRuntimeSummaryDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        AgentRuntimeSummaryWire::deserialize(deserializer)
            .and_then(|wire| wire.try_into().map_err(D::Error::custom))
    }
}

impl TryFrom<AgentFleetSnapshotWire> for AgentFleetSnapshotDto {
    type Error = AgentStatusValidationError;

    fn try_from(value: AgentFleetSnapshotWire) -> Result<Self, Self::Error> {
        let mut agents = Vec::with_capacity(value.agents.len());
        for wire in value.agents {
            agents.push(AgentRuntimeSummaryDto::try_from(wire)?);
        }
        let snapshot = Self {
            schema_version: value.schema_version,
            sequence: value.sequence,
            generated_at: value.generated_at,
            agents,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

impl From<&AgentFleetSnapshotDto> for AgentFleetSnapshotWire {
    fn from(value: &AgentFleetSnapshotDto) -> Self {
        Self {
            schema_version: value.schema_version,
            sequence: value.sequence,
            generated_at: value.generated_at.clone(),
            agents: value
                .agents
                .iter()
                .map(AgentRuntimeSummaryWire::from)
                .collect(),
        }
    }
}

impl Serialize for AgentFleetSnapshotDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(S::Error::custom)?;
        let wire = AgentFleetSnapshotWire::from(self);
        wire.serialize(serializer)
    }
}

impl AgentRuntimeSummaryDto {
    pub fn validate(&self) -> Result<(), AgentStatusValidationError> {
        validate_text(&self.agent_id, "agent_id")?;
        validate_text(&self.provider, "provider")?;
        validate_text(&self.display_name, "display_name")?;
        validate_icon_key(&self.icon_key)?;
        validate_text(&self.updated_at, "updated_at")?;
        if let Some(heartbeat) = &self.last_heartbeat_at {
            validate_text(heartbeat, "last_heartbeat_at")?;
        }
        for capability in &self.capabilities {
            validate_text(capability, "capabilities")?;
            if capability.len() > 80 || capability.contains(['/', '\\']) {
                return Err(AgentStatusValidationError::InvalidField("capabilities"));
            }
        }
        Ok(())
    }
}

impl AgentFleetSnapshotDto {
    pub fn validate(&self) -> Result<(), AgentStatusValidationError> {
        if self.schema_version != AGENT_STATUS_SCHEMA_VERSION {
            return Err(AgentStatusValidationError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: AGENT_STATUS_SCHEMA_VERSION,
            });
        }
        if self.sequence == 0 {
            return Err(AgentStatusValidationError::InvalidSequence);
        }
        validate_text(&self.generated_at, "generated_at")?;
        let mut ids = BTreeSet::new();
        for agent in &self.agents {
            agent.validate()?;
            if !ids.insert(agent.agent_id.clone()) {
                return Err(AgentStatusValidationError::DuplicateAgentId(
                    agent.agent_id.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for AgentFleetSnapshotDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        AgentFleetSnapshotWire::deserialize(deserializer)
            .and_then(|wire| wire.try_into().map_err(D::Error::custom))
    }
}
