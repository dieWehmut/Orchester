//! Bounded, provider-neutral coordination for the self-owned agent.
//!
//! This layer deliberately stops at a decoded pending action. Governance,
//! durable recording, approval, and tool execution consume that action in
//! later layers before returning one bounded observation to `resume`.

use std::fmt;

use orchester_modell::{
    ActionDecoder, DecodeError, LanguageModel, ModelError, ModelRequest, ModelUsage,
    MAX_CONTENT_BYTES,
};
use orchester_protokoll::{AgentAction, CallId};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::context::{
    ContextAssembler, ContextError, ContextInput, ContinuationInput, TranscriptEntry,
};

const MAX_MODEL_BYTES: usize = 4 * 1024;
const MAX_AGENT_STEPS: u32 = 256;

#[derive(Clone, PartialEq, Eq)]
pub struct AgentLoopConfig {
    pub model: String,
    pub max_steps: u32,
    pub max_text_bytes: usize,
    pub store: bool,
}

impl fmt::Debug for AgentLoopConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentLoopConfig")
            .field("model", &"<redacted>")
            .field("max_steps", &self.max_steps)
            .field("max_text_bytes", &self.max_text_bytes)
            .field("store", &self.store)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentLoopStop {
    AssistantText,
    FinishTool,
}

pub struct AgentLoopResult {
    final_text: String,
    stop: AgentLoopStop,
    model_calls: u32,
    usage: ModelUsage,
}

impl AgentLoopResult {
    pub fn final_text(&self) -> &str {
        &self.final_text
    }

    pub fn stop(&self) -> AgentLoopStop {
        self.stop
    }

    pub fn model_calls(&self) -> u32 {
        self.model_calls
    }

    pub fn usage(&self) -> ModelUsage {
        self.usage
    }
}

impl fmt::Debug for AgentLoopResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentLoopResult")
            .field("final_text_bytes", &self.final_text.len())
            .field("stop", &self.stop)
            .field("model_calls", &self.model_calls)
            .field("usage", &self.usage)
            .finish()
    }
}

pub enum AgentLoopOutcome {
    Final(AgentLoopResult),
    Pending(PendingAction),
}

impl fmt::Debug for AgentLoopOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Final(result) => formatter.debug_tuple("Final").field(result).finish(),
            Self::Pending(action) => formatter.debug_tuple("Pending").field(action).finish(),
        }
    }
}

pub struct PendingAction {
    call_id: CallId,
    action: AgentAction,
    state: LoopState,
}

impl PendingAction {
    pub fn call_id(&self) -> &CallId {
        &self.call_id
    }

    pub fn action(&self) -> &AgentAction {
        &self.action
    }

    pub fn model_calls(&self) -> u32 {
        self.state.model_calls
    }
}

