//! Read-only, bounded views of local provider transcripts and managed runs.
mod files;
mod managed;
mod metadata;
mod opencode;
mod transcript;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use cookie::Cookie;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use time::OffsetDateTime;

use crate::{
    SESSION_COOKIE_NAME, ServerContext,
    api_error::{ApiErrorCode, ApiErrorResponse, api_error_response, request_id_from_headers},
    health::no_store_headers,
};

const SESSION_LIMIT: usize = 24;
const ENTRY_LIMIT: usize = 100;
const TEXT_LIMIT: usize = 256 * 1024;

/// Provider-owned data locations. These are never accepted from an HTTP request.
#[derive(Clone, Default)]
pub struct AgentActivityRoots {
    pub codex: Option<PathBuf>,
    pub claude: Option<PathBuf>,
    pub opencode: Option<PathBuf>,
}

impl AgentActivityRoots {
    pub fn from_environment() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from);
        let codex = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|p| p.join(".codex")));
        let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|p| p.join(".claude")));
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|p| p.join(".local/share")));
        Self {
            codex: codex.map(|p| p.join("sessions")),
            claude: claude.map(|p| p.join("projects")),
            opencode: std::env::var_os("OPENCODE_DB")
                .map(PathBuf::from)
                .or_else(|| data.map(|p| p.join("opencode/opencode.db"))),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ActivityEntry {
    pub id: String,
    pub kind: &'static str,
    pub text: String,
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ActivityTurn {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
}

#[derive(Clone, Serialize)]
pub struct ActivitySummary {
    pub id: String,
    pub provider: String,
    pub source: &'static str,
    pub title: String,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    pub updated_at: i64,
    pub state: &'static str,
    pub preview: String,
    pub truncated: bool,
}

#[derive(Clone, Serialize)]
pub struct ActivityDetail {
    pub schema_version: u8,
    #[serde(flatten)]
    pub summary: ActivitySummary,
    pub entries: Vec<ActivityEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<ActivityTurn>,
    /// Exclusive decoded-record offset for the preceding page of Codex history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_before: Option<u64>,
    #[serde(skip)]
    content_limited: bool,
}

#[derive(Clone, Serialize)]
pub struct ActivitySource {
    pub provider: String,
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ActivityList {
    pub schema_version: u8,
    pub observed_at: i64,
    pub sources: Vec<ActivitySource>,
    pub sessions: Vec<ActivitySummary>,
}

#[derive(Default)]
struct Cache {
    refreshed: Option<Instant>,
    sources: Vec<ActivitySource>,
    sessions: Vec<ActivityDetail>,
    files: HashMap<PathBuf, files::CachedFile>,
    pages: HashMap<(PathBuf, Option<u64>), files::CachedFile>,
}

#[derive(Clone)]
pub(crate) struct AgentActivityStore {
    roots: AgentActivityRoots,
    cache: Arc<Mutex<Cache>>,
}

impl AgentActivityStore {
    fn codex_detail(&self, id: &str, before: Option<u64>) -> Result<Option<ActivityDetail>, ()> {
        let mut cache = self.cache.lock().map_err(|_| ())?;
        files::codex_detail(self.roots.codex.as_deref(), id, before, &mut cache.pages)
            .map_err(|_| ())
    }
    pub fn new(roots: AgentActivityRoots) -> Self {
        Self {
            roots,
            cache: Arc::new(Mutex::new(Cache::default())),
        }
    }

    fn snapshot(&self) -> Result<(Vec<ActivitySource>, Vec<ActivityDetail>), ()> {
        let mut cache = self.cache.lock().map_err(|_| ())?;
        if cache
            .refreshed
            .is_some_and(|at| at.elapsed() < Duration::from_millis(1500))
        {
            return Ok((cache.sources.clone(), cache.sessions.clone()));
        }
        let mut sessions = Vec::new();
        let mut sources = Vec::new();
        for (provider, root) in [("codex", &self.roots.codex), ("claude", &self.roots.claude)] {
            let (status, mut rows) =
                files::read_provider(provider, root.as_deref(), &mut cache.files);
            sources.push(ActivitySource {
                provider: provider.into(),
                status,
            });
            sessions.append(&mut rows);
        }
        let (status, mut rows) = opencode::read_provider(self.roots.opencode.as_deref());
        sources.push(ActivitySource {
            provider: "opencode".into(),
            status,
        });
        sessions.append(&mut rows);
        if cache.files.len() > 128 {
            cache.files.clear();
        }
        cache.sources = sources.clone();
        cache.sessions = sessions.clone();
        cache.refreshed = Some(Instant::now());
        Ok((sources, sessions))
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActivityQuery {
    provider: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActivityDetailQuery {
    before: Option<u64>,
}

fn authorize(context: &ServerContext, headers: &HeaderMap) -> Result<(), ApiErrorResponse> {
    let valid = headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            Cookie::split_parse(value).flatten().any(|cookie| {
                cookie.name() == SESSION_COOKIE_NAME
                    && context.sessions().validate_cookie(cookie.value())
            })
        });
    if valid {
        Ok(())
    } else {
        Err(api_error_response(
            ApiErrorCode::Unauthorized,
            request_id_from_headers(headers),
        ))
    }
}

async fn snapshot(
    context: &ServerContext,
    headers: &HeaderMap,
) -> Result<(Vec<ActivitySource>, Vec<ActivityDetail>), ApiErrorResponse> {
    let store = context.agent_activity().clone();
    let (mut sources, mut sessions) = tokio::task::spawn_blocking(move || store.snapshot())
        .await
        .ok()
        .and_then(Result::ok)
        .ok_or_else(|| {
            api_error_response(ApiErrorCode::Unavailable, request_id_from_headers(headers))
        })?;
    sources.push(ActivitySource {
        provider: "orchester".into(),
        status: "available",
    });
    sessions.extend(managed::snapshot(context).await);
    let now = now_ms();
    for session in &mut sessions {
        if session.summary.source == "external"
            && session.summary.state == "active"
            && now.saturating_sub(session.summary.updated_at) > 120_000
        {
            session.summary.state = "unknown";
        }
    }
    sessions.sort_by(|a, b| {
        b.summary
            .updated_at
            .cmp(&a.summary.updated_at)
            .then(a.summary.id.cmp(&b.summary.id))
    });
    Ok((sources, sessions))
}

pub(crate) async fn list(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Query(query): Query<ActivityQuery>,
) -> Result<(HeaderMap, Json<ActivityList>), ApiErrorResponse> {
    authorize(&context, &headers)?;
    if query
        .provider
        .as_deref()
        .is_some_and(|p| !["codex", "claude", "opencode", "orchester"].contains(&p))
    {
        return Err(api_error_response(
            ApiErrorCode::ValidationFailed,
            request_id_from_headers(&headers),
        ));
    }
    let (mut sources, sessions) = snapshot(&context, &headers).await?;
    sources.retain(|source| {
        query
            .provider
            .as_ref()
            .is_none_or(|provider| provider == &source.provider)
    });
    let sessions = sessions
        .into_iter()
        .map(|row| row.summary)
        .filter(|row| {
            query
                .provider
                .as_ref()
                .is_none_or(|provider| provider == &row.provider)
        })
        .collect();
    Ok((
        no_store_headers(),
        Json(ActivityList {
            schema_version: 1,
            observed_at: now_ms(),
            sources,
            sessions,
        }),
    ))
}

pub(crate) async fn detail(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ActivityDetailQuery>,
) -> Result<(HeaderMap, Json<ActivityDetail>), ApiErrorResponse> {
    authorize(&context, &headers)?;
    if query.before == Some(0) {
        return Err(api_error_response(
            ApiErrorCode::ValidationFailed,
            request_id_from_headers(&headers),
        ));
    }
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(api_error_response(
            ApiErrorCode::NotFound,
            request_id_from_headers(&headers),
        ));
    }
    let (_, sessions) = snapshot(&context, &headers).await?;
    let mut row = sessions
        .into_iter()
        .find(|row| row.summary.id == id)
        .ok_or_else(|| {
            api_error_response(ApiErrorCode::NotFound, request_id_from_headers(&headers))
        })?;
    if row.summary.provider == "codex" && row.summary.source == "external" {
        let store = context.agent_activity().clone();
        row = tokio::task::spawn_blocking(move || store.codex_detail(&id, query.before))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten()
            .ok_or_else(|| {
                api_error_response(ApiErrorCode::Unavailable, request_id_from_headers(&headers))
            })?;
    } else if query.before.is_some() {
        return Err(api_error_response(
            ApiErrorCode::ValidationFailed,
            request_id_from_headers(&headers),
        ));
    }
    if row.summary.source == "external"
        && row.summary.state == "active"
        && now_ms().saturating_sub(row.summary.updated_at) > 120_000
    {
        row.summary.state = "unknown";
    }
    Ok((no_store_headers(), Json(row)))
}

fn now_ms() -> i64 {
    (OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}
fn opaque_id(provider: &str, source: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{provider}\0{source}").as_bytes())
    )
}
fn clip(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .take(limit)
        .collect()
}

fn empty_detail(provider: &str, source_id: &str, updated_at: i64) -> ActivityDetail {
    ActivityDetail {
        schema_version: 1,
        summary: ActivitySummary {
            id: opaque_id(provider, source_id),
            provider: provider.into(),
            source: "external",
            title: String::new(),
            project: String::new(),
            project_id: None,
            pinned: None,
            updated_at,
            state: "unknown",
            preview: String::new(),
            truncated: false,
        },
        entries: Vec::new(),
        turns: Vec::new(),
        next_before: None,
        content_limited: false,
    }
}

fn push_entry(
    row: &mut ActivityDetail,
    id: &str,
    kind: &'static str,
    text: &str,
    tool: Option<&str>,
) {
    if text.trim().is_empty() && tool.is_none() {
        return;
    }
    if text.chars().count() > TEXT_LIMIT {
        row.summary.truncated = true;
        row.content_limited = true;
    }
    let text = clip(text, TEXT_LIMIT);
    let entry = ActivityEntry {
        id: opaque_id(&row.summary.id, id),
        kind,
        text,
        tool: tool.map(|value| clip(value, 120)),
        timestamp: None,
        phase: None,
        turn_id: None,
        call_id: None,
    };
    if let Some(old) = row.entries.iter_mut().find(|old| old.id == entry.id) {
        *old = entry;
    } else {
        row.entries.push(entry);
    }
    if row.entries.len() > ENTRY_LIMIT {
        row.entries.remove(0);
        row.summary.truncated = true;
    }
}

fn finish(row: &mut ActivityDetail) {
    row.turns.retain(|turn| {
        row.entries
            .iter()
            .any(|entry| entry.turn_id.as_deref() == Some(&turn.id))
    });
    if row.summary.title.is_empty() {
        row.summary.title = row
            .entries
            .iter()
            .find(|entry| entry.kind == "user")
            .map(|entry| clip(&entry.text, 120))
            .unwrap_or_default();
    }
    row.summary.preview = row
        .entries
        .iter()
        .rev()
        .find(|entry| entry.kind != "status")
        .map(|entry| clip(&entry.text, 200))
        .unwrap_or_default();
}

#[cfg(test)]
mod tests;
