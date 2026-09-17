//! Bridges the self-agent runtime's vendor-neutral event stream into the
//! context-owned run registry.
//!
//! The runtime narrates one run as [`Event`] values through [`RunEventSink`],
//! which it calls from the run's own task: the trait is documented as a channel
//! send, so the sink here only forwards. Translation, redaction and retention
//! happen in the drain task, where blocking is allowed and where a slow registry
//! cannot stall the run.

use std::collections::{HashMap, VecDeque};

use orchester_laufzeit::harness::service::RunEventSink;
use orchester_protokoll::{
    redact_ui_path, CallId, Event, RunId, ToolStatus, UiApprovalRequest, UiEventKind, UiToolState,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::run_registry::{RunHandle, RunRegistryError};

/// The approval slice is a follow-up, so the runtime's `ApprovalRequired` is
/// projected with the fields the decision endpoints will fill in later rather
/// than dropped: a frontend that sees the request can still say what it waits on.
const UNKNOWN_RISK: &str = "unknown";

/// Every runtime failure the narration reports shares one code. The registry
/// keeps the message for the human; the code exists so a frontend can branch
/// without parsing prose.
const RUNTIME_ERROR_CODE: &str = "runtime";

/// A [`RunEventSink`] that forwards the runtime's events to the drain task.
pub(crate) struct RegistryRunSink {
    sender: UnboundedSender<Event>,
}

impl RegistryRunSink {
    pub(crate) fn channel() -> (Self, UnboundedReceiver<Event>) {
        let (sender, receiver) = unbounded_channel();
        (Self { sender }, receiver)
    }
}

impl RunEventSink for RegistryRunSink {
    fn emit(&self, event: Event) {
        // A closed channel means the drain task is gone; the run keeps going and
        // the events are simply not retained.
        let _ = self.sender.send(event);
    }
}

/// Translates the runtime vocabulary into the UI event kinds the registry keeps.
///
/// `Event::ToolCall` carries no call id, and the same tool can run twice in one
/// turn, so ids are assigned in arrival order and paired back per tool name.
#[derive(Debug)]
pub(crate) struct UiEventTranslator {
    run_id: RunId,
    open_calls: HashMap<String, VecDeque<CallId>>,
    next_call: u64,
}

impl UiEventTranslator {
    pub(crate) fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            open_calls: HashMap::new(),
            next_call: 0,
        }
    }

    /// `None` means the runtime said something the UI vocabulary has no shape
    /// for; the frame is dropped rather than guessed at.
    pub(crate) fn translate(&mut self, event: Event) -> Option<UiEventKind> {
        Some(match event {
            // The registry's run id is its own; the durable session id the
            // runtime reports is not a title, so it stays off the wire.
            Event::SessionStarted { .. } => UiEventKind::RunStarted { title: None },
            Event::TurnStarted => UiEventKind::TurnStarted {},
            Event::Message { text } => UiEventKind::Message { text },
            Event::Reasoning { text } => UiEventKind::Reasoning { text },
            Event::ToolCall {
                name,
                status,
                detail,
            } => {
                let call_id = self.call_id(&name, status);
                UiEventKind::ToolCall {
                    call_id,
                    name,
                    state: tool_state(status),
                    detail,
                }
            }
            Event::FileChange { path, kind } => UiEventKind::FileChange {
                path: redact_ui_path(&path),
                kind,
            },
            Event::TodoList { items } => UiEventKind::TodoList { items },
            Event::Usage(usage) => UiEventKind::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cached_input_tokens: usage.cached_input_tokens,
                reasoning_output_tokens: usage.reasoning_output_tokens,
            },
            Event::ApprovalRequired {
                approval_id,
                action,
                reason,
            } => UiEventKind::ApprovalRequested {
                approval: UiApprovalRequest {
                    approval_id,
                    run_id: self.run_id.clone(),
                    row_version: 0,
                    risk: UNKNOWN_RISK.to_owned(),
                    action,
                    reason,
                    expires_at: None,
                },
            },
            // The final assistant text closes the transcript, so it is a whole
            // message rather than a delta.
            Event::Result { text } => UiEventKind::Message { text },
            Event::Stopped { reason } => UiEventKind::RunStopped { reason },
            Event::Error { message } => UiEventKind::Error {
                code: RUNTIME_ERROR_CODE.to_owned(),
                message,
            },
            Event::TurnCompleted => return None,
        })
    }

    fn call_id(&mut self, name: &str, status: ToolStatus) -> CallId {
        if let ToolStatus::InProgress = status {
            let id = self.next_id();
            self.open_calls
                .entry(name.to_owned())
                .or_default()
                .push_back(id.clone());
            return id;
        }
        // A completion for a call nobody opened still gets a unique id: an
        // unidentified tool row beats a dropped one.
        self.open_calls
            .get_mut(name)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| self.next_id())
    }

    fn next_id(&mut self) -> CallId {
        self.next_call = self.next_call.saturating_add(1);
        CallId::from(format!("call-{}", self.next_call))
    }
}

fn tool_state(status: ToolStatus) -> UiToolState {
    match status {
        ToolStatus::InProgress => UiToolState::Running,
        ToolStatus::Completed => UiToolState::Succeeded,
        ToolStatus::Failed => UiToolState::Failed,
    }
}

