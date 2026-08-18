//! The vendor-neutral event boundary of the self-owned agent.
//!
//! The adapter path has always streamed [`Event`]s: a subprocess prints its own
//! JSONL and `Conductor` normalizes it. The self-owned agent had no equivalent —
//! it streamed raw text deltas through `ModelEventSink` and returned everything
//! else in a `SelfAgentRunOutcome` once the run was already over. That is enough
//! for the TUI, which owns the run, and useless to anything that does not: a
//! browser or a desktop shell cannot wait until the end to draw the first tool
//! call.
//!
//! So the runtime narrates itself in the same vocabulary every adapter speaks,
//! and every text it puts on the wire goes through the same [`StreamingRedactor`]
//! the terminal uses. A stream that leaves the process must never be *less*
//! redacted than the one that stays inside it.

use std::sync::{Arc, Mutex};

use orchester_modell::ModelUsage;
use orchester_protokoll::{
    AgentAction, Event, PolicyDecision, RunId, StopReason, ToolStatus, Usage,
};

use super::SelfAgentTurn;
use crate::harness::execution::GovernedToolOutcome;
use crate::harness::feedback::StreamingRedactor;

/// A consumer of the unified event stream of one self-agent run.
///
/// Implementations are called from the run's own task and must not block: the
/// intended shape is a channel send.
pub trait RunEventSink: Send + Sync {
    fn emit(&self, event: Event);
}

/// Narrates one run to an optional [`RunEventSink`].
///
/// With no sink attached every method is a no-op, so the runtime narrates
/// unconditionally instead of branching at each call site.
pub(crate) struct RunNarrator {
    sink: Option<Arc<dyn RunEventSink>>,
    redactor: Mutex<StreamingRedactor>,
}

impl RunNarrator {
    pub(crate) fn new(sink: Option<Arc<dyn RunEventSink>>, redactor: StreamingRedactor) -> Self {
        Self {
            sink,
            redactor: Mutex::new(redactor),
        }
    }

    /// The run has a durable id. For the self-owned agent the [`RunId`] *is* the
    /// resume handle, which is exactly what `SessionStarted` means to a frontend.
    pub(crate) fn started(&self, run_id: &RunId) {
        self.emit(Event::SessionStarted {
            session_id: run_id.0.clone(),
        });
        self.emit(Event::TurnStarted);
    }

    /// A governed action is about to be executed.
    pub(crate) fn tool_started(&self, action: &AgentAction) {
        self.emit(Event::ToolCall {
            name: action.tool_name().to_owned(),
            status: ToolStatus::InProgress,
            detail: Some(action.action_summary()),
        });
    }

    /// A governed action finished, successfully or not.
    pub(crate) fn tool_finished(&self, action: &AgentAction, outcome: &GovernedToolOutcome) {
        let (status, detail) = match outcome {
            GovernedToolOutcome::Completed(observation) => {
                (ToolStatus::Completed, &observation.summary)
            }
            GovernedToolOutcome::Failed(feedback) => (ToolStatus::Failed, &feedback.summary),
        };
        self.emit(Event::ToolCall {
            name: action.tool_name().to_owned(),
            status,
            detail: Some(self.redact(detail)),
        });
    }

    /// The run reached a model boundary it cannot cross on its own.
    pub(crate) fn finished(&self, turn: &SelfAgentTurn) {
        match turn {
            SelfAgentTurn::Text { text, .. } => self.emit(Event::Result {
                text: self.redact(text),
            }),
            // Nothing executed this action, so it stays `in_progress`: together
            // with the stop reason below it tells a frontend what the run is
            // waiting on.
            SelfAgentTurn::Action { action, .. } => self.tool_started(action),
        }
        self.emit(Event::Usage(token_usage(turn.usage())));
        self.emit(Event::TurnCompleted);
        self.emit(Event::Stopped {
            reason: stop_reason(turn),
        });
    }

    /// The run could not continue.
    pub(crate) fn failed(&self, message: &str) {
        self.emit(Event::Error {
            message: self.redact(message),
        });
        self.emit(Event::Stopped {
            reason: StopReason::Failed,
        });
    }

    /// The caller cancelled the run.
    pub(crate) fn cancelled(&self) {
        self.emit(Event::Stopped {
            reason: StopReason::Cancelled,
        });
    }

