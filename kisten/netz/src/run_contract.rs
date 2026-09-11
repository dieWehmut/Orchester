//! Versioned REST and WebSocket shapes for one local run.
//!
//! The browser package owns the written contract.  These Rust types mirror it
//! exactly and keep request validation next to the wire shapes so handlers do not
//! grow ad-hoc string checks.

use orchester_protokoll::{RunId, StopReason, UiApprovalRequest, UiEventEnvelope, Usage};
use serde::{Deserialize, Serialize};

pub const RUN_SCHEMA_VERSION: u8 = 1;
pub const RUN_PROMPT_MAX_CHARS: usize = 32_000;
pub const RUN_RESUME_MAX_CHARS: usize = 512;
pub const RUN_REPLAY_DEFAULT_LIMIT: u32 = 200;
pub const RUN_REPLAY_MAX_LIMIT: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRunRequest {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

impl StartRunRequest {
    pub fn validate(&self) -> Result<(), RunRequestValidationError> {
        validate_text(
            &self.prompt,
            RUN_PROMPT_MAX_CHARS,
            RunRequestValidationError::EmptyPrompt,
            RunRequestValidationError::PromptTooLong,
        )?;
        if let Some(resume) = &self.resume {
            validate_text(
                resume,
                RUN_RESUME_MAX_CHARS,
                RunRequestValidationError::EmptyResume,
                RunRequestValidationError::ResumeTooLong,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartRunResponse {
    pub run_id: RunId,
    pub events_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStateDto {
    Created,
    Running,
    AwaitingApproval,
    Validating,
    Succeeded,
    Failed,
    Cancelled,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSnapshotDto {
    pub run_id: RunId,
    pub state: RunStateDto,
    pub events: Vec<UiEventEnvelope>,
    pub pending_approvals: Vec<UiApprovalRequest>,
    pub oldest_sequence: u64,
    pub latest_sequence: u64,
    pub next_sequence: u64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunReplayRequestDto {
    pub after_sequence: u64,
    #[serde(default)]
    pub limit: Option<u32>,
}

impl RunReplayRequestDto {
    pub fn bounded_limit(&self) -> Result<u32, RunRequestValidationError> {
        let limit = self.limit.unwrap_or(RUN_REPLAY_DEFAULT_LIMIT);
        if limit == 0 || limit > RUN_REPLAY_MAX_LIMIT {
            return Err(RunRequestValidationError::InvalidReplayLimit);
        }
        Ok(limit)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunReplayResponseDto {
    pub run_id: RunId,
    pub events: Vec<UiEventEnvelope>,
    pub first_sequence: Option<u64>,
    pub last_sequence: Option<u64>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResyncReason {
    RetentionExceeded,
    SequenceGap,
    SchemaMismatch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunStreamFrameDto {
    Event { event: UiEventEnvelope },
    ResyncRequired {
        run_id: RunId,
        requested_after_sequence: u64,
        oldest_sequence: u64,
        latest_sequence: u64,
        reason: ResyncReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunSummaryDto {
    pub run_id: RunId,
    pub usage: Usage,
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunRequestValidationError {
    EmptyPrompt,
    PromptTooLong,
    EmptyResume,
    ResumeTooLong,
    InvalidReplayLimit,
}

impl std::fmt::Display for RunRequestValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::EmptyPrompt => "prompt must not be empty",
            Self::PromptTooLong => "prompt exceeds the maximum length",
            Self::EmptyResume => "resume handle must not be empty",
            Self::ResumeTooLong => "resume handle exceeds the maximum length",
            Self::InvalidReplayLimit => "replay limit is outside the allowed range",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RunRequestValidationError {}

fn validate_text(
    value: &str,
    max_chars: usize,
    empty: RunRequestValidationError,
    too_long: RunRequestValidationError,
) -> Result<(), RunRequestValidationError> {
    if value.trim().is_empty() {
        return Err(empty);
    }
    if value.chars().count() > max_chars {
        return Err(too_long);
    }
    if value.chars().any(char::is_control) {
        return Err(RunRequestValidationError::PromptTooLong);
    }
    Ok(())
}

pub fn state_from_stop_reason(reason: &StopReason) -> RunStateDto {
    match reason {
        StopReason::Succeeded => RunStateDto::Succeeded,
        StopReason::Cancelled => RunStateDto::Cancelled,
        StopReason::AwaitingApproval => RunStateDto::AwaitingApproval,
        StopReason::BudgetExceeded
        | StopReason::RepeatedFailure
        | StopReason::InterruptedUnknownOutcome
        | StopReason::Failed => RunStateDto::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_request_rejects_blank_and_control_prompt() {
        assert_eq!(
            StartRunRequest {
                prompt: "  ".into(),
                resume: None,
            }
            .validate(),
            Err(RunRequestValidationError::EmptyPrompt)
        );
        assert!(matches!(
            StartRunRequest {
                prompt: "hello\nworld".into(),
                resume: None,
            }
            .validate(),
            Err(RunRequestValidationError::PromptTooLong)
        ));
    }

    #[test]
    fn replay_limit_defaults_and_has_a_hard_cap() {
        assert_eq!(
            RunReplayRequestDto {
                after_sequence: 0,
                limit: None,
            }
            .bounded_limit()
            .unwrap(),
            RUN_REPLAY_DEFAULT_LIMIT
        );
        assert_eq!(
            RunReplayRequestDto {
                after_sequence: 0,
                limit: Some(RUN_REPLAY_MAX_LIMIT + 1),
            }
            .bounded_limit(),
            Err(RunRequestValidationError::InvalidReplayLimit)
        );
    }

    #[test]
    fn stream_frame_uses_the_written_discriminator() {
        let frame = RunStreamFrameDto::ResyncRequired {
            run_id: "run-1".into(),
            requested_after_sequence: 2,
            oldest_sequence: 4,
            latest_sequence: 8,
            reason: ResyncReason::RetentionExceeded,
        };
        let json = serde_json::to_value(frame).unwrap();
        assert_eq!(json["type"], "resync_required");
        assert_eq!(json["reason"], "retention_exceeded");
    }
}
