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

const BASE_MIGRATION: &str = include_str!("../../migrations/0001_memory.sql");
const ACCESS_MIGRATION: &str = include_str!("../../migrations/0002_memory_access.sql");
const PROVENANCE_MIGRATION: &str = include_str!("../../migrations/0003_memory_provenance.sql");
const NAMESPACE_MIGRATION: &str = include_str!("../../migrations/0004_memory_schema_namespace.sql");
const SCHEMA_VERSION: i64 = 4;
const SCHEMA_VERSIONS: [i64; 4] = [1, 2, 3, SCHEMA_VERSION];
const MAX_CONTENT_BYTES: usize = 16 * 1024;
const MAX_ID_BYTES: usize = 256;
const MAX_SOURCE_BYTES: usize = 128;
const MAX_TIMESTAMP_BYTES: usize = 64;
const MAX_QUERY_BYTES: usize = 1024;
const MAX_RECALL_ITEMS: usize = 20;
const MAX_QUERY_TERMS: usize = 32;
const MAX_QUERY_TERM_BYTES: usize = 64;
const MAX_EVENTS: usize = 10_000;
const MAX_ITEMS_PER_PROJECT: i64 = 10_000;
const MAX_CONFIGURED_SECRET_BYTES: usize = 16 * 1024;
const MAX_CONFIGURED_SECRETS: usize = 256;

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
    pub proposed_by_actor_id: String,
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
            .field(
                "proposed_by_actor_id_bytes",
                &self.proposed_by_actor_id.len(),
            )
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

#[derive(Debug, Clone, Copy)]
enum MemoryDecision {
    Approve,
    Reject,
}

impl MemoryDecision {
    fn state(self) -> MemoryState {
        match self {
            Self::Approve => MemoryState::Accepted,
            Self::Reject => MemoryState::Rejected,
        }
    }

