use std::fmt;

use orchester_anwendung::{SessionHistoryDetail, SessionHistoryPage, SessionHistorySummary};
use orchester_protokoll::{Outcome, Usage};
use serde::Serialize;

pub const SESSION_HISTORY_SCHEMA_VERSION: u8 = 1;
pub const SESSION_PROMPT_MAX_CHARS: usize = 65_536;
pub const SESSION_RESULT_MAX_CHARS: usize = 262_144;

const SESSION_TITLE_MAX_CHARS: usize = 120;
const SESSION_AGENT_MAX_CHARS: usize = 80;
const SESSION_MODEL_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcomeDto {
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionSummaryDto {
    pub id: String,
    pub source: &'static str,
    pub recorded_at_unix: u64,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub outcome: SessionOutcomeDto,
    pub resumable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionPageDto {
    pub schema_version: u8,
    pub items: Vec<SessionSummaryDto>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct SessionDetailDto {
    pub schema_version: u8,
    pub id: String,
    pub source: &'static str,
    pub recorded_at_unix: u64,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub outcome: SessionOutcomeDto,
    pub resumable: bool,
    pub prompt: String,
    pub final_text: String,
    pub usage: Usage,
}

impl fmt::Debug for SessionDetailDto {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionDetailDto")
            .field("schema_version", &self.schema_version)
            .field("id", &self.id)
            .field("source", &self.source)
            .field("recorded_at_unix", &self.recorded_at_unix)
            .field("title", &self.title)
            .field("agent", &self.agent)
            .field("model", &self.model)
            .field("outcome", &self.outcome)
            .field("resumable", &self.resumable)
            .field("prompt", &"[REDACTED]")
            .field("final_text", &"[REDACTED]")
            .field("usage", &self.usage)
            .finish()
    }
}

pub fn session_page_response(page: &SessionHistoryPage) -> SessionPageDto {
    SessionPageDto {
        schema_version: SESSION_HISTORY_SCHEMA_VERSION,
        items: page.items.iter().map(session_summary_response).collect(),
        next_cursor: page.next_cursor.clone(),
    }
}

pub fn session_detail_response(detail: &SessionHistoryDetail) -> SessionDetailDto {
    let summary = session_summary_response(&detail.summary);
    SessionDetailDto {
        schema_version: SESSION_HISTORY_SCHEMA_VERSION,
        id: summary.id,
        source: summary.source,
        recorded_at_unix: summary.recorded_at_unix,
        title: summary.title,
        agent: summary.agent,
        model: summary.model,
        outcome: summary.outcome,
        resumable: summary.resumable,
        prompt: bounded_text(&detail.prompt, SESSION_PROMPT_MAX_CHARS),
        final_text: bounded_text(&detail.final_text, SESSION_RESULT_MAX_CHARS),
        usage: detail.usage,
    }
}

fn session_summary_response(summary: &SessionHistorySummary) -> SessionSummaryDto {
    SessionSummaryDto {
        id: summary.id.clone(),
        source: "delegate",
        recorded_at_unix: summary.recorded_at_unix,
        title: bounded_text(&summary.title, SESSION_TITLE_MAX_CHARS),
        agent: bounded_text(&summary.agent, SESSION_AGENT_MAX_CHARS),
        model: summary
            .model
            .as_deref()
            .map(|model| bounded_text(model, SESSION_MODEL_MAX_CHARS)),
        outcome: match summary.outcome {
            Outcome::Success => SessionOutcomeDto::Success,
            Outcome::Failed => SessionOutcomeDto::Failed,
            Outcome::Cancelled => SessionOutcomeDto::Cancelled,
        },
        resumable: summary.resumable,
    }
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let character_count = value.chars().count();
    if character_count <= max_chars {
        return value.to_owned();
    }

    let content_limit = max_chars.saturating_sub(3);
    let mut output = value.chars().take(content_limit).collect::<String>();
    output.push_str("...");
    output
}
