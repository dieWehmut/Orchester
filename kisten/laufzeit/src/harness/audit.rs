//! Append-only, redacted hash-chain audit sink.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fs2::FileExt;
use sha2::{Digest, Sha256};
use thiserror::Error;

mod record;

use record::{
    hash_current, validate_action_projection, AuditEntryV2, AuditRecord, CURRENT_SCHEMA_VERSION,
};

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ID_CHARS: usize = 512;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit path is unavailable")]
    Io(#[source] io::Error),
    #[error("audit path does not have user-only permissions")]
    InsecurePermissions,
    #[error("audit record is invalid")]
    InvalidRecord,
    #[error("audit chain is corrupt")]
    Corrupt,
    #[error("audit sink lock is poisoned")]
    LockPoisoned,
    #[error("audit sequence is not contiguous")]
    Sequence,
    #[error("audit event already exists with different content")]
    EventConflict,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuditInput {
    pub event_id: String,
    pub occurred_at: String,
    pub actor: String,
    pub run_id: String,
    pub action_id: Option<String>,
    pub action_summary: Option<String>,
    pub action_hash: Option<String>,
    pub approval_id: Option<String>,
    pub policy_rule: String,
    pub decision: String,
    pub result_summary: String,
}

impl std::fmt::Debug for AuditInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuditInput")
            .field("event_id", &"<redacted>")
            .field("run_id", &"<redacted>")
            .field("action_present", &self.action_id.is_some())
            .field("approval_present", &self.approval_id.is_some())
            .field("result_bytes", &self.result_summary.len())
            .finish_non_exhaustive()
    }
}

/// Durable result of an append+fsync operation.  Fields are private so a
/// caller cannot forge a checkpoint accepted by the execution barrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReceipt {
    event_id: String,
    audit_file: PathBuf,
    sequence: u64,
    head_hash: String,
    synced_at: String,
    action_id: Option<String>,
    action_summary: Option<String>,
    action_hash: Option<String>,
}

