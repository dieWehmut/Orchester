use crate::{LanguageModel, ModelError, ModelItem, ModelRequest, ModelResponse, ModelRole};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

/// A request summary safe to retain in deterministic test diagnostics.
///
/// It deliberately contains counts, role/tool names, and the store flag only;
/// message text, tool output, opaque values, argument JSON, and credentials are
/// never copied into this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSummary {
    pub model: String,
    pub message_count: usize,
    pub message_roles: Vec<ModelRole>,
    pub item_count: usize,
    pub text_item_count: usize,
    pub tool_call_item_count: usize,
    pub tool_result_item_count: usize,
    pub opaque_item_count: usize,
    pub tool_definition_names: Vec<String>,
    pub tool_call_names: Vec<String>,
    pub store: bool,
}

impl From<&ModelRequest> for RequestSummary {
    fn from(request: &ModelRequest) -> Self {
        let mut summary = Self {
            model: request.model.clone(),
            message_count: request.messages.len(),
            message_roles: request
                .messages
                .iter()
                .map(|message| message.role)
                .collect(),
            item_count: 0,
            text_item_count: 0,
            tool_call_item_count: 0,
            tool_result_item_count: 0,
            opaque_item_count: 0,
            tool_definition_names: request.tools.iter().map(|tool| tool.name.clone()).collect(),
            tool_call_names: Vec::new(),
            store: request.store,
        };

        for message in &request.messages {
            summary.item_count += message.items.len();
            for item in &message.items {
                match item {
                    ModelItem::Text(_) => summary.text_item_count += 1,
                    ModelItem::ToolCall(call) => {
                        summary.tool_call_item_count += 1;
                        summary.tool_call_names.push(call.name.clone());
                    }
                    ModelItem::ToolResult { .. } => summary.tool_result_item_count += 1,
                    ModelItem::Opaque(_) => summary.opaque_item_count += 1,
                }
            }
        }

        summary
    }
}

/// A deterministic, provider-free implementation of [`LanguageModel`].
pub struct ScriptedLlm {
    responses: Mutex<VecDeque<Result<ModelResponse, ModelError>>>,
    calls: AtomicUsize,
    summaries: Mutex<Vec<RequestSummary>>,
}

impl ScriptedLlm {
    pub fn new<I>(responses: I) -> Self
    where
        I: IntoIterator<Item = Result<ModelResponse, ModelError>>,
    {
        Self {
            responses: Mutex::new(responses.into_iter().collect()),
            calls: AtomicUsize::new(0),
            summaries: Mutex::new(Vec::new()),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    pub fn request_summaries(&self) -> Vec<RequestSummary> {
        lock_unpoisoned(&self.summaries).clone()
    }

    /// Alias useful when callers treat the summaries as a diagnostic log.
    pub fn summaries(&self) -> Vec<RequestSummary> {
        self.request_summaries()
    }
}

#[async_trait]
impl LanguageModel for ScriptedLlm {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        lock_unpoisoned(&self.summaries).push(RequestSummary::from(&request));
        self.calls.fetch_add(1, Ordering::SeqCst);

        lock_unpoisoned(&self.responses)
            .pop_front()
            .unwrap_or(Err(ModelError::ScriptExhausted))
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