    fn emit(&self, event: Event) {
        if let Some(sink) = self.sink.as_ref() {
            sink.emit(event);
        }
    }

    /// Sanitize one complete string with the run's secret set.
    ///
    /// A poisoned lock means another thread panicked mid-redaction, so this
    /// fails closed rather than forwarding text that was never scanned.
    fn redact(&self, text: &str) -> String {
        let Ok(mut redactor) = self.redactor.lock() else {
            return "[REDACTED]".to_owned();
        };
        redactor.begin_response();
        redactor.push(text);
        redactor.finish().to_owned()
    }
}

/// Why a run stopped at this turn.
fn stop_reason(turn: &SelfAgentTurn) -> StopReason {
    match turn {
        SelfAgentTurn::Text { .. } => StopReason::Succeeded,
        // `Allow` still stops the loop for a mutating action — the runtime only
        // auto-executes reads — so both decisions leave the run waiting for a
        // human, and `Deny` means policy already refused it.
        SelfAgentTurn::Action { policy, .. } => match policy.decision {
            PolicyDecision::Allow | PolicyDecision::Ask => StopReason::AwaitingApproval,
            PolicyDecision::Deny => StopReason::Failed,
        },
    }
}

/// The model's token counts in the protocol's shape.
///
/// [`ModelUsage`] tracks only the two counters every provider reports; the
/// cached and reasoning columns stay zero rather than being guessed.
fn token_usage(usage: ModelUsage) -> Usage {
    Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        ..Usage::default()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use orchester_protokoll::{FeedbackReport, Observation};

    use super::*;
    use crate::harness::governance::{EffectClass, PolicyResult, Risk};

    #[derive(Default)]
    struct Recorder {
        events: StdMutex<Vec<Event>>,
    }

    impl RunEventSink for Recorder {
        fn emit(&self, event: Event) {
            self.events.lock().expect("not poisoned").push(event);
        }
    }

    fn narrator(secret: Option<&str>) -> (Arc<Recorder>, RunNarrator) {
        let recorder = Arc::new(Recorder::default());
        let secrets = secret
            .map(|value| vec![secrecy::SecretString::from(value.to_owned())])
            .unwrap_or_default();
        let narrator = RunNarrator::new(
            Some(recorder.clone() as Arc<dyn RunEventSink>),
            StreamingRedactor::new(secrets),
        );
        (recorder, narrator)
    }

    fn events(recorder: &Recorder) -> Vec<Event> {
        recorder.events.lock().expect("not poisoned").clone()
    }

    fn text_turn(text: &str) -> SelfAgentTurn {
        SelfAgentTurn::Text {
            run_id: "run-1".into(),
            text: text.to_owned(),
            model_calls: 1,
            usage: ModelUsage {
                input_tokens: 11,
                output_tokens: 7,
            },
        }
    }

    fn action_turn(action: AgentAction, decision: PolicyDecision) -> SelfAgentTurn {
        SelfAgentTurn::Action {
            run_id: "run-1".into(),
            action_id: "act-1".into(),
            call_id: "call-1".into(),
            action,
            policy: PolicyResult {
                decision,
                rule_id: "rule".into(),
                risk: Risk::Low,
                reason: "test".into(),
                effect: EffectClass::ReadOnlyIdempotent,
            },
            model_calls: 1,
            usage: ModelUsage::default(),
        }
    }

    #[test]
    fn a_silent_narrator_costs_nothing() {
        let narrator = RunNarrator::new(None, StreamingRedactor::new(Vec::new()));
        narrator.started(&"run-1".into());
        narrator.finished(&text_turn("done"));
        // Nothing to assert but the absence of a panic: every emit is a no-op.
    }

    #[test]
    fn a_run_opens_with_its_resume_handle() {
        let (recorder, narrator) = narrator(None);
        narrator.started(&"run-7".into());
        assert_eq!(
            events(&recorder),
            vec![
                Event::SessionStarted {
                    session_id: "run-7".into()
                },
                Event::TurnStarted,
            ]
        );
    }

    #[test]
    fn a_text_turn_closes_the_run_with_a_result() {
        let (recorder, narrator) = narrator(None);
        narrator.finished(&text_turn("all done"));
        assert_eq!(
            events(&recorder),
            vec![
                Event::Result {
                    text: "all done".into()
                },
                Event::Usage(Usage {
                    input_tokens: 11,
                    output_tokens: 7,
                    ..Usage::default()
                }),
                Event::TurnCompleted,
                Event::Stopped {
                    reason: StopReason::Succeeded
                },
            ]
        );
    }

    #[test]
    fn a_pending_action_stays_in_progress_and_names_what_it_waits_on() {
        let (recorder, narrator) = narrator(None);
        narrator.finished(&action_turn(
            AgentAction::WriteFile {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            },
            PolicyDecision::Ask,
        ));
        let events = events(&recorder);
        assert_eq!(
            events.first(),
            Some(&Event::ToolCall {
                name: "write_file".into(),
                status: ToolStatus::InProgress,
                detail: Some("write_file path_bytes=11 content_bytes=12".into()),
            })
        );
        assert_eq!(
            events.last(),
            Some(&Event::Stopped {
                reason: StopReason::AwaitingApproval
            })
        );
    }

    #[test]
    fn a_denied_action_stops_the_run() {
        let (recorder, narrator) = narrator(None);
        narrator.finished(&action_turn(
            AgentAction::RunCommand {
                program: "rm".into(),
                args: vec!["-rf".into()],
                cwd: None,
            },
            PolicyDecision::Deny,
        ));
        assert_eq!(
            events(&recorder).last(),
            Some(&Event::Stopped {
                reason: StopReason::Failed
            })
        );
    }

    #[test]
    fn a_tool_reports_both_of_its_edges() {
        let (recorder, narrator) = narrator(None);
        let action = AgentAction::ReadFile {
            path: "a.rs".into(),
            start_line: None,
            end_line: None,
        };
        narrator.tool_started(&action);
        narrator.tool_finished(
            &action,
            &GovernedToolOutcome::Completed(Observation {
                observation_id: "obs-1".into(),
                call_id: "call-1".into(),
                kind: "read_file".into(),
                summary: "read 4 lines".into(),
                data: serde_json::Value::Null,
            }),
        );
        let events = events(&recorder);
        assert!(
            matches!(&events[0], Event::ToolCall { status, .. } if *status == ToolStatus::InProgress)
        );
        assert_eq!(
            events[1],
            Event::ToolCall {
                name: "read_file".into(),
                status: ToolStatus::Completed,
                detail: Some("read 4 lines".into()),
            }
        );
    }

    #[test]
    fn a_failed_tool_reports_the_feedback_summary() {
        let (recorder, narrator) = narrator(None);
        narrator.tool_finished(
            &AgentAction::ReadFile {
                path: "missing.rs".into(),
                start_line: None,
                end_line: None,
            },
            &GovernedToolOutcome::Failed(FeedbackReport {
                source: "tool".into(),
                validator_id: None,
                exit_code: None,
                classification: "not_found".into(),
                summary: "no such file".into(),
                stdout_tail: String::new(),
                stderr_tail: String::new(),
                fingerprint: "fp".into(),
                retryable: false,
            }),
        );
        assert_eq!(
            events(&recorder),
            vec![Event::ToolCall {
                name: "read_file".into(),
                status: ToolStatus::Failed,
                detail: Some("no such file".into()),
            }]
        );
    }

    #[test]
    fn a_secret_never_reaches_the_wire() {
        let (recorder, narrator) = narrator(Some("sk-super-secret-value"));
        narrator.finished(&text_turn("the key is sk-super-secret-value, keep it"));
        let Some(Event::Result { text }) = events(&recorder).into_iter().next() else {
            panic!("expected a result event");
        };
        assert!(!text.contains("sk-super-secret-value"), "{text}");
    }

    #[test]
    fn a_failure_and_a_cancellation_both_stop_the_run() {
        let (recorder, narrator) = narrator(None);
        narrator.failed("provider unreachable");
        narrator.cancelled();
        assert_eq!(
            events(&recorder),
            vec![
                Event::Error {
                    message: "provider unreachable".into()
                },
                Event::Stopped {
                    reason: StopReason::Failed
                },
                Event::Stopped {
                    reason: StopReason::Cancelled
                },
            ]
        );
    }
}
