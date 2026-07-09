//! A built-in adapter that runs **no subprocess**.
//!
//! `MockAdapter` emits a fixed, scripted event sequence so the whole Orchester
//! pipeline (registry → conductor → CLI renderer) can be exercised with zero
//! installed agent CLIs and zero API keys. It is the backbone of the
//! deterministic end-to-end test in `konsole`.

use async_trait::async_trait;
use futures::stream;

use orchester_protokoll::{Capability, Event, Task, TaskKind};
use orchester_vertrag::{AdapterError, AgentAdapter, EventStream};

/// Scripted, subprocess-free adapter.
pub struct MockAdapter;

impl MockAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentAdapter for MockAdapter {
    fn name(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> Capability {
        Capability {
            name: "mock".to_string(),
            kinds: vec![TaskKind::Chat],
            supports_resume: false,
            streaming: true,
        }
    }

    async fn run(&self, task: Task) -> Result<EventStream, AdapterError> {
        // Echo the prompt back through the normal event shape so tests can assert
        // the payload survived the pipeline unchanged.
        let events: Vec<Result<Event, AdapterError>> = vec![
            Ok(Event::SessionStarted {
                session_id: "mock-session".to_string(),
            }),
            Ok(Event::TurnStarted),
            Ok(Event::Message {
                text: format!("mock received: {}", task.prompt),
            }),
            Ok(Event::TurnCompleted),
            Ok(Event::Result {
                text: format!("mock done: {}", task.prompt),
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::path::PathBuf;

    #[tokio::test]
    async fn emits_scripted_sequence() {
        let adapter = MockAdapter::new();
        let task = Task::new("hi", PathBuf::from("."));
        let stream = adapter.run(task).await.expect("mock run");
        let events: Vec<_> = stream.map(|e| e.unwrap()).collect().await;

        assert!(matches!(events[0], Event::SessionStarted { .. }));
        assert!(matches!(events[1], Event::TurnStarted));
        assert!(matches!(&events[2], Event::Message { text } if text.contains("hi")));
        assert!(matches!(events[3], Event::TurnCompleted));
        assert!(matches!(&events[4], Event::Result { text } if text.contains("hi")));
    }
}