    fn event(self) -> MemoryEventKind {
        match self {
            Self::Approve => MemoryEventKind::Approved,
            Self::Reject => MemoryEventKind::Rejected,
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
    #[error("memory approval refers to stale content")]
    StaleApproval,
    #[error("memory project access is not authorized")]
    AuthorizationDenied,
    #[error("memory project quota is exhausted")]
    QuotaExceeded,
    #[error("memory content was forgotten but secure storage cleanup is incomplete")]
    SecureEraseIncomplete,
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
            | (Self::StaleApproval, Self::StaleApproval)
            | (Self::AuthorizationDenied, Self::AuthorizationDenied)
            | (Self::QuotaExceeded, Self::QuotaExceeded)
            | (Self::SecureEraseIncomplete, Self::SecureEraseIncomplete)
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

/// An owner-bound view of a memory database. The project and actor are
/// captured when the view is created, so individual calls cannot mix IDs from
/// different projects.
pub struct MemoryAccess<'a> {
    store: &'a MemoryStore,
    project_id: String,
    actor_id: String,
    run_id: Option<String>,
}

impl fmt::Debug for MemoryAccess<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryAccess")
            .field("project_id_bytes", &self.project_id.len())
            .field("actor_id_bytes", &self.actor_id.len())
            .field("run_id_bytes", &self.run_id.as_ref().map(String::len))
            .finish()
    }
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
        mut connection: Connection,
        configured_secrets: Vec<SecretString>,
    ) -> Result<Self, MemoryError> {
        if configured_secrets.len() > MAX_CONFIGURED_SECRETS
            || configured_secrets
                .iter()
                .any(|secret| secret.expose_secret().len() > MAX_CONFIGURED_SECRET_BYTES)
        {
            return Err(MemoryError::InvalidInput);
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             PRAGMA journal_mode = WAL;
             PRAGMA secure_delete = ON;",
        )?;
        ensure_schema(&mut connection)?;
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

    /// Bind a project to its owner once. Re-registering with the same owner is
    /// idempotent; attempting to claim an existing project is rejected.
    pub fn register_project(
        &self,
        project_id: &str,
        owner_actor_id: &str,
        created_at: &str,
    ) -> Result<(), MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        validate_identifier(owner_actor_id, MAX_ID_BYTES)?;
        if owner_actor_id.starts_with("agent:") {
            return Err(MemoryError::InvalidInput);
        }
        validate_timestamp(created_at)?;
        self.reject_secrets([project_id, owner_actor_id, created_at])?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT owner_actor_id FROM memory_projects WHERE project_id = ?1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match existing {
            Some(existing) if existing == owner_actor_id => {
                transaction.commit()?;
                Ok(())
            }
            Some(_) => Err(MemoryError::AuthorizationDenied),
            None => {
                transaction.execute(
                    "INSERT INTO memory_projects(project_id, owner_actor_id, created_at)
                     VALUES(?1, ?2, ?3)",
                    params![project_id, owner_actor_id, created_at],
                )?;
                transaction.commit()?;
                Ok(())
            }
        }
    }

    pub fn access<'a>(
        &'a self,
        project_id: &str,
        actor_id: &str,
    ) -> Result<MemoryAccess<'a>, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        validate_identifier(actor_id, MAX_ID_BYTES)?;
        if actor_id.starts_with("agent:") {
            return Err(MemoryError::AuthorizationDenied);
        }
        self.authorize_project(project_id, actor_id)?;
        Ok(MemoryAccess {
            store: self,
            project_id: project_id.to_owned(),
            actor_id: actor_id.to_owned(),
            run_id: None,
        })
    }

    fn authorize_project(&self, project_id: &str, actor_id: &str) -> Result<(), MemoryError> {
        let owner = self.project_owner(project_id)?;
        if owner == actor_id {
            Ok(())
        } else {
            Err(MemoryError::AuthorizationDenied)
        }
    }

    fn project_owner(&self, project_id: &str) -> Result<String, MemoryError> {
        let connection = self.connection()?;
        let owner = connection
            .query_row(
                "SELECT owner_actor_id FROM memory_projects WHERE project_id = ?1",
                params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        owner.ok_or(MemoryError::NotFound)
    }

    fn reject_secrets<'a, I>(&self, fields: I) -> Result<(), MemoryError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        for field in fields {
            if let Some(finding) = self.scanner.scan(field) {
                return Err(MemoryError::SecretDetected {
                    category: finding.category,
                    start: finding.start,
                    end: finding.end,
                });
            }
        }
        Ok(())
    }

    fn propose_unscoped(
        &self,
        proposal: MemoryProposal,
        proposed_by_actor_id: &str,
    ) -> Result<MemoryItem, MemoryError> {
        validate_proposal(&proposal)?;
        validate_identifier(proposed_by_actor_id, MAX_ID_BYTES)?;
        self.ensure_project_registered(&proposal.project_id)?;
        self.reject_secrets(
            [
                Some(proposal.memory_id.as_str()),
                Some(proposal.project_id.as_str()),
                Some(proposal.content.as_str()),
                Some(proposal.source.as_str()),
                proposal.source_run_id.as_deref(),
                Some(proposal.created_at.as_str()),
                Some(proposed_by_actor_id),
            ]
            .into_iter()
            .flatten(),
        )?;
        let content_hash = sha256_hex(proposal.content.as_bytes());
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if memory_exists(&transaction, &proposal.memory_id)? {
            return Err(MemoryError::AlreadyExists);
        }
        let project_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM memory_items WHERE project_id = ?1",
            params![proposal.project_id],
            |row| row.get(0),
        )?;
        if project_count >= MAX_ITEMS_PER_PROJECT {
            return Err(MemoryError::QuotaExceeded);
        }
        transaction.execute(
            "INSERT INTO memory_items(
                memory_id, project_id, state, kind, content, content_hash,
                proposed_by_actor_id, source_run_id, source, confidence, created_at
             ) VALUES(?1, ?2, 'proposed', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                proposal.memory_id,
                proposal.project_id,
                memory_kind_db(&proposal.kind),
                proposal.content,
                content_hash,
                proposed_by_actor_id,
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
            Some(proposed_by_actor_id),
            &proposal.created_at,
        )?;
        let item = load_item(&transaction, &proposal.project_id, &proposal.memory_id)?;
        transaction.commit()?;
        Ok(item)
    }

    fn ensure_project_registered(&self, project_id: &str) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM memory_projects WHERE project_id = ?1)",
            params![project_id],
            |row| row.get(0),
        )?;
        exists.then_some(()).ok_or(MemoryError::NotFound)
    }

    fn register_run_unscoped(
        &self,
        project_id: &str,
        run_id: &str,
        owner_actor_id: &str,
        created_at: &str,
    ) -> Result<(), MemoryError> {
        validate_identifier(run_id, MAX_ID_BYTES)?;
        validate_timestamp(created_at)?;
        self.authorize_project(project_id, owner_actor_id)?;
        self.reject_secrets([project_id, run_id, owner_actor_id, created_at])?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT project_id, owner_actor_id FROM memory_runs WHERE run_id = ?1",
                params![run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match existing {
            Some((existing_project, existing_owner))
                if existing_project == project_id && existing_owner == owner_actor_id =>
            {
                transaction.commit()?;
                Ok(())
            }
            Some(_) => Err(MemoryError::AuthorizationDenied),
            None => {
                transaction.execute(
                    "INSERT INTO memory_runs(run_id, project_id, owner_actor_id, created_at)
                     VALUES(?1, ?2, ?3, ?4)",
                    params![run_id, project_id, owner_actor_id, created_at],
                )?;
                transaction.commit()?;
                Ok(())
            }
        }
    }

    fn agent_access_unscoped<'a>(
        &'a self,
        project_id: &str,
        run_id: &str,
    ) -> Result<MemoryAccess<'a>, MemoryError> {
        let connection = self.connection()?;
        let mapping = connection
            .query_row(
                "SELECT project_id FROM memory_runs WHERE run_id = ?1",
                params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if mapping.as_deref() != Some(project_id) {
            return Err(MemoryError::AuthorizationDenied);
        }
        Ok(MemoryAccess {
            store: self,
            project_id: project_id.to_owned(),
            actor_id: format!("agent:{run_id}"),
            run_id: Some(run_id.to_owned()),
        })
    }

    fn approve_unscoped(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.decide(
            project_id,
            memory_id,
            actor_id,
            expected_content_hash,
            decided_at,
            MemoryDecision::Approve,
        )
    }

    fn reject_unscoped(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.decide(
            project_id,
            memory_id,
            actor_id,
            expected_content_hash,
            decided_at,
            MemoryDecision::Reject,
        )
    }

    fn decide(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
        decision: MemoryDecision,
    ) -> Result<MemoryItem, MemoryError> {
        let target = decision.state();
        let event_kind = decision.event();
        validate_transition_input(project_id, memory_id, actor_id, decided_at)?;
        validate_content_hash(expected_content_hash)?;
        self.authorize_project(project_id, actor_id)?;
        self.reject_secrets([project_id, memory_id, actor_id, decided_at])?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((state, content_hash)) = load_state_and_hash(&transaction, project_id, memory_id)?
        else {
            return Err(MemoryError::NotFound);
        };
        if state != MemoryState::Proposed {
            return Err(MemoryError::InvalidTransition);
        }
        if content_hash.as_deref() != Some(expected_content_hash) {
            return Err(MemoryError::StaleApproval);
        }
        let updated = transaction.execute(
            "UPDATE memory_items
             SET state = ?1, decided_at = ?2, decided_by_actor_id = ?3,
                 row_version = row_version + 1
             WHERE project_id = ?4 AND memory_id = ?5 AND state = 'proposed'
               AND content_hash = ?6",
            params![
                target.as_db(),
                decided_at,
                actor_id,
                project_id,
                memory_id,
                expected_content_hash
            ],
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
        let item = load_item(&transaction, project_id, memory_id)?;
        transaction.commit()?;
        Ok(item)
    }

    fn forget_unscoped(
        &self,
        project_id: &str,
        memory_id: &str,
        actor_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        validate_transition_input(project_id, memory_id, actor_id, decided_at)?;
        validate_content_hash(expected_content_hash)?;
        self.authorize_project(project_id, actor_id)?;
        self.reject_secrets([project_id, memory_id, actor_id, decided_at])?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((state, content_hash)) = load_state_and_hash(&transaction, project_id, memory_id)?
        else {
            return Err(MemoryError::NotFound);
        };
        if state == MemoryState::Forgotten {
            return Err(MemoryError::InvalidTransition);
        }
        if content_hash.as_deref() != Some(expected_content_hash) {
            return Err(MemoryError::StaleApproval);
        }
        let updated = transaction.execute(
            "UPDATE memory_items
             SET state = 'forgotten', content = '', content_hash = NULL,
                 decided_at = ?1, decided_by_actor_id = ?2,
                 row_version = row_version + 1
             WHERE project_id = ?3 AND memory_id = ?4 AND state <> 'forgotten'
               AND content_hash = ?5",
            params![
                decided_at,
                actor_id,
                project_id,
                memory_id,
                expected_content_hash
            ],
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
        let item = load_item(&transaction, project_id, memory_id)?;
        transaction.commit()?;
        secure_erase_checkpoint(&connection)?;
        Ok(item)
    }

    fn get_unscoped(&self, project_id: &str, memory_id: &str) -> Result<MemoryItem, MemoryError> {
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

    fn recall_unscoped(
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
                    i.content_hash, i.proposed_by_actor_id, i.source_run_id,
                    i.source, i.confidence,
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

    fn events_unscoped(&self, project_id: &str) -> Result<Vec<MemoryEvent>, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT event_sequence, event_id, memory_id, project_id, kind,
                    content_hash, actor_id, occurred_at
             FROM memory_events
             WHERE project_id = ?1
             ORDER BY event_sequence ASC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![project_id, (MAX_EVENTS + 1) as i64], |row| {
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
        let rows = rows.collect::<Result<Vec<_>, _>>()?;
        if rows.len() > MAX_EVENTS {
            return Err(MemoryError::QuotaExceeded);
        }
        rows.into_iter().map(MemoryEvent::try_from).collect()
    }

    fn count_project_unscoped(&self, project_id: &str) -> Result<u64, MemoryError> {
        validate_identifier(project_id, MAX_ID_BYTES)?;
        let connection = self.connection()?;
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM memory_items WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )?;
        u64::try_from(count).map_err(|_| MemoryError::Corrupt)
    }
}

impl MemoryAccess<'_> {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn agent_access(
        &self,
        source_run_id: &str,
        created_at: &str,
    ) -> Result<MemoryAccess<'_>, MemoryError> {
        if self.run_id.is_some() {
            return Err(MemoryError::AuthorizationDenied);
        }
        self.store.register_run_unscoped(
            &self.project_id,
            source_run_id,
            &self.actor_id,
            created_at,
        )?;
        self.store
            .agent_access_unscoped(&self.project_id, source_run_id)
    }

    pub fn propose(&self, proposal: MemoryProposal) -> Result<MemoryItem, MemoryError> {
        if proposal.project_id != self.project_id {
            return Err(MemoryError::AuthorizationDenied);
        }
        if self.run_id.as_deref() != proposal.source_run_id.as_deref() {
            return Err(MemoryError::AuthorizationDenied);
        }
        if self.run_id.is_none() {
            return Err(MemoryError::AuthorizationDenied);
        }
        if self.store.project_owner(&self.project_id)? == self.actor_id {
            return Err(MemoryError::AuthorizationDenied);
        }
        self.store.propose_unscoped(proposal, &self.actor_id)
    }

    pub fn approve(
        &self,
        memory_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.store.approve_unscoped(
            &self.project_id,
            memory_id,
            &self.actor_id,
            expected_content_hash,
            decided_at,
        )
    }

    pub fn reject(
        &self,
        memory_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.store.reject_unscoped(
            &self.project_id,
            memory_id,
            &self.actor_id,
            expected_content_hash,
            decided_at,
        )
    }

    pub fn forget(
        &self,
        memory_id: &str,
        expected_content_hash: &str,
        decided_at: &str,
    ) -> Result<MemoryItem, MemoryError> {
        self.store.forget_unscoped(
            &self.project_id,
            memory_id,
            &self.actor_id,
            expected_content_hash,
            decided_at,
        )
    }

    pub fn get(&self, memory_id: &str) -> Result<MemoryItem, MemoryError> {
        self.store.get_unscoped(&self.project_id, memory_id)
    }

    pub fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>, MemoryError> {
        self.store.recall_unscoped(&self.project_id, query, limit)
    }

    pub fn events(&self) -> Result<Vec<MemoryEvent>, MemoryError> {
        self.store.events_unscoped(&self.project_id)
    }

    pub fn count(&self) -> Result<u64, MemoryError> {
        self.store.count_project_unscoped(&self.project_id)
    }
}

