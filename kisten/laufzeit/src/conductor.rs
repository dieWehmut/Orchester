//! The Conductor: resolve an adapter and drive a task through it.
//!
//! The Conductor is deliberately thin. It owns a [`Registry`], resolves the
//! requested agent, and hands the [`Task`] to that adapter, returning the raw
//! [`EventStream`]. Lifecycle accounting lives in [`Session`], which the caller
//! folds events into (the CLI does this while rendering). This separation keeps
//! streaming (Conductor) orthogonal to summarization (Session).

use orchester_protokoll::{RunResult, Task};
use orchester_vertrag::{AdapterError, EventStream};
use orchester_verzeichnis::Registry;

use crate::session::Session;

/// Errors surfaced by the runtime layer.
#[derive(Debug, thiserror::Error)]
pub enum ConductorError {
    /// No adapter with this name is registered.
    #[error("unknown agent '{0}' (run `orchester list` to see available agents)")]
    UnknownAgent(String),

    /// The adapter failed to spawn or stream.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
}

/// Dispatches tasks to registered adapters.
pub struct Conductor {
    registry: Registry,
}

impl Conductor {
    /// Wrap a registry.
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// Access the underlying registry (for `orchester list`).
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Resolve `agent` and start `task`, returning its normalized event stream.
    pub async fn run(&self, agent: &str, task: Task) -> Result<EventStream, ConductorError> {
        let adapter = self
            .registry
            .get(agent)
            .ok_or_else(|| ConductorError::UnknownAgent(agent.to_string()))?;
        Ok(adapter.run(task).await?)
    }

    /// Convenience for non-interactive callers and tests: drive the run to
    /// completion, folding every event into a [`Session`], and return the
    /// summary. The `on_event` callback observes each event as it arrives (the
    /// CLI passes a renderer; tests can pass a collector or a no-op).
    pub async fn run_to_result<F>(
        &self,
        agent: &str,
        task: Task,
        mut on_event: F,
    ) -> Result<RunResult, ConductorError>
    where
        F: FnMut(&orchester_protokoll::Event),
    {
        use futures::StreamExt;

        let mut stream = self.run(agent, task).await?;
        let mut session = Session::new();
        while let Some(item) = stream.next().await {
            let event = item?;
            session.observe(&event);
            on_event(&event);
        }
        Ok(session.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_protokoll::{Event, Outcome};
    use std::path::PathBuf;

    fn conductor() -> Conductor {
        let mut registry = Registry::new();
        registry.register_builtins();
        Conductor::new(registry)
    }

    #[tokio::test]
    async fn runs_mock_to_success() {
        let c = conductor();
        let task = Task::new("hello", PathBuf::from("."));
        let mut seen = Vec::new();
        let result = c
            .run_to_result("mock", task, |e| seen.push(e.clone()))
            .await
            .expect("mock run");

        assert_eq!(result.outcome, Outcome::Success);
        assert_eq!(result.session_id.as_deref(), Some("mock-session"));
        assert!(result.final_text.contains("hello"));
        assert!(seen.iter().any(|e| matches!(e, Event::SessionStarted { .. })));
    }

    #[tokio::test]
    async fn unknown_agent_errors() {
        let c = conductor();
        let task = Task::new("hi", PathBuf::from("."));
        match c.run("ghost", task).await {
            Err(ConductorError::UnknownAgent(name)) => assert_eq!(name, "ghost"),
            _ => panic!("expected UnknownAgent error"),
        }
    }
}
