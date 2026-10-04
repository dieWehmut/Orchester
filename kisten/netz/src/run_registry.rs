use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex as StdMutex},
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
    state: StdMutex<RegistryState>,
}

struct RegistryState {
    runs: HashMap<RunId, Arc<RunEntry>>,
    idempotency: HashMap<String, IdempotencyEntry>,
}

struct IdempotencyEntry {
    fingerprint: [u8; 32],
    run_id: RunId,
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
    #[error("idempotency key was already used for a different request")]
    IdempotencyConflict,
}

#[derive(Clone)]
pub(crate) struct RunHandle {
    entry: Arc<RunEntry>,
}

pub(crate) enum RunCreation {
    Created(RunHandle),
    Reused(RunHandle),
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
                state: StdMutex::new(RegistryState {
                    runs: HashMap::new(),
                    idempotency: HashMap::new(),
                }),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn create(&self) -> Result<RunHandle, RunRegistryError> {
        self.create_inner(None)
    }

    /// Publish a run and its first event together with its submission key.
    ///
    /// Both maps have the same lifetime: a retry keeps referring to the retained
    /// run, including after termination. No lock is held across an await, and
    /// a failure to build the first event publishes neither a run nor a key.
    pub(crate) fn create_or_reuse(
        &self,
        idempotency_key: Option<&str>,
        fingerprint: [u8; 32],
        initial_kind: UiEventKind,
    ) -> Result<RunCreation, RunRegistryError> {
        let Some(idempotency_key) = idempotency_key else {
            return self
                .create_inner(Some(initial_kind))
                .map(RunCreation::Created);
        };

        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| RunRegistryError::LockPoisoned)?;

        if let Some(entry) = state.idempotency.get(idempotency_key) {
            if entry.fingerprint != fingerprint {
                return Err(RunRegistryError::IdempotencyConflict);
            }
            let run = state
                .runs
                .get(&entry.run_id)
                .cloned()
                .map(|entry| RunHandle { entry })
                .ok_or(RunRegistryError::NotFound)?;
            return Ok(RunCreation::Reused(run));
        }

        let run = create_entry_locked(&self.inner, &mut state, Some(initial_kind))?;
        state.idempotency.insert(
            idempotency_key.to_owned(),
            IdempotencyEntry {
                fingerprint,
                run_id: run.id().clone(),
            },
        );
        Ok(RunCreation::Created(run))
    }

    fn create_inner(
        &self,
        initial_kind: Option<UiEventKind>,
    ) -> Result<RunHandle, RunRegistryError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| RunRegistryError::LockPoisoned)?;
        create_entry_locked(&self.inner, &mut state, initial_kind)
    }

    pub(crate) fn get(&self, run_id: &RunId) -> Result<RunHandle, RunRegistryError> {
        self.inner
            .state
            .lock()
            .map_err(|_| RunRegistryError::LockPoisoned)?
            .runs
            .get(run_id)
            .cloned()
            .map(|entry| RunHandle { entry })
            .ok_or(RunRegistryError::NotFound)
    }
}

