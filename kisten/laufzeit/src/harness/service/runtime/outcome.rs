use std::fmt;

use orchester_modell::ModelUsage;
use orchester_protokoll::{ActionId, CallId, RunId};

use super::super::SelfAgentTurn;
use crate::harness::execution::GovernedToolOutcome;

/// One governed tool boundary crossed while automatically advancing a run.
pub struct SelfAgentToolStep {
    action_id: ActionId,
    call_id: CallId,
    outcome: GovernedToolOutcome,
}

impl SelfAgentToolStep {
    pub(super) fn new(action_id: ActionId, call_id: CallId, outcome: GovernedToolOutcome) -> Self {
        Self {
            action_id,
            call_id,
            outcome,
        }
    }

    pub fn action_id(&self) -> &ActionId {
        &self.action_id
    }

    pub fn call_id(&self) -> &CallId {
        &self.call_id
    }

    pub fn outcome(&self) -> &GovernedToolOutcome {
        &self.outcome
    }
}

impl fmt::Debug for SelfAgentToolStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentToolStep")
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

/// The bounded trace and final model boundary produced by one automatic run.
pub struct SelfAgentRunOutcome {
    final_turn: SelfAgentTurn,
    tool_steps: Vec<SelfAgentToolStep>,
}

impl SelfAgentRunOutcome {
    pub(super) fn new(final_turn: SelfAgentTurn, tool_steps: Vec<SelfAgentToolStep>) -> Self {
        Self {
            final_turn,
            tool_steps,
        }
    }

    pub fn run_id(&self) -> &RunId {
        self.final_turn.run_id()
    }

    pub fn final_turn(&self) -> &SelfAgentTurn {
        &self.final_turn
    }

    pub fn tool_steps(&self) -> &[SelfAgentToolStep] {
        &self.tool_steps
    }

    pub fn model_calls(&self) -> u32 {
        self.final_turn.model_calls()
    }

    pub fn usage(&self) -> ModelUsage {
        self.final_turn.usage()
    }
}

impl fmt::Debug for SelfAgentRunOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentRunOutcome")
            .field("final_turn", &self.final_turn)
            .field("tool_step_count", &self.tool_steps.len())
            .finish_non_exhaustive()
    }
}
