//! Runtime-owned lifecycle enforcement for model event streams.
//!
//! Provider adapters may override `LanguageModel::complete_with_events`, so
//! the runtime cannot rely on them to emit a complete lifecycle. This wrapper
//! owns the lifecycle observed by callers while still forwarding provider
//! text deltas as they arrive.

use std::sync::{Arc, Mutex};

use orchester_modell::{LanguageModel, ModelError, ModelEventSink, ModelRequest, ModelResponse};
use tokio_util::sync::CancellationToken;

pub(crate) async fn complete_model_call<M: LanguageModel + ?Sized>(
    model: &M,
    request: ModelRequest,
    cancel: CancellationToken,
    events: Option<Arc<dyn ModelEventSink>>,
) -> Result<ModelResponse, ModelError> {
    let Some(events) = events else {
        return model.complete_with_events(request, cancel, None).await;
    };

    let lifecycle = Arc::new(LifecycleSink::new(events));
    lifecycle.start_once();
    let provider_events: Arc<dyn ModelEventSink> = lifecycle.clone();
    let result = model
        .complete_with_events(request, cancel, Some(provider_events))
        .await;

    match result {
        Ok(response) => {
            lifecycle.complete_success(&response.assistant_text);
            Ok(response)
        }
        Err(error) => {
            lifecycle.abort();
            Err(error)
        }
    }
}

struct LifecycleSink {
    downstream: Arc<dyn ModelEventSink>,
    state: Mutex<LifecycleState>,
}

struct LifecycleState {
    started: bool,
    saw_text: bool,
    terminal: bool,
}

impl LifecycleSink {
    fn new(downstream: Arc<dyn ModelEventSink>) -> Self {
        Self {
            downstream,
            state: Mutex::new(LifecycleState {
                started: false,
                saw_text: false,
                terminal: false,
            }),
        }
    }

    fn start_once(&self) {
        let mut state = self.state.lock().expect("model event lifecycle lock");
        if state.terminal || state.started {
            return;
        }
        state.started = true;
        self.downstream.response_started();
    }

    fn complete_success(&self, assistant_text: &str) {
        let mut state = self.state.lock().expect("model event lifecycle lock");
        if state.terminal {
            return;
        }
        if !state.saw_text && !assistant_text.is_empty() {
            state.saw_text = true;
            self.downstream.text_delta(assistant_text);
        }
        state.terminal = true;
        self.downstream.response_completed();
    }

    fn abort(&self) {
        self.state
            .lock()
            .expect("model event lifecycle lock")
            .terminal = true;
    }
}

impl ModelEventSink for LifecycleSink {
    fn response_started(&self) {
        self.start_once();
    }

    fn text_delta(&self, delta: &str) {
        let mut state = self.state.lock().expect("model event lifecycle lock");
        if state.terminal {
            return;
        }
        if !state.started {
            state.started = true;
            self.downstream.response_started();
        }
        if !delta.is_empty() {
            state.saw_text = true;
        }
        self.downstream.text_delta(delta);
    }

    fn response_completed(&self) {
        // Completion is intentionally owned by `complete_model_call`, which
        // can distinguish a successful result from an error or cancellation.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use orchester_modell::{ModelItem, ModelMessage, ModelRole, ModelUsage};

    #[derive(Default)]
    struct RecordingSink(Mutex<Vec<String>>);

    impl ModelEventSink for RecordingSink {
        fn response_started(&self) {
            self.0
                .lock()
                .expect("recording sink lock")
                .push("<start>".into());
        }

        fn text_delta(&self, delta: &str) {
            self.0
                .lock()
                .expect("recording sink lock")
                .push(delta.to_owned());
        }

        fn response_completed(&self) {
            self.0
                .lock()
                .expect("recording sink lock")
                .push("<complete>".into());
        }
    }

    struct LifecycleModel {
        result: Result<ModelResponse, ModelError>,
        emit_text: bool,
        emit_completion: bool,
    }

    #[async_trait]
    impl LanguageModel for LifecycleModel {
        async fn complete(
            &self,
            _request: ModelRequest,
            _cancel: CancellationToken,
        ) -> Result<ModelResponse, ModelError> {
            self.result.clone()
        }

        async fn complete_with_events(
            &self,
            _request: ModelRequest,
            _cancel: CancellationToken,
            events: Option<Arc<dyn ModelEventSink>>,
        ) -> Result<ModelResponse, ModelError> {
            if let Some(events) = events {
                events.response_started();
                if self.emit_text {
                    events.text_delta("partial");
                }
                if self.emit_completion {
                    events.response_completed();
                }
            }
            self.result.clone()
        }
    }

    fn response(text: &str) -> ModelResponse {
        ModelResponse {
            assistant_text: text.to_owned(),
            tool_call: None,
            usage: ModelUsage::default(),
            opaque_items: Vec::new(),
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            model: "test".into(),
            messages: vec![ModelMessage {
                role: ModelRole::User,
                items: vec![ModelItem::Text("hello".into())],
            }],
            tools: Vec::new(),
            store: false,
        }
    }

    #[tokio::test]
    async fn supplements_missing_lifecycle_and_avoids_duplicate_final_text() {
        let sink = Arc::new(RecordingSink::default());
        let result = complete_model_call(
            &LifecycleModel {
                result: Ok(response("partial")),
                emit_text: true,
                emit_completion: true,
            },
            request(),
            CancellationToken::new(),
            Some(sink.clone()),
        )
        .await
        .expect("model call");

        assert_eq!(result.assistant_text, "partial");
        assert_eq!(
            *sink.0.lock().expect("recording sink lock"),
            ["<start>", "partial", "<complete>"]
        );
    }

    #[tokio::test]
    async fn emits_legacy_final_text_once_when_no_delta_arrives() {
        let sink = Arc::new(RecordingSink::default());
        complete_model_call(
            &LifecycleModel {
                result: Ok(response("legacy")),
                emit_text: false,
                emit_completion: false,
            },
            request(),
            CancellationToken::new(),
            Some(sink.clone()),
        )
        .await
        .expect("model call");

        assert_eq!(
            *sink.0.lock().expect("recording sink lock"),
            ["<start>", "legacy", "<complete>"]
        );
    }

    #[tokio::test]
    async fn suppresses_completion_when_provider_returns_error() {
        let sink = Arc::new(RecordingSink::default());
        let error = complete_model_call(
            &LifecycleModel {
                result: Err(ModelError::Transport),
                emit_text: true,
                emit_completion: true,
            },
            request(),
            CancellationToken::new(),
            Some(sink.clone()),
        )
        .await
        .expect_err("model error");

        assert_eq!(error, ModelError::Transport);
        assert_eq!(
            *sink.0.lock().expect("recording sink lock"),
            ["<start>", "partial"]
        );
    }
}