impl fmt::Debug for PendingAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingAction")
            .field("call_id", &"<redacted>")
            .field("action_summary", &self.action.action_summary())
            .field("model_calls", &self.state.model_calls)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum AgentLoopError {
    #[error("agent loop configuration is invalid")]
    InvalidConfig,
    #[error("agent model step budget is exhausted")]
    StepBudgetExceeded,
    #[error("agent model response is empty")]
    EmptyResponse,
    #[error("agent model text exceeds its configured limit")]
    ModelTextTooLarge,
    #[error("agent tool result exceeds its configured limit")]
    ToolResultTooLarge,
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

struct LoopState {
    history: Vec<TranscriptEntry>,
    model_calls: u32,
    usage: ModelUsage,
}

pub struct SelfAgentLoop<M> {
    model: M,
    context: ContextAssembler,
    config: AgentLoopConfig,
}

impl<M> fmt::Debug for SelfAgentLoop<M> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentLoop")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<M: LanguageModel> SelfAgentLoop<M> {
    pub fn new(
        model: M,
        context: ContextAssembler,
        config: AgentLoopConfig,
    ) -> Result<Self, AgentLoopError> {
        if config.model.trim().is_empty()
            || config.model.len() > MAX_MODEL_BYTES
            || config.model.chars().any(char::is_control)
            || config.max_steps == 0
            || config.max_steps > MAX_AGENT_STEPS
            || config.max_text_bytes == 0
            || config.max_text_bytes > MAX_CONTENT_BYTES
        {
            return Err(AgentLoopError::InvalidConfig);
        }
        Ok(Self {
            model,
            context,
            config,
        })
    }

    pub fn model(&self) -> &M {
        &self.model
    }

    pub async fn start(
        &self,
        prompt: impl Into<String>,
        cancel: CancellationToken,
    ) -> Result<AgentLoopOutcome, AgentLoopError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled.into());
        }
        let prompt = prompt.into();
        let assembled = self.context.assemble(ContextInput {
            model: self.config.model.clone(),
            prompt: prompt.clone(),
            history: Vec::new(),
            store: self.config.store,
        })?;
        let state = LoopState {
            history: vec![TranscriptEntry::user(prompt)],
            model_calls: 0,
            usage: ModelUsage::default(),
        };
        self.complete_step(assembled.request, state, cancel).await
    }

    pub async fn resume(
        &self,
        pending: PendingAction,
        tool_result: impl Into<String>,
        cancel: CancellationToken,
    ) -> Result<AgentLoopOutcome, AgentLoopError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled.into());
        }
        if pending.state.model_calls >= self.config.max_steps {
            return Err(AgentLoopError::StepBudgetExceeded);
        }
        let tool_result = tool_result.into();
        if tool_result.len() > self.config.max_text_bytes {
            return Err(AgentLoopError::ToolResultTooLarge);
        }

        let mut state = pending.state;
        state
            .history
            .push(TranscriptEntry::tool_result(pending.call_id, tool_result));
        let assembled = self.context.assemble_continuation(ContinuationInput {
            model: self.config.model.clone(),
            history: state.history.clone(),
            store: self.config.store,
        })?;
        if assembled.omitted_entries > 0 {
            state.history.drain(..assembled.omitted_entries);
        }
        self.complete_step(assembled.request, state, cancel).await
    }

    async fn complete_step(
        &self,
        request: ModelRequest,
        mut state: LoopState,
        cancel: CancellationToken,
    ) -> Result<AgentLoopOutcome, AgentLoopError> {
        if state.model_calls >= self.config.max_steps {
            return Err(AgentLoopError::StepBudgetExceeded);
        }
        let response = self.model.complete(request, cancel).await?;
        state.model_calls += 1;
        state.usage.input_tokens = state
            .usage
            .input_tokens
            .saturating_add(response.usage.input_tokens);
        state.usage.output_tokens = state
            .usage
            .output_tokens
            .saturating_add(response.usage.output_tokens);

        if response.assistant_text.len() > self.config.max_text_bytes {
            return Err(AgentLoopError::ModelTextTooLarge);
        }
        let assistant_text = response.assistant_text;
        if !assistant_text.is_empty() {
            state
                .history
                .push(TranscriptEntry::assistant(assistant_text.clone()));
        }

        let Some(call) = response.tool_call else {
            if assistant_text.trim().is_empty() {
                return Err(AgentLoopError::EmptyResponse);
            }
            return Ok(AgentLoopOutcome::Final(AgentLoopResult {
                final_text: assistant_text,
                stop: AgentLoopStop::AssistantText,
                model_calls: state.model_calls,
                usage: state.usage,
            }));
        };

        let action = ActionDecoder.decode(&call)?;
        let call_id = call.call_id.clone();
        state.history.push(TranscriptEntry::tool_call(
            call.call_id,
            call.name,
            call.arguments_json,
        ));
        if let AgentAction::Finish { summary } = action {
            if summary.len() > self.config.max_text_bytes {
                return Err(AgentLoopError::ModelTextTooLarge);
            }
            return Ok(AgentLoopOutcome::Final(AgentLoopResult {
                final_text: summary,
                stop: AgentLoopStop::FinishTool,
                model_calls: state.model_calls,
                usage: state.usage,
            }));
        }

        Ok(AgentLoopOutcome::Pending(PendingAction {
            call_id,
            action,
            state,
        }))
    }
}