fn create_entry_locked(
    inner: &RegistryInner,
    state: &mut RegistryState,
    initial_kind: Option<UiEventKind>,
) -> Result<RunHandle, RunRegistryError> {
    for _ in 0..8 {
        let id = RunId::from(random_id("run")?);
        if state.runs.contains_key(&id) {
            continue;
        }
        let mut mutable = RunMutable::new();
        if let Some(kind) = initial_kind.clone() {
            append_to_state(&id, inner.retention, &mut mutable, kind)?;
        }
        let (frames, _) = broadcast::channel(inner.retention.max(32));
        let entry = Arc::new(RunEntry {
            id: id.clone(),
            retention: inner.retention,
            state: Mutex::new(mutable),
            frames,
            cancellation: CancellationToken::new(),
        });
        state.runs.insert(id, Arc::clone(&entry));
        return Ok(RunHandle { entry });
    }
    Err(RunRegistryError::Entropy)
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
        let event = append_to_state(self.id(), self.retention(), &mut state, kind)?;
        let frame = RunStreamFrameDto::Event {
            event: event.clone(),
        };
        // Broadcasting is synchronous and non-blocking. Keep it under the
        // sequence lock so cancellation and runtime narration cannot publish
        // sequence N+1 before N, which would make streaming clients skip N.
        let _ = self.entry.frames.send(frame);
        drop(state);
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

/// Also used before publication, when there are no async locks or subscribers.
fn append_to_state(
    run_id: &RunId,
    retention: usize,
    state: &mut RunMutable,
    kind: UiEventKind,
) -> Result<UiEventEnvelope, RunRegistryError> {
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
        run_id: run_id.clone(),
        turn_id: None,
        call_id,
        sequence,
        occurred_at: occurred_at.clone(),
        kind,
    };
    event
        .validate()
        .map_err(|error| RunRegistryError::InvalidEvent(error.to_string()))?;
    apply_kind(state, &event.kind);
    state.events.push_back(event.clone());
    state.latest_sequence = sequence;
    state.next_sequence = sequence.saturating_add(1);
    state.updated_at = occurred_at;
    while state.events.len() > retention {
        state.events.pop_front();
    }
    state.oldest_sequence = state
        .events
        .front()
        .map(|event| event.sequence)
        .unwrap_or(state.next_sequence);
    Ok(event)
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
        // The reader's own turn is journalled before the runtime has said
        // anything, and the API has already accepted the run by then: a
        // snapshot taken between the two must not claim the run has not begun.
        | UiEventKind::UserMessage { .. }
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
        // An error is retained before its stop reason. Only RunStopped may
        // advertise a terminal snapshot, otherwise a reconnecting client can
        // close its socket before the final journal event has been published.
        UiEventKind::Error { .. } => {}
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_appends_broadcast_in_journal_sequence_order() {
        let registry = RunRegistry::new(256);
        let run = registry.create().unwrap();
        let mut frames = run.subscribe();
        let barrier = Arc::new(tokio::sync::Barrier::new(256));
        let mut tasks = Vec::new();
        for index in 0..256 {
            let run = run.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                run.append(UiEventKind::Message {
                    text: index.to_string(),
                })
                .await
                .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        for sequence in 1..=256 {
            let RunStreamFrameDto::Event { event } = frames.try_recv().unwrap() else {
                panic!("expected the append's event frame");
            };
            assert_eq!(event.sequence, sequence);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_retries_publish_one_initialized_run_and_one_execution_owner() {
        let registry = RunRegistry::new(4);
        let barrier = Arc::new(tokio::sync::Barrier::new(32));
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let registry = registry.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                let creation = registry
                    .create_or_reuse(
                        Some("one-submission"),
                        [1; 32],
                        UiEventKind::UserMessage {
                            text: "asked once".into(),
                        },
                    )
                    .expect("accepted retry");
                let (run, owner) = match creation {
                    RunCreation::Created(run) => (run, true),
                    RunCreation::Reused(run) => (run, false),
                };
                // Every response, including those which lost the race, can
                // already replay the reader's turn without waiting for its owner.
                let snapshot = run.snapshot().await.expect("published snapshot");
                assert_eq!(snapshot.events.len(), 1);
                assert_eq!(snapshot.events[0].sequence, 1);
                assert_eq!(
                    snapshot.events[0].kind,
                    UiEventKind::UserMessage {
                        text: "asked once".into(),
                    }
                );
                assert_eq!(snapshot.state, RunStateDto::Running);
                (run.id().clone(), owner)
            }));
        }
        let mut ids = std::collections::HashSet::new();
        let mut owners = 0;
        for task in tasks {
            let (id, owner) = task.await.expect("retry task");
            ids.insert(id);
            owners += usize::from(owner);
        }
        assert_eq!(ids.len(), 1);
        assert_eq!(owners, 1);
    }

    #[tokio::test]
    async fn a_failed_initial_event_publishes_neither_run_nor_key() {
        let registry = RunRegistry::new(4);
        let invalid = registry.create_or_reuse(
            Some("reusable-after-failure"),
            [1; 32],
            UiEventKind::ToolCall {
                call_id: "".into(),
                name: "invalid-call".into(),
                state: orchester_protokoll::UiToolState::Running,
                detail: None,
            },
        );
        assert!(matches!(invalid, Err(RunRegistryError::InvalidEvent(_))));
        {
            let state = registry.inner.state.lock().unwrap();
            assert!(state.runs.is_empty());
            assert!(state.idempotency.is_empty());
        }
        assert!(matches!(
            registry.create_or_reuse(
                Some("reusable-after-failure"),
                [2; 32],
                UiEventKind::UserMessage {
                    text: "accepted".into(),
                },
            ),
            Ok(RunCreation::Created(_))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn keys_survive_the_retry_window_event_eviction_and_termination() {
        let registry = RunRegistry::new(2);
        let initial_kind = UiEventKind::UserMessage {
            text: "retained identity".into(),
        };
        let RunCreation::Created(run) = registry
            .create_or_reuse(Some("long-lived-key"), [3; 32], initial_kind.clone())
            .unwrap()
        else {
            panic!("first submission owns execution");
        };
        run.append(UiEventKind::Message { text: "one".into() })
            .await
            .unwrap();
        run.cancel().await.unwrap();
        tokio::time::advance(std::time::Duration::from_secs(16 * 60)).await;
        let before = run.snapshot().await.unwrap();
        assert!(before.oldest_sequence > 1);
        let RunCreation::Reused(retried) = registry
            .create_or_reuse(Some("long-lived-key"), [3; 32], initial_kind.clone())
            .unwrap()
        else {
            panic!("a retained run must never execute again");
        };
        assert_eq!(retried.id(), run.id());
        assert_eq!(retried.snapshot().await.unwrap(), before);
        assert!(matches!(
            registry.create_or_reuse(Some("long-lived-key"), [4; 32], initial_kind),
            Err(RunRegistryError::IdempotencyConflict)
        ));
    }

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
    async fn an_error_does_not_advertise_a_terminal_snapshot_before_its_stop_frame() {
        for reason in [StopReason::Failed, StopReason::Succeeded] {
            let registry = RunRegistry::new(8);
            let run = registry.create().expect("run");
            run.append(UiEventKind::UserMessage {
                text: "asked".into(),
            })
            .await
            .unwrap();
            run.append(UiEventKind::Error {
                code: "runtime".into(),
                message: "a retained error".into(),
            })
            .await
            .unwrap();
            assert_eq!(run.snapshot().await.unwrap().state, RunStateDto::Running);
            assert!(!run.summary().await.unwrap().stopped);

            run.append(UiEventKind::RunStopped {
                reason: reason.clone(),
            })
            .await
            .unwrap();
            let snapshot = run.snapshot().await.unwrap();
            assert_eq!(snapshot.state, state_from_stop_reason(&reason));
            assert!(run.summary().await.unwrap().stopped);
            assert!(matches!(
                snapshot.events.last().unwrap().kind,
                UiEventKind::RunStopped { .. }
            ));
        }
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
