use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock},
};

use getrandom::fill as fill_random;
use orchester_protokoll::{
    EventId, RunId, StopReason, UiApprovalRequest, UiEventEnvelope, UiEventKind, Usage,
    UI_SCHEMA_VERSION,
};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::run_contract::{
    state_from_stop_reason, RunReplayResponseDto, RunSnapshotDto, RunStateDto, RunStreamFrameDto,
    RunSummaryDto, RUN_REPLAY_DEFAULT_LIMIT, RUN_REPLAY_MAX_LIMIT,
};

const DEFAULT_RETENTION: usize = 256;

#[derive(Clone)]
pub(crate) struct RunRegistry {
    inner: Arc<RegistryInner>,
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_RETENTION)
    }
}

struct RegistryInner {
    retention: usize,
    runs: RwLock<HashMap<RunId, Arc<RunEntry>>>,
}

struct RunEntry {
    id: RunId,
    retention: usize,
    state: Mutex<RunMutable>,
    frames: broadcast::Sender<RunStreamFrameDto>,
    cancellation: CancellationToken,
}

struct RunMutable {
    events: VecDeque<UiEventEnvelope>,
    pending_approvals: Vec<UiApprovalRequest>,
    state: RunStateDto,
    usage: Usage,
    oldest_sequence: u64,
    latest_sequence: u64,
    next_sequence: u64,
    updated_at: String,
    stopped: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum RunRegistryError {
    #[error("run was not found")]
    NotFound,
    #[error("run event retention no longer covers sequence {requested_after_sequence}")]
    RetentionExceeded {
        requested_after_sequence: u64,
        oldest_sequence: u64,
        latest_sequence: u64,
    },
    #[error("run replay limit is invalid")]
    InvalidLimit,
    #[error("run event cannot be appended after termination")]
    Terminal,
    #[error("run event id generation failed")]
    Entropy,
    #[error("run event violates the UI protocol: {0}")]
    InvalidEvent(String),
    #[error("run registry lock is poisoned")]
    LockPoisoned,
}

#[derive(Clone)]
pub(crate) struct RunHandle {
    entry: Arc<RunEntry>,
}

impl RunRegistry {
    pub(crate) fn new(retention: usize) -> Self {
        let retention = if retention == 0 {
            DEFAULT_RETENTION
        } else {
            retention.min(4_096)
        };
        Self {
            inner: Arc::new(RegistryInner {
                retention,
                runs: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub(crate) fn create(&self) -> Result<RunHandle, RunRegistryError> {
        for _ in 0..8 {
            let id = RunId::from(random_id("run")?);
            let (frames, _) = broadcast::channel(self.inner.retention.max(32));
            let entry = Arc::new(RunEntry {
                id: id.clone(),
                retention: self.inner.retention,
                state: Mutex::new(RunMutable::new()),
                frames,
                cancellation: CancellationToken::new(),
            });
            let mut runs = self
                .inner
                .runs
                .write()
                .map_err(|_| RunRegistryError::LockPoisoned)?;
            if runs.contains_key(&id) {
                continue;
            }
            runs.insert(id, Arc::clone(&entry));
            return Ok(RunHandle { entry });
        }
        Err(RunRegistryError::Entropy)
    }

    pub(crate) fn get(&self, run_id: &RunId) -> Result<RunHandle, RunRegistryError> {
        self.inner
            .runs
            .read()
            .map_err(|_| RunRegistryError::LockPoisoned)?
            .get(run_id)
            .cloned()
            .map(|entry| RunHandle { entry })
            .ok_or(RunRegistryError::NotFound)
    }
}

impl RunHandle {
    pub(crate) fn id(&self) -> &RunId {
        &self.entry.id
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.entry.cancellation.clone()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<RunStreamFrameDto> {
        self.entry.frames.subscribe()
    }

    pub(crate) async fn append(
        &self,
        kind: UiEventKind,
    ) -> Result<UiEventEnvelope, RunRegistryError> {
        let mut state = self.entry.state.lock().await;
        if state.stopped {
            return Err(RunRegistryError::Terminal);
        }

        let sequence = state.next_sequence;
        let occurred_at = now_rfc3339();
        let call_id = match &kind {
            UiEventKind::ToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        };
        let event = UiEventEnvelope {
            schema_version: UI_SCHEMA_VERSION,
            event_id: EventId::from(random_id("evt")?),
            run_id: self.entry.id.clone(),
            turn_id: None,
            call_id,
            sequence,
            occurred_at: occurred_at.clone(),
            kind,
        };
        event
            .validate()
            .map_err(|error| RunRegistryError::InvalidEvent(error.to_string()))?;
        apply_kind(&mut state, &event.kind);
        state.events.push_back(event.clone());
        state.latest_sequence = sequence;
        state.next_sequence = sequence.saturating_add(1);
        state.updated_at = occurred_at;
        while state.events.len() > self.retention() {
            state.events.pop_front();
        }
        state.oldest_sequence = state
            .events
            .front()
            .map(|event| event.sequence)
            .unwrap_or(state.next_sequence);
        let frame = RunStreamFrameDto::Event {
            event: Box::new(event.clone()),
        };
        drop(state);
        let _ = self.entry.frames.send(frame);
        Ok(event)
    }

    pub(crate) async fn snapshot(&self) -> Result<RunSnapshotDto, RunRegistryError> {
        let state = self.entry.state.lock().await;
        Ok(snapshot_from_state(self.id().clone(), &state))
    }

    pub(crate) async fn replay(
        &self,
        after_sequence: u64,
        limit: Option<u32>,
    ) -> Result<RunReplayResponseDto, RunRegistryError> {
        let limit = limit.unwrap_or(RUN_REPLAY_DEFAULT_LIMIT);
        if limit == 0 || limit > RUN_REPLAY_MAX_LIMIT {
            return Err(RunRegistryError::InvalidLimit);
        }
        let state = self.entry.state.lock().await;
        if state.latest_sequence > 0
            && state.oldest_sequence > 1
            && after_sequence.saturating_add(1) < state.oldest_sequence
        {
            return Err(RunRegistryError::RetentionExceeded {
                requested_after_sequence: after_sequence,
                oldest_sequence: state.oldest_sequence,
                latest_sequence: state.latest_sequence,
            });
        }
        let mut events = state
            .events
            .iter()
            .filter(|event| event.sequence > after_sequence)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = events.len() > limit as usize;
        events.truncate(limit as usize);
        Ok(RunReplayResponseDto {
            run_id: self.id().clone(),
            first_sequence: events.first().map(|event| event.sequence),
            last_sequence: events.last().map(|event| event.sequence),
            events,
            has_more,
        })
    }

    pub(crate) async fn cancel(&self) -> Result<RunSummaryDto, RunRegistryError> {
        self.entry.cancellation.cancel();
        let stopped = self.entry.state.lock().await.stopped;
        if !stopped {
            let _ = self
                .append(UiEventKind::RunStopped {
                    reason: StopReason::Cancelled,
                })
                .await;
        }
        self.summary().await
    }

    pub(crate) async fn summary(&self) -> Result<RunSummaryDto, RunRegistryError> {
        let state = self.entry.state.lock().await;
        Ok(RunSummaryDto {
            run_id: self.id().clone(),
            usage: state.usage,
            stopped: state.stopped,
        })
    }

    fn retention(&self) -> usize {
        self.entry.retention
    }
}

impl RunMutable {
    fn new() -> Self {
        Self {
            events: VecDeque::new(),
            pending_approvals: Vec::new(),
            state: RunStateDto::Created,
            usage: Usage::default(),
            oldest_sequence: 1,
            latest_sequence: 0,
            next_sequence: 1,
            updated_at: now_rfc3339(),
            stopped: false,
        }
    }
}

fn snapshot_from_state(run_id: RunId, state: &RunMutable) -> RunSnapshotDto {
    RunSnapshotDto {
        run_id,
        state: state.state,
        events: state.events.iter().cloned().collect(),
        pending_approvals: state.pending_approvals.clone(),
        oldest_sequence: state.oldest_sequence,
        latest_sequence: state.latest_sequence,
        next_sequence: state.next_sequence,
        updated_at: state.updated_at.clone(),
    }
}

fn apply_kind(state: &mut RunMutable, kind: &UiEventKind) {
    match kind {
        UiEventKind::RunStarted { .. }
        | UiEventKind::TurnStarted {}
        | UiEventKind::Message { .. }
        | UiEventKind::MessageDelta { .. }
        | UiEventKind::Reasoning { .. }
        | UiEventKind::ToolCall { .. }
        | UiEventKind::FileChange { .. }
        | UiEventKind::TodoList { .. }
        | UiEventKind::Usage { .. }
        | UiEventKind::Validation { .. } => {
            if state.state == RunStateDto::Created {
                state.state = RunStateDto::Running;
            }
        }
        UiEventKind::ApprovalRequested { approval } => {
            state.state = RunStateDto::AwaitingApproval;
            state.pending_approvals.push(approval.clone());
        }
        UiEventKind::ApprovalResolved { resolution } => {
            state
                .pending_approvals
                .retain(|item| item.approval_id != resolution.approval_id);
            if !state.stopped {
                state.state = RunStateDto::Running;
            }
        }
        UiEventKind::RunStopped { reason } => {
            state.state = state_from_stop_reason(reason);
            state.stopped = true;
        }
        UiEventKind::Error { .. } => {
            if state.state == RunStateDto::Created {
                state.state = RunStateDto::Failed;
            }
        }
    }
    if let UiEventKind::Usage {
        input_tokens,
        output_tokens,
        cached_input_tokens,
        reasoning_output_tokens,
    } = kind
    {
        state.usage.input_tokens = state.usage.input_tokens.saturating_add(*input_tokens);
        state.usage.output_tokens = state.usage.output_tokens.saturating_add(*output_tokens);
        state.usage.cached_input_tokens = state
            .usage
            .cached_input_tokens
            .saturating_add(*cached_input_tokens);
        state.usage.reasoning_output_tokens = state
            .usage
            .reasoning_output_tokens
            .saturating_add(*reasoning_output_tokens);
    }
}

fn random_id(prefix: &str) -> Result<String, RunRegistryError> {
    let mut bytes = [0_u8; 16];
    fill_random(&mut bytes).map_err(|_| RunRegistryError::Entropy)?;
    let suffix = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("{prefix}-{suffix}"))
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_contract::RunStateDto;

    #[tokio::test]
    async fn a_registry_assigns_monotonic_sequences_and_replays_events() {
        let registry = RunRegistry::new(4);
        let run = registry.create().expect("run");
        let first = run
            .append(UiEventKind::RunStarted { title: None })
            .await
            .expect("first event");
        let second = run
            .append(UiEventKind::Message {
                text: "hello".into(),
            })
            .await
            .expect("second event");

        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        let snapshot = run.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.state, RunStateDto::Running);
        assert_eq!(snapshot.next_sequence, 3);
        assert_eq!(snapshot.latest_sequence, 2);
        assert_eq!(snapshot.events.len(), 2);

        let replay = run.replay(1, Some(10)).await.expect("replay");
        assert_eq!(replay.events.len(), 1);
        assert_eq!(replay.first_sequence, Some(2));
        assert_eq!(replay.last_sequence, Some(2));
        assert!(!replay.has_more);
    }

    #[tokio::test]
    async fn retention_reports_a_resync_boundary_instead_of_silent_loss() {
        let registry = RunRegistry::new(2);
        let run = registry.create().expect("run");
        for text in ["one", "two", "three"] {
            run.append(UiEventKind::Message { text: text.into() })
                .await
                .expect("event");
        }

        let error = run.replay(0, None).await.expect_err("retention error");
        assert!(matches!(error, RunRegistryError::RetentionExceeded { .. }));
        let snapshot = run.snapshot().await.expect("snapshot");
        assert_eq!(snapshot.oldest_sequence, 2);
        assert_eq!(snapshot.latest_sequence, 3);
    }

    #[tokio::test]
    async fn cancellation_is_idempotent_and_marks_the_run_terminal() {
        let registry = RunRegistry::new(4);
        let run = registry.create().expect("run");
        let first = run.cancel().await.expect("cancel");
        let second = run.cancel().await.expect("repeat cancel");
        assert!(first.stopped);
        assert!(second.stopped);
        assert_eq!(first.run_id, second.run_id);
        assert!(run.cancellation_token().is_cancelled());
        assert_eq!(
            run.snapshot().await.expect("snapshot").state,
            RunStateDto::Cancelled
        );
    }
}
