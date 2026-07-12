//! Project-scoped durable memory with explicit human approval.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use getrandom::fill as fill_random;
use orchester_protokoll::MemoryKind;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MIGRATION: &str = include_str!("../../migrations/0001_memory.sql");
const SCHEMA_VERSION: i64 = 1;
const MAX_CONTENT_BYTES: usize = 16 * 1024;
const MAX_ID_BYTES: usize = 256;
const MAX_SOURCE_BYTES: usize = 128;
const MAX_TIMESTAMP_BYTES: usize = 64;
const MAX_QUERY_BYTES: usize = 1024;
const MAX_RECALL_ITEMS: usize = 20;
const MAX_QUERY_TERMS: usize = 32;
const MAX_QUERY_TERM_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryState {
    Proposed,
    Accepted,
    Rejected,
    Forgotten,
}

impl MemoryState {
    fn as_db(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Forgotten => "forgotten",
        }
    }

    fn from_db(value: &str) -> Result<Self, MemoryError> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "forgotten" => Ok(Self::Forgotten),
            _ => Err(MemoryError::Corrupt),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct MemoryProposal {
    pub memory_id: String,
    pub project_id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub source_run_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub created_at: String,
}

impl fmt::Debug for MemoryProposal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryProposal")
            .field("memory_id", &self.memory_id)
            .field("project_id", &self.project_id)
            .field("kind", &self.kind)
            .field("content_bytes", &self.content.len())
            .field("source_run_id", &self.source_run_id)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct MemoryItem {
    pub memory_id: String,
    pub project_id: String,
    pub state: MemoryState,
    pub kind: MemoryKind,
    pub content: String,
    pub content_hash: Option<String>,
    pub source_run_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub created_at: String,
    pub decided_at: Option<String>,
    pub decided_by_actor_id: Option<String>,
}

impl fmt::Debug for MemoryItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryItem")
            .field("memory_id", &self.memory_id)
            .field("project_id", &self.project_id)
            .field("state", &self.state)
            .field("kind", &self.kind)
            .field("content_bytes", &self.content.len())
            .field("content_hash", &self.content_hash)
            .field("source_run_id", &self.source_run_id)
            .field("source", &self.source)
            .field("confidence", &self.confidence)
            .field("created_at", &self.created_at)
            .field("decided_at", &self.decided_at)
            .field("decided_by_actor_id", &self.decided_by_actor_id)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryEventKind {
    Proposed,
    Approved,
    Rejected,
    Forgotten,
}

impl MemoryEventKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Forgotten => "forgotten",
        }
    }

    fn from_db(value: &str) -> Result<Self, MemoryError> {
        match value {
            "proposed" => Ok(Self::Proposed),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "forgotten" => Ok(Self::Forgotten),
            _ => Err(MemoryError::Corrupt),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEvent {
    pub sequence: u64,
    pub event_id: String,
    pub memory_id: String,
    pub project_id: String,
    pub kind: MemoryEventKind,
    pub content_hash: Option<String>,
    pub actor_id: Option<String>,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretCategory {
    ConfiguredCredential,
    PrivateKey,
    AuthorizationHeader,
    ProviderToken,
    HighEntropyToken,
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("memory input is invalid")]
    InvalidInput,
    #[error("memory item was not found")]
    NotFound,
    #[error("memory item already exists")]
    AlreadyExists,
    #[error("memory state transition is invalid")]
    InvalidTransition,
    #[error("memory candidate contains a secret-like value")]
    SecretDetected {
        category: SecretCategory,
        start: usize,
        end: usize,
    },
    #[error("memory database schema is unsupported")]
    UnsupportedSchema,
    #[error("memory database is corrupt")]
    Corrupt,
    #[error("memory database permissions are not user-only")]
    InsecurePermissions,
    #[error("memory database lock is poisoned")]
    LockPoisoned,
    #[error("memory database operation failed")]
    Database(#[source] rusqlite::Error),
    #[error("memory database filesystem operation failed")]
    Io(#[source] std::io::Error),
    #[error("secure event identifier generation failed")]
    Entropy,
}

impl PartialEq for MemoryError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidInput, Self::InvalidInput)
            | (Self::NotFound, Self::NotFound)
            | (Self::AlreadyExists, Self::AlreadyExists)
            | (Self::InvalidTransition, Self::InvalidTransition)
            | (Self::UnsupportedSchema, Self::UnsupportedSchema)
            | (Self::Corrupt, Self::Corrupt)
            | (Self::InsecurePermissions, Self::InsecurePermissions)
            | (Self::LockPoisoned, Self::LockPoisoned)
            | (Self::Entropy, Self::Entropy) => true,
            (
                Self::SecretDetected {
                    category: left_category,
                    start: left_start,
                    end: left_end,
                },
                Self::SecretDetected {
                    category: right_category,
                    start: right_start,
                    end: right_end,
                },
            ) => {
                left_category == right_category
                    && left_start == right_start
                    && left_end == right_end
            }
            _ => false,
        }
    }
}