/// Retains every translated event on the run until the run is terminal.
///
/// The registry refuses frames once a run stopped, so the drain ends on the
/// first terminal frame instead of racing the cancellation that follows it.
pub(crate) async fn drain_run_events(
    run: RunHandle,
    run_id: RunId,
    mut receiver: UnboundedReceiver<Event>,
) {
    let mut translator = UiEventTranslator::new(run_id);
    while let Some(event) = receiver.recv().await {
        let Some(kind) = translator.translate(event) else {
            continue;
        };
        let terminal = matches!(
            kind,
            UiEventKind::RunStopped { .. } | UiEventKind::Error { .. }
        );
        match run.append(kind).await {
            Ok(_) => {}
            // Terminal means the run was cancelled or already stopped; either
            // way there is nothing left to retain.
            Err(RunRegistryError::Terminal) | Err(RunRegistryError::NotFound) => break,
            Err(_) => break,
        }
        if terminal {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use orchester_protokoll::{ApprovalId, ChangeKind, TodoItem};

    use super::*;

    fn translator() -> UiEventTranslator {
        UiEventTranslator::new(RunId::from("run-1".to_owned()))
    }

    #[test]
    fn the_runtime_vocabulary_maps_onto_the_ui_vocabulary() {
        let mut translator = translator();

        assert_eq!(
            translator.translate(Event::SessionStarted {
                session_id: "session-1".into()
            }),
            Some(UiEventKind::RunStarted { title: None })
        );
        assert_eq!(
            translator.translate(Event::TurnStarted),
            Some(UiEventKind::TurnStarted {})
        );
        assert_eq!(
            translator.translate(Event::Message {
                text: "hello".into()
            }),
            Some(UiEventKind::Message {
                text: "hello".into()
            })
        );
        assert_eq!(
            translator.translate(Event::Result {
                text: "final".into()
            }),
            Some(UiEventKind::Message {
                text: "final".into()
            })
        );
        assert_eq!(
            translator.translate(Event::Stopped {
                reason: orchester_protokoll::StopReason::Succeeded
            }),
            Some(UiEventKind::RunStopped {
                reason: orchester_protokoll::StopReason::Succeeded
            })
        );
        assert_eq!(
            translator.translate(Event::Error {
                message: "boom".into()
            }),
            Some(UiEventKind::Error {
                code: RUNTIME_ERROR_CODE.into(),
                message: "boom".into()
            })
        );
        assert_eq!(
            translator.translate(Event::Usage(orchester_protokoll::Usage {
                input_tokens: 3,
                output_tokens: 5,
                cached_input_tokens: 1,
                reasoning_output_tokens: 2,
            })),
            Some(UiEventKind::Usage {
                input_tokens: 3,
                output_tokens: 5,
                cached_input_tokens: 1,
                reasoning_output_tokens: 2,
            })
        );
        assert_eq!(
            translator.translate(Event::TodoList {
                items: vec![TodoItem {
                    text: "step".into(),
                    completed: false
                }]
            }),
            Some(UiEventKind::TodoList {
                items: vec![TodoItem {
                    text: "step".into(),
                    completed: false
                }]
            })
        );
        assert_eq!(
            translator.translate(Event::ApprovalRequired {
                approval_id: ApprovalId::from("approval-1".to_owned()),
                action: "run: ls".into(),
                reason: "outside the workspace".into(),
            }),
            Some(UiEventKind::ApprovalRequested {
                approval: UiApprovalRequest {
                    approval_id: ApprovalId::from("approval-1".to_owned()),
                    run_id: RunId::from("run-1".to_owned()),
                    row_version: 0,
                    risk: UNKNOWN_RISK.to_owned(),
                    action: "run: ls".into(),
                    reason: "outside the workspace".into(),
                    expires_at: None,
                }
            })
        );

        // The UI vocabulary has no turn boundary of its own.
        assert_eq!(translator.translate(Event::TurnCompleted), None);
    }

    #[test]
    fn tool_calls_pair_by_name_and_arrival_order() {
        let mut translator = translator();

        let first = translator
            .translate(Event::ToolCall {
                name: "shell".into(),
                status: ToolStatus::InProgress,
                detail: None,
            })
            .expect("a tool call is a UI event");
        let second = translator
            .translate(Event::ToolCall {
                name: "shell".into(),
                status: ToolStatus::InProgress,
                detail: None,
            })
            .expect("a tool call is a UI event");
        let UiEventKind::ToolCall {
            call_id: first_id, ..
        } = first
        else {
            panic!("expected a tool call");
        };
        let UiEventKind::ToolCall {
            call_id: second_id, ..
        } = second
        else {
            panic!("expected a tool call");
        };
        assert_ne!(first_id, second_id, "two calls of one tool are distinct");

        // The first completion belongs to the first call.
        let finished = translator
            .translate(Event::ToolCall {
                name: "shell".into(),
                status: ToolStatus::Completed,
                detail: None,
            })
            .expect("a completion is a UI event");
        assert_eq!(
            finished,
            UiEventKind::ToolCall {
                call_id: first_id,
                name: "shell".into(),
                state: UiToolState::Succeeded,
                detail: None,
            }
        );

        // A completion nobody opened still gets its own row.
        let orphan = translator
            .translate(Event::ToolCall {
                name: "read_file".into(),
                status: ToolStatus::Failed,
                detail: None,
            })
            .expect("a completion is a UI event");
        let UiEventKind::ToolCall {
            call_id: orphan_id,
            state,
            ..
        } = orphan
        else {
            panic!("expected a tool call");
        };
        assert_ne!(orphan_id, second_id);
        assert_eq!(state, UiToolState::Failed);
    }

    #[test]
    fn file_change_paths_are_redacted_before_they_leave_the_process() {
        let mut translator = translator();

        let event = translator
            .translate(Event::FileChange {
                path: r"C:\Users\someone\project\src\main.rs".into(),
                kind: ChangeKind::Update,
            })
            .expect("a file change is a UI event");

        let UiEventKind::FileChange { path, .. } = event else {
            panic!("expected a file change");
        };
        assert!(
            !path.contains("someone"),
            "the host path must be redacted: {path}"
        );
    }
}