const MEMORY_ITEM_SELECT_WITH_SCOPE: &str =
    "SELECT memory_id, project_id, state, kind, content, content_hash,
            proposed_by_actor_id, source_run_id, source, confidence,
            created_at, decided_at, decided_by_actor_id
     FROM memory_items
     WHERE project_id = ?1 AND memory_id = ?2";

struct RawMemoryItem {
    memory_id: String,
    project_id: String,
    state: String,
    kind: String,
    content: String,
    content_hash: Option<String>,
    proposed_by_actor_id: String,
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
        proposed_by_actor_id: row.get(6)?,
        source_run_id: row.get(7)?,
        source: row.get(8)?,
        confidence: row.get(9)?,
        created_at: row.get(10)?,
        decided_at: row.get(11)?,
        decided_by_actor_id: row.get(12)?,
    })
}

impl TryFrom<RawMemoryItem> for MemoryItem {
    type Error = MemoryError;

    fn try_from(raw: RawMemoryItem) -> Result<Self, Self::Error> {
        let state = MemoryState::from_db(&raw.state)?;
        let kind = memory_kind_from_db(&raw.kind)?;
        validate_stored(validate_identifier(&raw.memory_id, MAX_ID_BYTES))?;
        validate_stored(validate_identifier(&raw.project_id, MAX_ID_BYTES))?;
        validate_stored(validate_identifier(&raw.proposed_by_actor_id, MAX_ID_BYTES))?;
        validate_stored(validate_identifier(&raw.source, MAX_SOURCE_BYTES))?;
        validate_stored(validate_timestamp(&raw.created_at))?;
        if let Some(run_id) = raw.source_run_id.as_deref() {
            validate_stored(validate_identifier(run_id, MAX_ID_BYTES))?;
        }
        if let Some(decided_at) = raw.decided_at.as_deref() {
            validate_stored(validate_timestamp(decided_at))?;
        }
        if let Some(actor_id) = raw.decided_by_actor_id.as_deref() {
            validate_stored(validate_identifier(actor_id, MAX_ID_BYTES))?;
        }
        if raw.content.len() > MAX_CONTENT_BYTES
            || raw.content.chars().any(forbidden_content_character)
            || !raw.confidence.is_finite()
            || !(0.0..=1.0).contains(&raw.confidence)
        {
            return Err(MemoryError::Corrupt);
        }
        if (state == MemoryState::Forgotten)
            != (raw.content.is_empty() && raw.content_hash.is_none())
        {
            return Err(MemoryError::Corrupt);
        }
        if state != MemoryState::Forgotten
            && raw.content_hash.as_deref() != Some(sha256_hex(raw.content.as_bytes()).as_str())
        {
            return Err(MemoryError::Corrupt);
        }
        if let Some(hash) = raw.content_hash.as_deref() {
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(MemoryError::Corrupt);
            }
        }
        Ok(Self {
            memory_id: raw.memory_id,
            project_id: raw.project_id,
            state,
            kind,
            content: raw.content,
            content_hash: raw.content_hash,
            proposed_by_actor_id: raw.proposed_by_actor_id,
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
        let sequence = u64::try_from(raw.sequence).map_err(|_| MemoryError::Corrupt)?;
        validate_stored(validate_identifier(&raw.event_id, MAX_ID_BYTES))?;
        validate_stored(validate_identifier(&raw.memory_id, MAX_ID_BYTES))?;
        validate_stored(validate_identifier(&raw.project_id, MAX_ID_BYTES))?;
        validate_stored(validate_timestamp(&raw.occurred_at))?;
        if let Some(actor_id) = raw.actor_id.as_deref() {
            validate_stored(validate_identifier(actor_id, MAX_ID_BYTES))?;
        }
        if let Some(hash) = raw.content_hash.as_deref() {
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(MemoryError::Corrupt);
            }
        }
        Ok(Self {
            sequence,
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

fn load_item(
    transaction: &Transaction<'_>,
    project_id: &str,
    memory_id: &str,
) -> Result<MemoryItem, MemoryError> {
    let raw = transaction
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

fn ensure_schema(connection: &mut Connection) -> Result<(), MemoryError> {
    // Serialize schema discovery with migration. Otherwise a concurrent opener
    // can observe the legacy version table while another connection is midway
    // through namespacing it and reject a valid first-open migration.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_memory_versions = table_exists(&transaction, "memory_schema_versions")?;
    let has_legacy_versions = table_exists(&transaction, "schema_versions")?;
    if has_memory_versions && has_legacy_versions {
        return Err(MemoryError::UnsupportedSchema);
    }
    if !has_memory_versions && !has_legacy_versions {
        let user_objects: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE name NOT LIKE 'sqlite_%'
               AND type IN ('table', 'index', 'trigger', 'view')",
            [],
            |row| row.get(0),
        )?;
        if user_objects != 0 {
            return Err(MemoryError::UnsupportedSchema);
        }
        transaction.execute_batch(BASE_MIGRATION)?;
    } else if has_legacy_versions {
        let memory_marker = table_exists(&transaction, "memory_items")?
            && table_exists(&transaction, "memory_events")?
            && table_exists(&transaction, "memory_fts")?;
        if !memory_marker || table_exists(&transaction, "runs")? {
            return Err(MemoryError::UnsupportedSchema);
        }
        let item_count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM memory_items", [], |row| row.get(0))?;
        if item_count != 0 {
            // Legacy rows predate owner/run provenance. Claiming an owner
            // automatically would turn a migration into an authorization
            // bypass, so an explicit export/review/import is required.
            return Err(MemoryError::UnsupportedSchema);
        }
    }

    let mut versions = schema_versions(&transaction)?;
    if versions == [1] {
        transaction.execute_batch(ACCESS_MIGRATION)?;
        versions = schema_versions(&transaction)?;
    }
    if versions == [1, 2] {
        transaction.execute_batch(PROVENANCE_MIGRATION)?;
        versions = schema_versions(&transaction)?;
    }
    if versions == [1, 2, 3] {
        transaction.execute_batch(NAMESPACE_MIGRATION)?;
        versions = schema_versions(&transaction)?;
    }
    if versions != SCHEMA_VERSIONS {
        return Err(MemoryError::UnsupportedSchema);
    }
    transaction.commit()?;
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, MemoryError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1
             )",
            params![table],
            |row| row.get(0),
        )
        .map_err(MemoryError::from)
}

fn schema_versions(connection: &Connection) -> Result<Vec<i64>, MemoryError> {
    let table = if table_exists(connection, "memory_schema_versions")? {
        "memory_schema_versions"
    } else if table_exists(connection, "schema_versions")? {
        "schema_versions"
    } else {
        return Err(MemoryError::UnsupportedSchema);
    };
    let sql = if table == "memory_schema_versions" {
        "SELECT version FROM memory_schema_versions ORDER BY version"
    } else {
        "SELECT version FROM schema_versions ORDER BY version"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map([], |row| row.get::<_, i64>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryError::from)
}

fn random_event_id() -> Result<String, MemoryError> {
    let mut bytes = [0_u8; 16];
    fill_random(&mut bytes).map_err(|_| MemoryError::Entropy)?;
    Ok(format!("mem-{}", hex(&bytes)))
}

fn verify_schema(connection: &Connection) -> Result<(), MemoryError> {
    let versions = schema_versions(connection)?;
    if versions != SCHEMA_VERSIONS {
        return Err(MemoryError::UnsupportedSchema);
    }
    for table in [
        "memory_schema_versions",
        "memory_items",
        "memory_fts",
        "memory_events",
        "memory_projects",
        "memory_runs",
    ] {
        if !table_exists(connection, table)? {
            return Err(MemoryError::UnsupportedSchema);
        }
    }
    if schema_fingerprint(connection)? != expected_schema_fingerprint()? {
        return Err(MemoryError::UnsupportedSchema);
    }
    Ok(())
}

fn expected_schema_fingerprint() -> Result<String, MemoryError> {
    let connection = Connection::open_in_memory()?;
    connection.execute_batch(BASE_MIGRATION)?;
    connection.execute_batch(ACCESS_MIGRATION)?;
    connection.execute_batch(PROVENANCE_MIGRATION)?;
    connection.execute_batch(NAMESPACE_MIGRATION)?;
    schema_fingerprint(&connection)
}

fn schema_fingerprint(connection: &Connection) -> Result<String, MemoryError> {
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, COALESCE(sql, '')
         FROM sqlite_master
         WHERE name NOT LIKE 'sqlite_%'
         ORDER BY type ASC, name ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut hasher = Sha256::new();
    for row in rows {
        let (kind, name, table, sql) = row?;
        for field in [kind, name, table, sql] {
            hasher.update((field.len() as u64).to_be_bytes());
            hasher.update(field.as_bytes());
        }
    }
    Ok(hex(&hasher.finalize()))
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
    let orphan_projects: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         LEFT JOIN memory_projects p ON p.project_id = i.project_id
         WHERE p.project_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let orphan_events: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_events e
         LEFT JOIN memory_items i ON i.memory_id = e.memory_id
         WHERE i.memory_id IS NULL OR i.project_id <> e.project_id",
        [],
        |row| row.get(0),
    )?;
    if orphan_projects != 0 || orphan_events != 0 {
        return Err(MemoryError::Corrupt);
    }
    let invalid_event_actors: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_events e
         JOIN memory_items i ON i.memory_id = e.memory_id
         JOIN memory_projects p ON p.project_id = i.project_id
         WHERE (
           e.kind = 'proposed'
           AND (e.actor_id IS NULL OR e.actor_id <> i.proposed_by_actor_id)
         ) OR (
           e.kind <> 'proposed'
           AND (e.actor_id IS NULL OR e.actor_id <> p.owner_actor_id)
         )",
        [],
        |row| row.get(0),
    )?;
    let invalid_event_hashes: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_events e
         JOIN memory_events proposed
           ON proposed.memory_id = e.memory_id AND proposed.kind = 'proposed'
         WHERE e.content_hash IS NULL OR e.content_hash <> proposed.content_hash",
        [],
        |row| row.get(0),
    )?;
    let invalid_event_states: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         WHERE (
           SELECT COUNT(*) FROM memory_events e
           WHERE e.memory_id = i.memory_id AND e.kind = 'proposed'
         ) <> 1 OR (
           i.state = 'proposed' AND (
             SELECT COUNT(*) FROM memory_events e
             WHERE e.memory_id = i.memory_id AND e.kind <> 'proposed'
           ) <> 0
         ) OR (
           i.state = 'accepted' AND (
             (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind = 'approved') <> 1
             OR (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind IN ('rejected', 'forgotten')) <> 0
           )
         ) OR (
           i.state = 'rejected' AND (
             (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind = 'rejected') <> 1
             OR (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind IN ('approved', 'forgotten')) <> 0
           )
         ) OR (
           i.state = 'forgotten' AND (
             (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind = 'forgotten') <> 1
             OR (SELECT kind FROM memory_events e WHERE e.memory_id = i.memory_id ORDER BY event_sequence DESC LIMIT 1) <> 'forgotten'
             OR (SELECT COUNT(*) FROM memory_events e WHERE e.memory_id = i.memory_id AND e.kind IN ('approved', 'rejected')) > 1
           )
         )",
        [],
        |row| row.get(0),
    )?;
    let invalid_event_order: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         WHERE (
           SELECT kind FROM memory_events e
           WHERE e.memory_id = i.memory_id
           ORDER BY event_sequence ASC LIMIT 1
         ) <> 'proposed' OR EXISTS (
           SELECT 1 FROM memory_events e
           JOIN memory_events proposed
             ON proposed.memory_id = e.memory_id AND proposed.kind = 'proposed'
           WHERE e.memory_id = i.memory_id
             AND e.kind <> 'proposed'
             AND e.event_sequence <= proposed.event_sequence
         ) OR EXISTS (
           SELECT 1 FROM memory_events forgotten
           JOIN memory_events decision
             ON decision.memory_id = forgotten.memory_id
            AND decision.kind IN ('approved', 'rejected')
           WHERE forgotten.memory_id = i.memory_id
             AND forgotten.kind = 'forgotten'
             AND decision.event_sequence >= forgotten.event_sequence
         )",
        [],
        |row| row.get(0),
    )?;
    if invalid_event_actors != 0
        || invalid_event_hashes != 0
        || invalid_event_states != 0
        || invalid_event_order != 0
    {
        return Err(MemoryError::Corrupt);
    }
    let invalid_runs: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_runs r
         LEFT JOIN memory_projects p
           ON p.project_id = r.project_id AND p.owner_actor_id = r.owner_actor_id
         WHERE p.project_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let invalid_provenance: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         LEFT JOIN memory_runs r
           ON r.run_id = i.source_run_id AND r.project_id = i.project_id
         WHERE i.proposed_by_actor_id <> 'legacy'
           AND (r.run_id IS NULL OR i.proposed_by_actor_id <> 'agent:' || r.run_id)",
        [],
        |row| row.get(0),
    )?;
    let invalid_decisions: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         JOIN memory_projects p ON p.project_id = i.project_id
         WHERE (
           i.state IN ('accepted', 'rejected', 'forgotten')
           AND (
               i.decided_at IS NULL OR i.decided_by_actor_id IS NULL
               OR i.decided_by_actor_id <> p.owner_actor_id
           )
         ) OR (
           i.state = 'proposed'
           AND (i.decided_at IS NOT NULL OR i.decided_by_actor_id IS NOT NULL)
         )",
        [],
        |row| row.get(0),
    )?;
    let invalid_fts: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_fts f
         LEFT JOIN memory_items i
           ON i.memory_id = f.memory_id
          AND i.project_id = f.project_id
          AND i.content = f.content
          AND i.state = 'accepted'
         WHERE i.memory_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let missing_fts: i64 = connection.query_row(
        "SELECT COUNT(*) FROM memory_items i
         LEFT JOIN memory_fts f
           ON f.memory_id = i.memory_id
          AND f.project_id = i.project_id
          AND f.content = i.content
         WHERE i.state = 'accepted' AND f.memory_id IS NULL",
        [],
        |row| row.get(0),
    )?;
    let duplicate_fts: i64 = connection.query_row(
        "SELECT COUNT(*) FROM (
           SELECT memory_id, project_id, COUNT(*) AS copies
           FROM memory_fts GROUP BY memory_id, project_id HAVING copies <> 1
         )",
        [],
        |row| row.get(0),
    )?;
    if invalid_runs != 0
        || invalid_provenance != 0
        || invalid_decisions != 0
        || invalid_fts != 0
        || missing_fts != 0
        || duplicate_fts != 0
    {
        return Err(MemoryError::Corrupt);
    }
    let mut statement = connection.prepare(
        "SELECT memory_id, project_id, state, kind, content, content_hash,
                proposed_by_actor_id, source_run_id, source, confidence,
                created_at, decided_at, decided_by_actor_id
         FROM memory_items",
    )?;
    let rows = statement.query_map([], raw_memory_item)?;
    for row in rows {
        MemoryItem::try_from(row?).map(|_| ())?;
    }
    Ok(())
}