impl Eq for MemoryError {}

impl From<rusqlite::Error> for MemoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

pub struct MemoryStore {
    connection: Mutex<Connection>,
    scanner: SecretScanner,
    max_recall_items: usize,
}

impl fmt::Debug for MemoryStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryStore")
            .field("configured_secret_count", &self.scanner.configured.len())
            .field("max_recall_items", &self.max_recall_items)
            .finish_non_exhaustive()
    }
}

impl MemoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MemoryError> {
        Self::open_with_secrets(path, Vec::new())
    }

    pub fn open_with_secrets(
        path: impl AsRef<Path>,
        configured_secrets: Vec<SecretString>,
    ) -> Result<Self, MemoryError> {
        prepare_database_file(path.as_ref())?;
        let connection = Connection::open(path)?;
        Self::initialize(connection, configured_secrets)
    }

    pub fn in_memory() -> Result<Self, MemoryError> {
        Self::initialize(Connection::open_in_memory()?, Vec::new())
    }

    pub fn in_memory_with_secrets(secrets: Vec<SecretString>) -> Result<Self, MemoryError> {
        Self::initialize(Connection::open_in_memory()?, secrets)
    }

    fn initialize(
        connection: Connection,
        configured_secrets: Vec<SecretString>,
    ) -> Result<Self, MemoryError> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL; PRAGMA journal_mode = WAL;",
        )?;
        connection.execute_batch(MIGRATION)?;
        verify_schema(&connection)?;
        verify_integrity(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            scanner: SecretScanner::new(configured_secrets),
            max_recall_items: MAX_RECALL_ITEMS,
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, MemoryError> {
        self.connection
            .lock()
            .map_err(|_| MemoryError::LockPoisoned)
    }

    pub fn propose(&self, proposal: MemoryProposal) -> Result<MemoryItem, MemoryError> {
        validate_proposal(&proposal)?;
        if let Some(finding) = self.scanner.scan(&proposal.content) {
            return Err(MemoryError::SecretDetected {
                category: finding.category,
                start: finding.start,
                end: finding.end,
            });
        }
        let content_hash = sha256_hex(proposal.content.as_bytes());
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if memory_exists(&transaction, &proposal.memory_id)? {
            return Err(MemoryError::AlreadyExists);
        }
        transaction.execute(
            "INSERT INTO memory_items(
                memory_id, project_id, state, kind, content, content_hash,
                source_run_id, source, confidence, created_at
             ) VALUES(?1, ?2, 'proposed', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                proposal.memory_id,
                proposal.project_id,
                memory_kind_db(&proposal.kind),
                proposal.content,
                content_hash,
                proposal.source_run_id,
                proposal.source,
                proposal.confidence,
                proposal.created_at,
            ],
        )?;
        insert_event(
            &transaction,
            &proposal.project_id,
            &proposal.memory_id,
            MemoryEventKind::Proposed,
            Some(&content_hash),
            None,
            &proposal.created_at,
        )?;
        transaction.commit()?;
        drop(connection);
        self.get(&proposal.project_id, &proposal.memory_id)
    }

    pub fn approve(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.decide(
            project_id,
            memory_id,
            actor_id,
            decided_at,
            MemoryState::Accepted,
            MemoryEventKind::Approved,
        )
    }

    pub fn reject(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.decide(
            project_id,
            memory_id,
            actor_id,
            decided_at,
            MemoryState::Rejected,
            MemoryEventKind::Rejected,
        )
    }

    fn decide(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        decided_at: &str,
        target: MemoryState,
        event_kind: MemoryEventKind,
    ) -> Result<MemoryItem, MemoryError> {
        validate_transition_input(project_id, memory_id, actor_id, decided_at)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((state, content_hash)) = load_state_and_hash(&transaction, project_id, memory_id)?
        else {
            return Err(MemoryError::NotFound);
        };
        if state != MemoryState::Proposed {
            return Err(MemoryError::InvalidTransition);
        }
        let updated = transaction.execute(
            "UPDATE memory_items
             SET state = ?1, decided_at = ?2, decided_by_actor_id = ?3,
                 row_version = row_version + 1
             WHERE project_id = ?4 AND memory_id = ?5 AND state = 'proposed'",
            params![target.as_db(), decided_at, actor_id, project_id, memory_id],
        )?;
        if updated != 1 {
            return Err(MemoryError::InvalidTransition);
        }
        insert_event(
            &transaction,
            project_id,
            memory_id,
            event_kind,
            content_hash.as_deref(),
            Some(actor_id),
            decided_at,
        )?;
        transaction.commit()?;
        drop(connection);
        self.get(project_id, memory_id)
    }

    pub fn forget(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        validate_transition_input(project_id, memory_id, actor_id, decided_at)?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((state, content_hash)) = load_state_and_hash(&transaction, project_id, memory_id)?
        else {
            return Err(MemoryError::NotFound);
        };
        if state == MemoryState::Forgotten {
            return Err(MemoryError::InvalidTransition);
        }
        let updated = transaction.execute(
            "UPDATE memory_items
             SET state = 'forgotten', content = '', content_hash = NULL,
                 decided_at = ?1, decided_by_actor_id = ?2,
                 row_version = row_version + 1
             WHERE project_id = ?3 AND memory_id = ?4 AND state <> 'forgotten'",
            params![decided_at, actor_id, project_id, memory_id],
        )?;
        if updated != 1 {
            return Err(MemoryError::InvalidTransition);
        }
        insert_event(
            &transaction,
            project_id,
            memory_id,
            MemoryEventKind::Forgotten,
            content_hash.as_deref(),
            Some(actor_id),
            decided_at,
        )?;
        transaction.commit()?;
        drop(connection);
        self.get(project_id, memory_id)
    }

    pub fn get(&self, project_id: &str, memory_id: &str) -> Result<MemoryItem, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        validate_identifier(memory_id, MAX_ID_BYTES)?;
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                MEMORY_ITEM_SELECT_WITH_SCOPE,
                params![project_id, memory_id],
                raw_memory_item,
            )
            .optional()?;
        raw.map(MemoryItem::try_from)
            .transpose()?
            .ok_or(MemoryError::NotFound)
    }

    pub fn recall(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryItem>, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        if query.len() > MAX_QUERY_BYTES || query.chars().any(char::is_control) {
            return Err(MemoryError::InvalidInput);
        }
        let Some(query) = literal_fts_query(query) else {
            return Ok(Vec::new());
        };
        let limit = limit.min(self.max_recall_items);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT i.memory_id, i.project_id, i.state, i.kind, i.content,
                    i.content_hash, i.source_run_id, i.source, i.confidence,
                    i.created_at, i.decided_at, i.decided_by_actor_id
             FROM memory_fts
             JOIN memory_items i ON i.memory_id = memory_fts.memory_id
             WHERE memory_fts MATCH ?1
               AND memory_fts.project_id = ?2
               AND i.project_id = ?2
               AND i.state = 'accepted'
             ORDER BY bm25(memory_fts) ASC, i.created_at DESC, i.memory_id ASC
             LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![query, project_id, limit as i64], raw_memory_item)?;
        let raw = rows.collect::<Result<Vec<_>, _>>()?;
        raw.into_iter().map(MemoryItem::try_from).collect()
    }

    pub fn events(&self, project_id: &str) -> Result<Vec<MemoryEvent>, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT event_sequence, event_id, memory_id, project_id, kind,
                    content_hash, actor_id, occurred_at
             FROM memory_events
             WHERE project_id = ?1
             ORDER BY event_sequence ASC",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(RawMemoryEvent {
                sequence: row.get(0)?,
                event_id: row.get(1)?,
                memory_id: row.get(2)?,
                project_id: row.get(3)?,
                kind: row.get(4)?,
                content_hash: row.get(5)?,
                actor_id: row.get(6)?,
                occurred_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(MemoryEvent::try_from)
            .collect()
    }

    pub fn count_all(&self) -> Result<u64, MemoryError> {
        let connection = self.connection()?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM memory_items", [], |row| row.get(0))?;
        u64::try_from(count).map_err(|_| MemoryError::Corrupt)
    }
}

