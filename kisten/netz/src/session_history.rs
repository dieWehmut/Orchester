use std::fmt;

use axum::{
    extract::{rejection::QueryRejection, Query, State},
    http::HeaderMap,
    Json,
};
use orchester_anwendung::{SessionHistoryDetail, SessionHistoryPage, SessionHistorySummary};
use orchester_protokoll::{Outcome, Usage};
use serde::{Deserialize, Serialize};

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::ServerContext,
    health::no_store_headers,
};

pub const SESSION_LIST_DEFAULT_LIMIT: usize = 20;
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

#[derive(Debug, Default, Deserialize)]
pub(crate) struct SessionListQuery {
    cursor: Option<String>,
    limit: Option<usize>,
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

pub(crate) async fn session_list_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    query: Result<Query<SessionListQuery>, QueryRejection>,
) -> Result<(HeaderMap, Json<SessionPageDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let query = query
        .map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?
        .0;
    let limit = query.limit.unwrap_or(SESSION_LIST_DEFAULT_LIMIT);
    if !(1..=100).contains(&limit) {
        return Err(api_error_response(ApiErrorCode::BadRequest, request_id));
    }

    let history = context
        .session_history()
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;
    let page = history
        .page(query.cursor.as_deref(), limit)
        .map_err(|error| {
            let code = match error {
                orchester_anwendung::SessionHistoryError::InvalidCursor => ApiErrorCode::BadRequest,
                orchester_anwendung::SessionHistoryError::Io(_)
                | orchester_anwendung::SessionHistoryError::NotFound => ApiErrorCode::Unavailable,
            };
            api_error_response(code, request_id)
        })?;
    Ok((no_store_headers(), Json(session_page_response(&page))))
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