fn secure_erase_checkpoint(connection: &Connection) -> Result<(), MemoryError> {
    let (busy, _log_frames, _checkpointed): (i64, i64, i64) =
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
    if busy != 0 {
        return Err(MemoryError::SecureEraseIncomplete);
    }
    Ok(())
}

fn prepare_database_file(path: &Path) -> Result<(), MemoryError> {
    if path.as_os_str().is_empty() {
        return Err(MemoryError::InvalidInput);
    }
    reject_path_links(path)?;
    if let Some(parent) = path.parent() {
        let created = !parent.exists();
        fs::create_dir_all(parent).map_err(MemoryError::Io)?;
        if created {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                    .map_err(MemoryError::Io)?;
            }
        }
        reject_path_links(parent)?;
        let metadata = fs::metadata(parent).map_err(MemoryError::Io)?;
        if !metadata.is_dir() {
            return Err(MemoryError::InsecurePermissions);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(MemoryError::InsecurePermissions);
            }
        }
        #[cfg(windows)]
        {
            let secure = crate::harness::config::check_permissions(parent)
                .into_iter()
                .all(|finding| finding.secure);
            if !secure {
                return Err(MemoryError::InsecurePermissions);
            }
        }
    }
    if !path.exists() {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(MemoryError::Io(error)),
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(MemoryError::Io)?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(MemoryError::InsecurePermissions);
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
    #[cfg(windows)]
    {
        let secure = crate::harness::config::check_permissions(path)
            .into_iter()
            .all(|finding| finding.secure);
        if !secure {
            return Err(MemoryError::InsecurePermissions);
        }
    }
    Ok(())
}