const MEMORY_ITEM_SELECT_WITH_SCOPE: &str =
    "SELECT memory_id, project_id, state, kind, content, content_hash,
            source_run_id, source, confidence, created_at, decided_at,
            decided_by_actor_id
     FROM memory_items
     WHERE project_id = ?1 AND memory_id = ?2";

struct RawMemoryItem {
    memory_id: String,
    project_id: String,
    state: String,
    kind: String,
    content: String,
    content_hash: Option<String>,
    source_run_id: Option<String>,
    source: String,
    confidence: f64,
    created_at: String,
    decided_at: Option<String>,
    decided_by_actor_id: Option<String>,
}

fn raw_memory_item(row: &Row<'_>) -> rusqlite::Result<RawMemoryItem> {
    Ok(RawMemoryItem {
        memory_id: row.get(0)?,
        project_id: row.get(1)?,
        state: row.get(2)?,
        kind: row.get(3)?,
        content: row.get(4)?,
        content_hash: row.get(5)?,
        source_run_id: row.get(6)?,
        source: row.get(7)?,
        confidence: row.get(8)?,
        created_at: row.get(9)?,
        decided_at: row.get(10)?,
        decided_by_actor_id: row.get(11)?,
    })
}

impl TryFrom<RawMemoryItem> for MemoryItem {
    type Error = MemoryError;

