//! Workspace-scoped reading preferences, independent of execution and transcripts.
use crate::{OrchesterPaths, SessionHistory};
use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LibraryStateDto {
    pub revision: u64,
    pub projects: Vec<LibraryProjectDto>,
    pub sections: Vec<LibrarySectionDto>,
    pub sessions: Vec<LibrarySessionDto>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LibraryProjectDto {
    pub id: String,
    pub name: String,
    pub folders: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LibrarySectionDto {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LibrarySessionDto {
    pub id: String,
    pub title: Option<String>,
    pub pinned: bool,
    pub unread: bool,
    pub archived: bool,
    pub project_id: Option<String>,
    pub section_id: Option<String>,
}
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("library state is unavailable")]
    Unavailable,
    #[error("library snapshot is invalid")]
    Invalid,
    #[error("library revision has changed")]
    Conflict,
}
fn opaque(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn name(value: &str) -> bool {
    !value.trim().is_empty() && value.chars().count() <= 120 && !value.chars().any(char::is_control)
}
fn validate(
    state: &LibraryStateDto,
    history: &SessionHistory,
    previous: &LibraryStateDto,
) -> Result<(), LibraryError> {
    let invalid = || LibraryError::Invalid;
    if state.projects.len() > 100 || state.sections.len() > 100 || state.sessions.len() > 5000 {
        return Err(invalid());
    }
    let authorized = if state.sessions.is_empty() {
        HashSet::new()
    } else {
        history
            .authorized_ids()
            .map_err(|_| LibraryError::Unavailable)?
    };
    let mut projects = HashSet::new();
    let mut sections = HashSet::new();
    let mut sessions = HashSet::new();
    for project in &state.projects {
        if !opaque(&project.id)
            || !name(&project.name)
            || !projects.insert(&project.id)
            || project.folders.is_empty()
            || project.folders.len() > 50
        {
            return Err(invalid());
        }
        let mut folders = HashSet::new();
        for folder in &project.folders {
            let path = Path::new(folder);
            if folder.len() > 4096
                || !path.is_absolute()
                || (!previous
                    .projects
                    .iter()
                    .any(|p| p.id == project.id && p.folders.contains(folder))
                    && !path.is_dir())
            {
                return Err(invalid());
            }
            if !folders.insert(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())) {
                return Err(invalid());
            }
        }
    }
    for section in &state.sections {
        if !opaque(&section.id) || !name(&section.name) || !sections.insert(&section.id) {
            return Err(invalid());
        }
    }
    for session in &state.sessions {
        if session.id.len() != 34
            || !session.id.starts_with("s-")
            || !session.id[2..].bytes().all(|b| b.is_ascii_hexdigit())
            || !sessions.insert(&session.id)
            || session.title.as_deref().is_some_and(|t| !name(t))
            || session
                .project_id
                .as_ref()
                .is_some_and(|p| !projects.contains(p))
            || session
                .section_id
                .as_ref()
                .is_some_and(|s| !sections.contains(s))
            || !authorized.contains(&session.id)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn open(paths: &OrchesterPaths) -> Result<(Connection, String), LibraryError> {
    let identity = orchester_laufzeit::harness::service::WorkspaceIdentitySnapshot::for_workspace(
        paths.workspace(),
        "local-user",
    )
    .map_err(|_| LibraryError::Unavailable)?;
    let scope = identity.project_id;
    let connection = orchester_laufzeit::harness::run_store::open_private_database(
        &paths.state_root().join("library-preferences.db"),
    )
    .map_err(|_| LibraryError::Unavailable)?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| LibraryError::Unavailable)?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS library (scope TEXT PRIMARY KEY, snapshot TEXT NOT NULL)",
        )
        .map_err(|_| LibraryError::Unavailable)?;
    Ok((connection, scope))
}
fn read(connection: &Connection, scope: &str) -> Result<LibraryStateDto, LibraryError> {
    use rusqlite::OptionalExtension;
    let json: Option<String> = connection
        .query_row(
            "SELECT snapshot FROM library WHERE scope=?1",
            [scope],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| LibraryError::Unavailable)?;
    json.map(|s| serde_json::from_str(&s).map_err(|_| LibraryError::Unavailable))
        .unwrap_or_else(|| Ok(LibraryStateDto::default()))
}
pub fn library_get(paths: &OrchesterPaths) -> Result<LibraryStateDto, LibraryError> {
    let (db, scope) = open(paths)?;
    read(&db, &scope)
}
pub fn library_put(
    paths: &OrchesterPaths,
    mut state: LibraryStateDto,
) -> Result<LibraryStateDto, LibraryError> {
    let (mut db, scope) = open(paths)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| LibraryError::Unavailable)?;
    let previous = read(&tx, &scope)?;
    if previous.revision != state.revision {
        return Err(LibraryError::Conflict);
    }
    validate(&state, &SessionHistory::for_paths(paths), &previous)?;
    state.revision = state
        .revision
        .checked_add(1)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or(LibraryError::Invalid)?;
    let json = serde_json::to_string(&state).map_err(|_| LibraryError::Unavailable)?;
    tx.execute("INSERT INTO library(scope,snapshot) VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET snapshot=excluded.snapshot", [&scope, &json]).map_err(|_| LibraryError::Unavailable)?;
    tx.commit().map_err(|_| LibraryError::Unavailable)?;
    Ok(state)
}