impl AuditReceipt {
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn audit_file(&self) -> &Path {
        &self.audit_file
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn head_hash(&self) -> &str {
        &self.head_hash
    }

    pub fn synced_at(&self) -> &str {
        &self.synced_at
    }

    pub(crate) fn action_id(&self) -> Option<&str> {
        self.action_id.as_deref()
    }

    pub(crate) fn action_summary(&self) -> Option<&str> {
        self.action_summary.as_deref()
    }

    pub(crate) fn action_hash(&self) -> Option<&str> {
        self.action_hash.as_deref()
    }
}

pub trait AuditSink: Send + Sync {
    fn append_and_sync(&self, input: AuditInput) -> Result<AuditReceipt, AuditError>;
}

impl AuditInput {
    /// Deterministic fixture constructor used by offline tests.
    pub fn test(sequence: u64, run_id: &str, action_id: &str, summary: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(action_id.as_bytes());
        Self {
            event_id: format!("event-{sequence}"),
            occurred_at: format!("2026-07-12T00:00:{sequence:02}Z"),
            actor: "local-user".into(),
            run_id: run_id.into(),
            action_id: Some(action_id.into()),
            action_summary: Some(format!("test_action summary_bytes={}", summary.len())),
            action_hash: Some(hex_hash(&hasher.finalize().into())),
            approval_id: None,
            policy_rule: "test.rule".into(),
            decision: "allow".into(),
            result_summary: summary.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditVerification {
    pub entries: u64,
    pub head_hash: Option<[u8; 32]>,
}

impl AuditVerification {
    pub fn is_valid(self) -> bool {
        self.entries > 0 || self.head_hash.is_none()
    }
}

pub struct JsonlAuditSink {
    path: PathBuf,
    file: Mutex<File>,
    next_sequence: Mutex<u64>,
    head_hash: Mutex<String>,
}

impl JsonlAuditSink {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AuditError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let parent_existed = parent.exists();
            fs::create_dir_all(parent).map_err(AuditError::Io)?;
            ensure_private_dir(parent, !parent_existed)?;
        }
        let file_existed = path.exists();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .map_err(AuditError::Io)?;
        ensure_private_file(&path, !file_existed)?;
        let (next_sequence, head_hash) = with_exclusive_file(&mut file, scan_file)?;
        let path = fs::canonicalize(path).map_err(AuditError::Io)?;
        Ok(Self {
            path,
            file: Mutex::new(file),
            next_sequence: Mutex::new(next_sequence),
            head_hash: Mutex::new(head_hash),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, input: AuditInput) -> Result<u64, AuditError> {
        Ok(self.append_and_sync(input)?.sequence())
    }

    pub fn append_and_sync(&self, input: AuditInput) -> Result<AuditReceipt, AuditError> {
        validate_audit_input(&input)?;
        let mut file = self.file.lock().map_err(|_| AuditError::LockPoisoned)?;
        let mut sequence = self
            .next_sequence
            .lock()
            .map_err(|_| AuditError::LockPoisoned)?;
        let mut previous = self
            .head_hash
            .lock()
            .map_err(|_| AuditError::LockPoisoned)?;
        with_exclusive_file(&mut file, |file| {
            // Re-read while holding the OS lock. Multiple processes/sink
            // instances can now safely converge on the durable head.
            let (durable_next, durable_head) = scan_file(file)?;
            *sequence = durable_next;
            *previous = durable_head;
            if let Some(existing) = find_event(file, &input.event_id)? {
                let AuditRecord::V2(existing) = existing else {
                    return Err(AuditError::EventConflict);
                };
                if !audit_entry_matches(&existing, &input) {
                    return Err(AuditError::EventConflict);
                }
                // A previous process may have crashed after writing the line
                // but before returning its receipt. Re-sync before treating
                // the existing entry as the durable side of reconciliation.
                file.sync_data().map_err(AuditError::Io)?;
                return Ok(AuditReceipt {
                    event_id: existing.event_id,
                    audit_file: self.path.clone(),
                    sequence: existing.sequence,
                    head_hash: existing.entry_hash,
                    synced_at: existing.occurred_at,
                    action_id: existing.action_id,
                    action_summary: existing.action_summary,
                    action_hash: existing.action_hash,
                });
            }
            let current = *sequence;
            let entry = AuditEntryV2 {
                schema_version: CURRENT_SCHEMA_VERSION,
                sequence: current,
                occurred_at: bounded_text(&input.occurred_at),
                event_id: bounded_id(&input.event_id),
                actor: bounded_id(&input.actor),
                run_id: bounded_id(&input.run_id),
                action_id: input.action_id.as_deref().map(bounded_id),
                action_summary: input.action_summary.clone(),
                action_hash: input.action_hash.clone(),
                approval_id: input.approval_id.as_deref().map(bounded_id),
                policy_rule: bounded_id(&input.policy_rule),
                decision: bounded_id(&input.decision),
                // Do not persist model/tool output. The byte count is useful
                // for diagnostics and cannot disclose a future token format.
                result_summary: format!("summary_bytes={}", input.result_summary.len()),
                prev_hash: previous.clone(),
                entry_hash: String::new(),
            };
            if entry.event_id.is_empty() || entry.run_id.is_empty() {
                return Err(AuditError::InvalidRecord);
            }
            let hash = hash_current(&entry)?;
            let mut committed = entry;
            committed.entry_hash = hex_hash(&hash);
            let line = serde_json::to_vec(&committed).map_err(|_| AuditError::InvalidRecord)?;
            file.write_all(&line).map_err(AuditError::Io)?;
            file.write_all(b"\n").map_err(AuditError::Io)?;
            file.sync_data().map_err(AuditError::Io)?;
            *previous = committed.entry_hash.clone();
            *sequence = current.checked_add(1).ok_or(AuditError::Sequence)?;
            Ok(AuditReceipt {
                event_id: committed.event_id,
                audit_file: self.path.clone(),
                sequence: current,
                head_hash: committed.entry_hash,
                synced_at: committed.occurred_at,
                action_id: committed.action_id,
                action_summary: committed.action_summary,
                action_hash: committed.action_hash,
            })
        })
    }

    pub fn verify(&self) -> Result<AuditVerification, AuditError> {
        let mut file = self.file.lock().map_err(|_| AuditError::LockPoisoned)?;
        with_exclusive_file(&mut file, |file| {
            let (next, head) = scan_file(file)?;
            Ok(AuditVerification {
                entries: next.saturating_sub(1),
                head_hash: if head == GENESIS {
                    None
                } else {
                    Some(decode_hash(&head)?)
                },
            })
        })
    }
}

impl AuditSink for JsonlAuditSink {
    fn append_and_sync(&self, input: AuditInput) -> Result<AuditReceipt, AuditError> {
        JsonlAuditSink::append_and_sync(self, input)
    }
}

fn scan_file(file: &mut File) -> Result<(u64, String), AuditError> {
    file.seek(SeekFrom::Start(0)).map_err(AuditError::Io)?;
    let result = scan_reader(BufReader::new(&mut *file));
    file.seek(SeekFrom::End(0)).map_err(AuditError::Io)?;
    result
}

fn with_exclusive_file<T>(
    file: &mut File,
    operation: impl FnOnce(&mut File) -> Result<T, AuditError>,
) -> Result<T, AuditError> {
    file.lock_exclusive().map_err(AuditError::Io)?;
    let result = operation(file);
    let unlock = FileExt::unlock(file).map_err(AuditError::Io);
    match (result, unlock) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn scan_reader<R: BufRead>(reader: R) -> Result<(u64, String), AuditError> {
    let mut expected_sequence = 1u64;
    let mut previous = GENESIS.to_owned();
    let mut event_ids = HashSet::new();
    let mut saw_current = false;
    for line in reader.lines() {
        let line = line.map_err(AuditError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        let entry = AuditRecord::parse(&line)?;
        entry.validate_semantics()?;
        if (!entry.is_current() && saw_current)
            || entry.sequence() != expected_sequence
            || entry.previous_hash() != previous
            || !is_lower_hex_hash(entry.entry_hash())
        {
            return Err(AuditError::Corrupt);
        }
        if !event_ids.insert(entry.event_id().to_owned()) {
            return Err(AuditError::Corrupt);
        }
        let expected_hash = hex_hash(&entry.hash()?);
        if expected_hash != entry.entry_hash() {
            return Err(AuditError::Corrupt);
        }
        saw_current |= entry.is_current();
        previous = entry.entry_hash().to_owned();
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(AuditError::Sequence)?;
    }
    Ok((expected_sequence, previous))
}

fn find_event(file: &mut File, event_id: &str) -> Result<Option<AuditRecord>, AuditError> {
    file.seek(SeekFrom::Start(0)).map_err(AuditError::Io)?;
    let target = bounded_id(event_id);
    let mut found = None;
    for line in BufReader::new(&mut *file).lines() {
        let line = line.map_err(AuditError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        let entry = AuditRecord::parse(&line)?;
        if entry.event_id() == target {
            found = Some(entry);
            break;
        }
    }
    file.seek(SeekFrom::End(0)).map_err(AuditError::Io)?;
    Ok(found)
}

fn audit_entry_matches(entry: &AuditEntryV2, input: &AuditInput) -> bool {
    entry.event_id == bounded_id(&input.event_id)
        && entry.actor == bounded_id(&input.actor)
        && entry.run_id == bounded_id(&input.run_id)
        && entry.action_id == input.action_id.as_deref().map(bounded_id)
        && entry.action_summary == input.action_summary
        && entry.action_hash == input.action_hash
        && entry.approval_id == input.approval_id.as_deref().map(bounded_id)
        && entry.policy_rule == bounded_id(&input.policy_rule)
        && entry.decision == bounded_id(&input.decision)
        && entry.occurred_at == bounded_text(&input.occurred_at)
        && entry.result_summary == format!("summary_bytes={}", input.result_summary.len())
}

fn validate_audit_input(input: &AuditInput) -> Result<(), AuditError> {
    let required = [
        input.event_id.as_str(),
        input.actor.as_str(),
        input.run_id.as_str(),
        input.policy_rule.as_str(),
        input.decision.as_str(),
    ];
    let optional = [input.action_id.as_deref(), input.approval_id.as_deref()];
    if required.iter().any(|value| !is_canonical_id(value))
        || optional
            .iter()
            .flatten()
            .any(|value| !is_canonical_id(value))
    {
        return Err(AuditError::InvalidRecord);
    }
    if !validate_action_projection(
        input.action_id.as_deref(),
        input.action_summary.as_deref(),
        input.action_hash.as_deref(),
    ) {
        return Err(AuditError::InvalidRecord);
    }
    Ok(())
}

fn is_canonical_id(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= MAX_ID_CHARS && bounded_id(value) == value
}

fn hex_hash(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_lower_hex_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn decode_hash(value: &str) -> Result<[u8; 32], AuditError> {
    if !is_lower_hex_hash(value) {
        return Err(AuditError::Corrupt);
    }
    let mut output = [0u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = u8::from_str_radix(
            std::str::from_utf8(chunk).map_err(|_| AuditError::Corrupt)?,
            16,
        )
        .map_err(|_| AuditError::Corrupt)?;
    }
    Ok(output)
}

fn bounded_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_TEXT_BYTES)
        .collect()
}

fn bounded_id(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | ':' | '_' | '-')
        })
        .take(MAX_ID_CHARS)
        .collect()
}

#[cfg(unix)]
fn ensure_private_dir(path: &Path, created: bool) -> Result<(), AuditError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)
        .map_err(AuditError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if created {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(AuditError::Io)
    } else if mode == 0o700 {
        Ok(())
    } else {
        Err(AuditError::InsecurePermissions)
    }
}

#[cfg(windows)]
fn ensure_private_dir(path: &Path, _created: bool) -> Result<(), AuditError> {
    if crate::harness::config::check_permissions(path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(AuditError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
fn ensure_private_dir(_path: &Path, _created: bool) -> Result<(), AuditError> {
    Err(AuditError::InsecurePermissions)
}

#[cfg(unix)]
fn ensure_private_file(path: &Path, created: bool) -> Result<(), AuditError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)
        .map_err(AuditError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if created {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(AuditError::Io)
    } else if mode == 0o600 {
        Ok(())
    } else {
        Err(AuditError::InsecurePermissions)
    }
}

#[cfg(windows)]
fn ensure_private_file(path: &Path, _created: bool) -> Result<(), AuditError> {
    if crate::harness::config::check_permissions(path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(AuditError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
fn ensure_private_file(_path: &Path, _created: bool) -> Result<(), AuditError> {
    Err(AuditError::InsecurePermissions)
}
