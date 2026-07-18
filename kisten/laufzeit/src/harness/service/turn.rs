use std::fmt;

use orchester_modell::ModelUsage;
use orchester_protokoll::{ActionId, AgentAction, CallId, RunId};

use super::super::governance::PolicyResult;

/// The bounded result of one self-agent model step.
pub enum SelfAgentTurn {
    Text {
        run_id: RunId,
        text: String,
        model_calls: u32,
        usage: ModelUsage,
    },
    Action {
        run_id: RunId,
        action_id: ActionId,
        call_id: CallId,
        action: AgentAction,
        policy: PolicyResult,
        model_calls: u32,
        usage: ModelUsage,
    },
}

impl SelfAgentTurn {
    pub fn run_id(&self) -> &RunId {
        match self {
            Self::Text { run_id, .. } | Self::Action { run_id, .. } => run_id,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            Self::Action { .. } => None,
        }
    }

    pub fn action(&self) -> Option<&AgentAction> {
        match self {
            Self::Text { .. } => None,
            Self::Action { action, .. } => Some(action),
        }
    }

    pub fn policy(&self) -> Option<&PolicyResult> {
        match self {
            Self::Text { .. } => None,
            Self::Action { policy, .. } => Some(policy),
        }
    }

    pub fn model_calls(&self) -> u32 {
        match self {
            Self::Text { model_calls, .. } | Self::Action { model_calls, .. } => *model_calls,
        }
    }

    pub fn usage(&self) -> ModelUsage {
        match self {
            Self::Text { usage, .. } | Self::Action { usage, .. } => *usage,
        }
    }
}

impl fmt::Debug for SelfAgentTurn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text {
                model_calls,
                usage,
                text,
                ..
            } => formatter
                .debug_struct("Text")
                .field("text_bytes", &text.len())
                .field("model_calls", model_calls)
                .field("usage", usage)
                .finish(),
            Self::Action {
                action,
                policy,
                model_calls,
                usage,
                ..
            } => formatter
                .debug_struct("Action")
                .field("action_summary", &action.action_summary())
                .field("policy_decision", &policy.decision)
                .field("policy_rule_id", &policy.rule_id)
                .field("model_calls", model_calls)
                .field("usage", usage)
                .finish(),
        }
    }
}
