use std::fmt;
use std::path::PathBuf;

use orchester_laufzeit::{SessionRecord, SessionStore};
use orchester_protokoll::{Outcome, Usage};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_PAGE_SIZE: usize = 100;
const ID_PREFIX: &str = "s-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHistory {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHistoryPage {
    pub items: Vec<SessionHistorySummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHistorySummary {
    pub id: String,
    pub recorded_at_unix: u64,
    pub title: String,
    pub agent: String,
    pub model: Option<String>,
    pub outcome: Outcome,
    /// Reserved for a future explicit delegate-resume contract. This history
    /// endpoint never treats a native vendor session ID as a resume handle.
    pub resumable: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionHistoryDetail {
    pub summary: SessionHistorySummary,
    pub prompt: String,
    pub final_text: String,
    pub usage: Usage,
}

impl fmt::Debug for SessionHistoryDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionHistoryDetail")
            .field("summary", &self.summary)
            .field("prompt", &"[REDACTED]")
            .field("final_text", &"[REDACTED]")
            .field("usage", &self.usage)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum SessionHistoryError {
    #[error("session history could not be read")]
    Io(#[source] std::io::Error),
    #[error("session history cursor is invalid")]
    InvalidCursor,
    #[error("session history item was not found")]
    NotFound,
}

impl SessionHistory {
    pub fn for_paths(paths: &crate::OrchesterPaths) -> Self {
        Self::new(paths.session_log())
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn page(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<SessionHistoryPage, SessionHistoryError> {
        let records = self.load_records()?;
        let summaries = summaries(&records);
        let start = match cursor {
            None => 0,
            Some(cursor) => summaries
                .iter()
                .position(|item| item.id == cursor)
                .map(|index| index + 1)
                .ok_or(SessionHistoryError::InvalidCursor)?,
        };
        let page_size = limit.clamp(1, MAX_PAGE_SIZE);
        let items = summaries
            .into_iter()
            .skip(start)
            .take(page_size)
            .collect::<Vec<_>>();
        let next_cursor = (start + items.len() < records.len())
            .then(|| items.last().map(|item| item.id.clone()))
            .flatten();
        Ok(SessionHistoryPage { items, next_cursor })
    }

    pub fn detail(&self, id: &str) -> Result<SessionHistoryDetail, SessionHistoryError> {
        let records = self.load_records()?;
        for (index, record) in records.iter().enumerate() {
            if opaque_id(index, record) == id {
                return Ok(SessionHistoryDetail {
                    summary: summary(index, record),
                    prompt: record.prompt.clone(),
                    final_text: record.final_text.clone(),
                    usage: record.usage,
                });
            }
        }
        Err(SessionHistoryError::NotFound)
    }

    fn load_records(&self) -> Result<Vec<SessionRecord>, SessionHistoryError> {
        SessionStore::new(&self.path)
            .load()
            .map_err(SessionHistoryError::Io)
    }
}

fn summaries(records: &[SessionRecord]) -> Vec<SessionHistorySummary> {
    records
        .iter()
        .enumerate()
        .rev()
        .map(|(index, record)| summary(index, record))
        .collect()
}

fn summary(index: usize, record: &SessionRecord) -> SessionHistorySummary {
    SessionHistorySummary {
        id: opaque_id(index, record),
        recorded_at_unix: record.recorded_at_unix,
        title: compact_title(&record.prompt),
        agent: bounded_text(&record.agent, 80),
        model: record
            .model
            .as_deref()
            .map(|value| bounded_text(value, 120)),
        outcome: record.outcome,
        resumable: false,
    }
}

fn opaque_id(index: usize, record: &SessionRecord) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-session-history-v1\0");
    hasher.update((index as u64).to_be_bytes());
    hash_field(&mut hasher, &record.recorded_at_unix.to_be_bytes());
    hash_field(&mut hasher, record.agent.as_bytes());
    hash_field(
        &mut hasher,
        record.session_id.as_deref().unwrap_or_default().as_bytes(),
    );
    hash_field(&mut hasher, record.prompt.as_bytes());
    hash_field(
        &mut hasher,
        record.model.as_deref().unwrap_or_default().as_bytes(),
    );
    hash_field(&mut hasher, outcome_name(record.outcome).as_bytes());
    hash_field(&mut hasher, record.final_text.as_bytes());
    for usage in [
        record.usage.input_tokens,
        record.usage.output_tokens,
        record.usage.cached_input_tokens,
        record.usage.reasoning_output_tokens,
    ] {
        hash_field(&mut hasher, &usage.to_be_bytes());
    }
    let digest = hasher.finalize();
    let encoded = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{ID_PREFIX}{encoded}")
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Success => "success",
        Outcome::Failed => "failed",
        Outcome::Cancelled => "cancelled",
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn compact_title(value: &str) -> String {
    let first_line = value.lines().next().unwrap_or_default().trim();
    let bounded = bounded_text(first_line, 120);
    if bounded.is_empty() {
        "untitled session".to_owned()
    } else {
        bounded
    }
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut output = value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>();
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output
}
