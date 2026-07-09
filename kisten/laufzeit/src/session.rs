//! Per-run state: the session state machine and result accumulator.
//!
//! A [`Session`] is a *pure accumulator*: you feed it each [`Event`] as it
//! streams past with [`Session::observe`], and at EOF you call [`Session::finish`]
//! to get a [`RunResult`]. Keeping it side-effect-free (no I/O, no stream
//! ownership) makes the whole normalization → summary path unit-testable without
//! spawning anything, and lets the CLI drive the same logic while it renders.

use orchester_protokoll::{Event, Outcome, RunResult, SessionState, Usage};

/// Accumulates the outcome of a single agent run from its event stream.
#[derive(Debug, Clone)]
pub struct Session {
    state: SessionState,
    session_id: Option<String>,
    final_text: String,
    error_message: Option<String>,
    usage: Usage,
}

impl Session {
    /// A fresh session, before any events, in [`SessionState::Starting`].
    pub fn new() -> Self {
        Self {
            state: SessionState::Starting,
            session_id: None,
            final_text: String::new(),
            error_message: None,
            usage: Usage::default(),
        }
    }

    /// Current lifecycle state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// The resumable session id, once seen.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Fold one event into the running state.
    ///
    /// State transitions:
    /// * `SessionStarted` → record id, `Starting`/`Running` → `Running`
    /// * `Result`         → capture final text, → `Completed`
    /// * `Error`          → capture message, → `Failed`
    /// * `Usage`          → accumulate token counts
    ///
    /// A terminal state (`Failed`/`Cancelled`) is sticky: later non-error events
    /// won't silently flip it back to `Completed`.
    pub fn observe(&mut self, event: &Event) {
        match event {
            Event::SessionStarted { session_id } => {
                self.session_id = Some(session_id.clone());
                self.enter_running();
            }
            Event::Result { text } => {
                self.final_text = text.clone();
                if self.state != SessionState::Failed {
                    self.state = SessionState::Completed;
                }
            }
            Event::Error { message } => {
                self.error_message = Some(message.clone());
                self.state = SessionState::Failed;
            }
            Event::Usage(usage) => {
                self.usage.add(usage);
                self.enter_running();
            }
            // Any other event means the agent is producing output → Running.
            _ => self.enter_running(),
        }
    }

    /// Mark the run as cancelled (e.g. Ctrl-C). Terminal.
    pub fn cancel(&mut self) {
        self.state = SessionState::Cancelled;
    }

    /// Consume the session and produce its summary.
    ///
    /// If the stream ended without an explicit `Result` or `Error`, a run still
    /// in `Starting`/`Running` is treated as a success (the process exited
    /// cleanly) unless it was cancelled or failed.
    pub fn finish(self) -> RunResult {
        let outcome = match self.state {
            SessionState::Completed => Outcome::Success,
            SessionState::Failed => Outcome::Failed,
            SessionState::Cancelled => Outcome::Cancelled,
            // Stream ended cleanly without a terminal event.
            SessionState::Starting | SessionState::Running => Outcome::Success,
        };
        let final_text = match self.error_message {
            Some(msg) if self.final_text.is_empty() => msg,
            _ => self.final_text,
        };
        RunResult {
            session_id: self.session_id,
            final_text,
            usage: self.usage,
            outcome,
        }
    }

    /// Advance `Starting` → `Running`; leave terminal states untouched.
    fn enter_running(&mut self) {
        if self.state == SessionState::Starting {
            self.state = SessionState::Running;
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_completes() {
        let mut s = Session::new();
        assert_eq!(s.state(), SessionState::Starting);
        s.observe(&Event::SessionStarted {
            session_id: "sid".into(),
        });
        assert_eq!(s.state(), SessionState::Running);
        assert_eq!(s.session_id(), Some("sid"));
        s.observe(&Event::Message { text: "hi".into() });
        s.observe(&Event::Result {
            text: "final".into(),
        });
        assert_eq!(s.state(), SessionState::Completed);

        let r = s.finish();
        assert_eq!(r.outcome, Outcome::Success);
        assert_eq!(r.final_text, "final");
        assert_eq!(r.session_id.as_deref(), Some("sid"));
    }

    #[test]
    fn error_makes_failed_and_is_sticky() {
        let mut s = Session::new();
        s.observe(&Event::Error {
            message: "boom".into(),
        });
        assert_eq!(s.state(), SessionState::Failed);
        // A late Result must not resurrect a failed run to Completed.
        s.observe(&Event::Result { text: "x".into() });
        assert_eq!(s.state(), SessionState::Failed);

        let r = s.finish();
        assert_eq!(r.outcome, Outcome::Failed);
        // final_text present, so error message is not substituted.
        assert_eq!(r.final_text, "x");
    }

    #[test]
    fn error_without_result_surfaces_message() {
        let mut s = Session::new();
        s.observe(&Event::Error {
            message: "boom".into(),
        });
        let r = s.finish();
        assert_eq!(r.final_text, "boom");
    }

    #[test]
    fn usage_accumulates() {
        let mut s = Session::new();
        s.observe(&Event::Usage(Usage {
            input_tokens: 10,
            output_tokens: 5,
            ..Usage::default()
        }));
        s.observe(&Event::Usage(Usage {
            input_tokens: 3,
            output_tokens: 2,
            ..Usage::default()
        }));
        let r = s.finish();
        assert_eq!(r.usage.input_tokens, 13);
        assert_eq!(r.usage.output_tokens, 7);
    }

    #[test]
    fn clean_eof_without_result_is_success() {
        let mut s = Session::new();
        s.observe(&Event::Message { text: "hi".into() });
        assert_eq!(s.finish().outcome, Outcome::Success);
    }
}