    fn try_from(raw: RawMemoryItem) -> Result<Self, Self::Error> {
        let state = MemoryState::from_db(&raw.state)?;
        let kind = memory_kind_from_db(&raw.kind)?;
        if (state == MemoryState::Forgotten)
            != (raw.content.is_empty() && raw.content_hash.is_none())
        {
            return Err(MemoryError::Corrupt);
        }
        Ok(Self {
            memory_id: raw.memory_id,
            project_id: raw.project_id,
            state,
            kind,
            content: raw.content,
            content_hash: raw.content_hash,
            source_run_id: raw.source_run_id,
            source: raw.source,
            confidence: raw.confidence,
            created_at: raw.created_at,
            decided_at: raw.decided_at,
            decided_by_actor_id: raw.decided_by_actor_id,
        })
    }
}

struct RawMemoryEvent {
    sequence: i64,
    event_id: String,
    memory_id: String,
    project_id: String,
    kind: String,
    content_hash: Option<String>,
    actor_id: Option<String>,
    occurred_at: String,
}

impl TryFrom<RawMemoryEvent> for MemoryEvent {
    type Error = MemoryError;

    fn try_from(raw: RawMemoryEvent) -> Result<Self, Self::Error> {
        Ok(Self {
            sequence: u64::try_from(raw.sequence).map_err(|_| MemoryError::Corrupt)?,
            event_id: raw.event_id,
            memory_id: raw.memory_id,
            project_id: raw.project_id,
            kind: MemoryEventKind::from_db(&raw.kind)?,
            content_hash: raw.content_hash,
            actor_id: raw.actor_id,
            occurred_at: raw.occurred_at,
        })
    }
}