fn reject_path_links(path: &Path) -> Result<(), MemoryError> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if is_link_or_reparse(&metadata) {
            return Err(MemoryError::InsecurePermissions);
        }
    }
    Ok(())
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
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
        || proposal.content.chars().any(forbidden_content_character)
        || !proposal.confidence.is_finite()
        || !(0.0..=1.0).contains(&proposal.confidence)
    {
        return Err(MemoryError::InvalidInput);
    }
    Ok(())
}

fn validate_stored(result: Result<(), MemoryError>) -> Result<(), MemoryError> {
    result.map_err(|_| MemoryError::Corrupt)
}

fn forbidden_content_character(character: char) -> bool {
    (character.is_control() && !matches!(character, '\n' | '\t')) || is_format_character(character)
}

fn is_format_character(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'
            | '\u{200C}'
            | '\u{200D}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2060}'
            | '\u{2061}'
            | '\u{2062}'
            | '\u{2063}'
            | '\u{2064}'
            | '\u{206A}'
            | '\u{206B}'
            | '\u{206C}'
            | '\u{206D}'
            | '\u{206E}'
            | '\u{206F}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
            | '\u{feff}'
    )
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
    validate_identifier(value, MAX_TIMESTAMP_BYTES)?;
    if !timestamp_pattern().is_match(value) {
        return Err(MemoryError::InvalidInput);
    }
    let year = parse_timestamp_part(value, 0, 4)?;
    let month = parse_timestamp_part(value, 5, 7)?;
    let day = parse_timestamp_part(value, 8, 10)?;
    let hour = parse_timestamp_part(value, 11, 13)?;
    let minute = parse_timestamp_part(value, 14, 16)?;
    let second = parse_timestamp_part(value, 17, 19)?;
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return Err(MemoryError::InvalidInput),
    };
    if day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return Err(MemoryError::InvalidInput);
    }
    Ok(())
}

fn parse_timestamp_part(value: &str, start: usize, end: usize) -> Result<u32, MemoryError> {
    value
        .get(start..end)
        .and_then(|part| part.parse::<u32>().ok())
        .ok_or(MemoryError::InvalidInput)
}

fn validate_content_hash(value: &str) -> Result<(), MemoryError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(MemoryError::InvalidInput);
    }
    Ok(())
}

fn timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$")
            .expect("static timestamp pattern")
    })
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
            if character.is_control() || is_format_character(character) {
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
