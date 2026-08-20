//! Browser-safe projection of the agents known to the local registry.
//!
//! Registry discovery can establish whether an adapter is installed and
//! launchable, but it cannot inspect arbitrary provider windows. Until the
//! runtime manager reports live handles, activity and managed-session counts
//! therefore remain explicit zeroes rather than guessed process data.

use std::{
    fmt,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::HeaderMap,
    response::Response,
    Json,
};
use orchester_protokoll::{
    AgentActivityState, AgentAvailabilityState, AgentFleetSnapshotDto, AgentFleetStreamFrameDto,
    AgentRuntimeSummaryDto, AgentStatusValidationError, AgentWindowCountSource, Capability,
    TaskKind, AGENT_STATUS_SCHEMA_VERSION,
};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};
use orchester_verzeichnis::Registry;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::broadcast;

use crate::{bootstrap::ServerContext, health::no_store_headers};

pub const AGENT_STATUS_ROUTE_SCHEMA_VERSION: u8 = AGENT_STATUS_SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRuntimeStatusUpdate {
    pub agent_id: String,
    pub activity: AgentActivityState,
    pub active_windows: u64,
    pub active_sessions: u64,
    pub active_runs: u64,
    pub active_subagents: u64,
    pub window_count_source: AgentWindowCountSource,
    pub last_heartbeat_at: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRuntimeStatusError {
    InvalidSnapshot(AgentStatusValidationError),
    InvalidUpdate(&'static str),
    UnknownAgent,
    SequenceExhausted,
    LockPoisoned,
}

impl fmt::Display for AgentRuntimeStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSnapshot(error) => write!(formatter, "invalid agent snapshot: {error}"),
            Self::InvalidUpdate(field) => write!(formatter, "invalid agent update field {field}"),
            Self::UnknownAgent => formatter.write_str("agent is not registered"),
            Self::SequenceExhausted => formatter.write_str("agent status sequence exhausted"),
            Self::LockPoisoned => formatter.write_str("agent status lock is poisoned"),
        }
    }
}

impl std::error::Error for AgentRuntimeStatusError {}

#[derive(Clone)]
pub struct AgentRuntimeStatusStore {
    snapshot: Arc<RwLock<AgentFleetSnapshotDto>>,
    frames: broadcast::Sender<AgentFleetStreamFrameDto>,
}

impl AgentRuntimeStatusStore {
    pub fn new(snapshot: AgentFleetSnapshotDto) -> Result<Self, AgentRuntimeStatusError> {
        snapshot
            .validate()
            .map_err(AgentRuntimeStatusError::InvalidSnapshot)?;
        let (frames, _) = broadcast::channel(32);
        Ok(Self {
            snapshot: Arc::new(RwLock::new(snapshot)),
            frames,
        })
    }

    pub fn snapshot(&self) -> Result<AgentFleetSnapshotDto, AgentRuntimeStatusError> {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| AgentRuntimeStatusError::LockPoisoned)
    }

    pub fn update(&self, update: AgentRuntimeStatusUpdate) -> Result<u64, AgentRuntimeStatusError> {
        validate_update(&update)?;
        let mut guard = self
            .snapshot
            .write()
            .map_err(|_| AgentRuntimeStatusError::LockPoisoned)?;
        let next_sequence = guard
            .sequence
            .checked_add(1)
            .ok_or(AgentRuntimeStatusError::SequenceExhausted)?;
        let mut next = guard.clone();
        let Some(agent) = next
            .agents
            .iter_mut()
            .find(|agent| agent.agent_id == update.agent_id)
        else {
            return Err(AgentRuntimeStatusError::UnknownAgent);
        };
        agent.activity = update.activity;
        agent.active_windows = update.active_windows;
        agent.active_sessions = update.active_sessions;
        agent.active_runs = update.active_runs;
        agent.active_subagents = update.active_subagents;
        agent.window_count_source = update.window_count_source;
        agent.last_heartbeat_at = update.last_heartbeat_at;
        agent.last_error = update.last_error;
        agent.updated_at = update.updated_at.clone();
        next.sequence = next_sequence;
        next.generated_at = update.updated_at;
        next.validate()
            .map_err(AgentRuntimeStatusError::InvalidSnapshot)?;
        let frame = AgentFleetStreamFrameDto::Snapshot {
            snapshot: next.clone(),
        };
        *guard = next;
        drop(guard);
        let _ = self.frames.send(frame);
        Ok(next_sequence)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentFleetStreamFrameDto> {
        self.frames.subscribe()
    }

    pub fn heartbeat(&self, sent_at: impl Into<String>) -> Result<(), AgentRuntimeStatusError> {
        let sent_at = sent_at.into();
        if sent_at.trim().is_empty() {
            return Err(AgentRuntimeStatusError::InvalidUpdate("sent_at"));
        }
        let sequence = self.snapshot()?.sequence;
        let _ = self
            .frames
            .send(AgentFleetStreamFrameDto::Heartbeat { sequence, sent_at });
        Ok(())
    }
}

fn validate_update(update: &AgentRuntimeStatusUpdate) -> Result<(), AgentRuntimeStatusError> {
    if update.agent_id.trim().is_empty() {
        return Err(AgentRuntimeStatusError::InvalidUpdate("agent_id"));
    }
    if update.updated_at.trim().is_empty() {
        return Err(AgentRuntimeStatusError::InvalidUpdate("updated_at"));
    }
    if update
        .last_heartbeat_at
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(AgentRuntimeStatusError::InvalidUpdate("last_heartbeat_at"));
    }
    Ok(())
}

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
        Json(
            context
                .agent_status_store()
                .snapshot()
                .unwrap_or_else(|_| agent_status_response(context.registry())),
        ),
    )
}

pub(crate) async fn agent_status_socket_handler(
    State(context): State<ServerContext>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(move |socket| stream_agent_status(socket, context))
}

async fn stream_agent_status(mut socket: WebSocket, context: ServerContext) {
    let store = context.agent_status_store().clone();
    let mut receiver = store.subscribe();
    let Ok(snapshot) = store.snapshot() else {
        return;
    };
    let mut latest_sequence = snapshot.sequence;
    if send_stream_frame(&mut socket, AgentFleetStreamFrameDto::Snapshot { snapshot })
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    if socket.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
            },
            frame = receiver.recv() => match frame {
                Ok(AgentFleetStreamFrameDto::Snapshot { snapshot }) => {
                    if snapshot.sequence <= latest_sequence {
                        continue;
                    }
                    latest_sequence = snapshot.sequence;
                    if send_stream_frame(
                        &mut socket,
                        AgentFleetStreamFrameDto::Snapshot { snapshot },
                    ).await.is_err() {
                        break;
                    }
                }
                Ok(frame @ AgentFleetStreamFrameDto::Heartbeat { .. }) => {
                    if send_stream_frame(&mut socket, frame).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let Ok(snapshot) = store.snapshot() else {
                        break;
                    };
                    latest_sequence = snapshot.sequence;
                    if send_stream_frame(
                        &mut socket,
                        AgentFleetStreamFrameDto::Snapshot { snapshot },
                    ).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

async fn send_stream_frame(
    socket: &mut WebSocket,
    frame: AgentFleetStreamFrameDto,
) -> Result<(), ()> {
    let text = serde_json::to_string(&frame).map_err(|_| ())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
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