fn memory_exists(transaction: &Transaction<'_>, memory_id: &str) -> Result<bool, MemoryError> {
    transaction
        .query_row(
            "SELECT 1 FROM memory_items WHERE memory_id = ?1",
            params![memory_id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(MemoryError::from)
}

fn load_state_and_hash(
    transaction: &Transaction<'_>,
    project_id: &str,
    memory_id: &str,
) -> Result<Option<(MemoryState, Option<String>)>, MemoryError> {
    let raw = transaction
        .query_row(
            "SELECT state, content_hash
             FROM memory_items
             WHERE project_id = ?1 AND memory_id = ?2",
            params![project_id, memory_id],
            |row| Ok((row.get::<_, String>(0)?, row.get(1)?)),
        )
        .optional()?;
    raw.map(|(state, hash)| Ok((MemoryState::from_db(&state)?, hash)))
        .transpose()
}

fn insert_event(
    transaction: &Transaction<'_>,
    project_id: &str,
    memory_id: &str,
    kind: MemoryEventKind,
    content_hash: Option<&str>,
    actor_id: Option<&str>,
    occurred_at: &str,
) -> Result<(), MemoryError> {
    let event_id = random_event_id()?;
    transaction.execute(
        "INSERT INTO memory_events(
            event_id, memory_id, project_id, kind, content_hash, actor_id, occurred_at
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            event_id,
            memory_id,
            project_id,
            kind.as_db(),
            content_hash,
            actor_id,
            occurred_at,
        ],
    )?;
    Ok(())
}

fn random_event_id() -> Result<String, MemoryError> {
    let mut bytes = [0_u8; 16];
    fill_random(&mut bytes).map_err(|_| MemoryError::Entropy)?;
    Ok(format!("mem-{}", hex(&bytes)))
}

fn verify_schema(connection: &Connection) -> Result<(), MemoryError> {
    let mut statement =
        connection.prepare("SELECT version FROM schema_versions ORDER BY version")?;
    let versions = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if versions != [SCHEMA_VERSION] {
        return Err(MemoryError::UnsupportedSchema);
    }
    Ok(())
}

fn verify_integrity(connection: &Connection) -> Result<(), MemoryError> {
    let result: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(MemoryError::Corrupt);
    }
    let violations: i64 =
        connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if violations != 0 {
        return Err(MemoryError::Corrupt);
    }
    Ok(())
}

fn prepare_database_file(path: &Path) -> Result<(), MemoryError> {
    if path.as_os_str().is_empty() {
        return Err(MemoryError::InvalidInput);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(MemoryError::Io)?;
    }
    if !path.exists() {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path).map_err(MemoryError::Io)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(MemoryError::Io)?
            .permissions()
            .mode()
            & 0o777;
        if mode & 0o077 != 0 {
            return Err(MemoryError::InsecurePermissions);
        }
    }
    Ok(())
}

fn validate_proposal(proposal: &MemoryProposal) -> Result<(), MemoryError> {
    validate_identifier(&proposal.memory_id, MAX_ID_BYTES)?;
    validate_identifier(&proposal.project_id, MAX_ID_BYTES)?;
    validate_identifier(&proposal.source, MAX_SOURCE_BYTES)?;
    validate_timestamp(&proposal.created_at)?;
    if let Some(run_id) = proposal.source_run_id.as_deref() {
        validate_identifier(run_id, MAX_ID_BYTES)?;
    }
    if proposal.content.is_empty()
        || proposal.content.len() > MAX_CONTENT_BYTES
        || proposal.content.contains('\0')
        || !proposal.confidence.is_finite()
        || !(0.0..=1.0).contains(&proposal.confidence)
    {
        return Err(MemoryError::InvalidInput);
    }
    Ok(())
}

fn validate_transition_input(
    project_id: &str,
    memory_id: &str,
    actor_id: &str,
    decided_at: &str,
) -> Result<(), MemoryError> {
    validate_identifier(project_id, MAX_ID_BYTES)?;
    validate_identifier(memory_id, MAX_ID_BYTES)?;
    validate_identifier(actor_id, MAX_ID_BYTES)?;
    validate_timestamp(decided_at)
}

fn validate_identifier(value: &str, max_bytes: usize) -> Result<(), MemoryError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(MemoryError::InvalidInput);
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), MemoryError> {
    validate_identifier(value, MAX_TIMESTAMP_BYTES)
}

fn memory_kind_db(kind: &MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Convention => "convention",
        MemoryKind::ArchitectureDecision => "architecture_decision",
        MemoryKind::Lesson => "lesson",
    }
}

fn memory_kind_from_db(value: &str) -> Result<MemoryKind, MemoryError> {
    match value {
        "convention" => Ok(MemoryKind::Convention),
        "architecture_decision" => Ok(MemoryKind::ArchitectureDecision),
        "lesson" => Ok(MemoryKind::Lesson),
        _ => Err(MemoryError::Corrupt),
    }
}

fn literal_fts_query(query: &str) -> Option<String> {
    let terms = fts_term_pattern()
        .find_iter(query)
        .filter_map(|matched| {
            let term = matched.as_str();
            (term.len() <= MAX_QUERY_TERM_BYTES).then_some(format!("\"{term}\""))
        })
        .take(MAX_QUERY_TERMS)
        .collect::<Vec<_>>();
    (!terms.is_empty()).then(|| terms.join(" AND "))
}

fn fts_term_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"[\p{L}\p{N}_]+").expect("static FTS term pattern"))
}

#[derive(Debug, Clone, Copy)]
struct SecretFinding {
    category: SecretCategory,
    start: usize,
    end: usize,
}

struct SecretScanner {
    configured: Vec<SecretString>,
}

impl SecretScanner {
    fn new(configured: Vec<SecretString>) -> Self {
        Self { configured }
    }

    fn scan(&self, input: &str) -> Option<SecretFinding> {
        let normalized = NormalizedText::without_controls(input);
        for secret in &self.configured {
            let secret = secret.expose_secret();
            if !secret.is_empty() {
                if let Some(start) = normalized.text.find(secret) {
                    return Some(normalized.finding(
                        SecretCategory::ConfiguredCredential,
                        start,
                        start + secret.len(),
                    ));
                }
            }
        }
        for (pattern, category) in [
            (private_key_pattern(), SecretCategory::PrivateKey),
            (authorization_pattern(), SecretCategory::AuthorizationHeader),
            (provider_token_pattern(), SecretCategory::ProviderToken),
        ] {
            if let Some(matched) = pattern.find(&normalized.text) {
                return Some(normalized.finding(category, matched.start(), matched.end()));
            }
        }
        high_entropy_pattern()
            .find_iter(&normalized.text)
            .find(|candidate| high_entropy(candidate.as_str()))
            .map(|candidate| {
                normalized.finding(
                    SecretCategory::HighEntropyToken,
                    candidate.start(),
                    candidate.end(),
                )
            })
    }
}

struct NormalizedText {
    text: String,
    original_byte: Vec<usize>,
    original_len: usize,
}

impl NormalizedText {
    fn without_controls(input: &str) -> Self {
        let mut text = String::with_capacity(input.len());
        let mut original_byte = Vec::with_capacity(input.len());
        for (start, character) in input.char_indices() {
            if character.is_control() {
                continue;
            }
            text.push(character);
            original_byte.extend((0..character.len_utf8()).map(|offset| start + offset));
        }
        Self {
            text,
            original_byte,
            original_len: input.len(),
        }
    }

    fn finding(&self, category: SecretCategory, start: usize, end: usize) -> SecretFinding {
        let original_start = self
            .original_byte
            .get(start)
            .copied()
            .unwrap_or(self.original_len);
        let original_end = end
            .checked_sub(1)
            .and_then(|index| self.original_byte.get(index).copied())
            .map(|index| index.saturating_add(1))
            .unwrap_or(original_start);
        SecretFinding {
            category,
            start: original_start,
            end: original_end,
        }
    }
}

fn private_key_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)-----BEGIN [^-\r\n]*PRIVATE KEY-----").expect("static private-key pattern")
    })
}

fn authorization_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)authorization\s*:\s*(?:bearer|basic)\s+\S+")
            .expect("static authorization pattern")
    })
}

fn provider_token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"(?i)(?:sk[-_][A-Za-z0-9._-]{8,}|ghp_[A-Za-z0-9]{8,}|github_pat_[A-Za-z0-9_]{8,}|xox[baprs]-[A-Za-z0-9-]{8,}|AKIA[A-Z0-9]{12,})",
        )
        .expect("static provider-token pattern")
    })
}

fn high_entropy_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9+/=_-]{24,}").expect("static high-entropy candidate pattern")
    })
}

fn high_entropy(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() < 24 {
        return false;
    }
    let classes = [
        bytes.iter().any(u8::is_ascii_lowercase),
        bytes.iter().any(u8::is_ascii_uppercase),
        bytes.iter().any(u8::is_ascii_digit),
        bytes
            .iter()
            .any(|byte| matches!(byte, b'+' | b'/' | b'=' | b'_' | b'-')),
    ]
    .into_iter()
    .filter(|present| *present)
    .count();
    if classes < 3 {
        return false;
    }
    let mut counts = BTreeMap::<u8, usize>::new();
    for byte in bytes {
        *counts.entry(*byte).or_default() += 1;
    }
    let length = bytes.len() as f64;
    let entropy = counts.values().fold(0.0, |total, count| {
        let probability = *count as f64 / length;
        total - probability * probability.log2()
    });
    entropy >= 4.0
}

fn sha256_hex(input: &[u8]) -> String {
    hex(&Sha256::digest(input))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
