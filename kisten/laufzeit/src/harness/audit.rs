//! Append-only, redacted hash-chain audit sink.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const SCHEMA_VERSION: u16 = 1;
const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit path is unavailable")]
    Io(#[source] io::Error),
    #[error("audit record is invalid")]
    InvalidRecord,
    #[error("audit chain is corrupt")]
    Corrupt,
    #[error("audit sink lock is poisoned")]
    LockPoisoned,
    #[error("audit sequence is not contiguous")]
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditInput {
    pub occurred_at: String,
    pub actor: String,
    pub run_id: String,
    pub action_id: Option<String>,
    pub approval_id: Option<String>,
    pub policy_rule: String,
    pub decision: String,
    pub result_summary: String,
}

impl AuditInput {
    /// Deterministic fixture constructor used by offline tests.
    pub fn test(sequence: u64, run_id: &str, action_id: &str, summary: &str) -> Self {
        Self {
            occurred_at: format!("2026-07-12T00:00:{sequence:02}Z"),
            actor: "local-user".into(),
            run_id: run_id.into(),
            action_id: Some(action_id.into()),
            approval_id: None,
            policy_rule: "test.rule".into(),
            decision: "allow".into(),
            result_summary: summary.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AuditEntry {
    schema_version: u16,
    sequence: u64,
    occurred_at: String,
    actor: String,
    run_id: String,
    action_id: Option<String>,
    approval_id: Option<String>,
    policy_rule: String,
    decision: String,
    result_summary: String,
    prev_hash: String,
    entry_hash: String,
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
            fs::create_dir_all(parent).map_err(AuditError::Io)?;
            set_private_dir(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .map_err(AuditError::Io)?;
        set_private_file(&path);
        let (next_sequence, head_hash) = scan_file(&mut file)?;
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
        let mut file = self.file.lock().map_err(|_| AuditError::LockPoisoned)?;
        let mut sequence = self
            .next_sequence
            .lock()
            .map_err(|_| AuditError::LockPoisoned)?;
        let mut previous = self
            .head_hash
            .lock()
            .map_err(|_| AuditError::LockPoisoned)?;
        // Re-verify the durable file before every append.  A process must
        // fail closed if another writer or a crash left a broken chain; it
        // must never extend corrupted history with a seemingly valid tail.
        file.flush().map_err(AuditError::Io)?;
        file.seek(SeekFrom::Start(0)).map_err(AuditError::Io)?;
        let (durable_next, durable_head) = scan_reader(BufReader::new(&mut *file))?;
        file.seek(SeekFrom::End(0)).map_err(AuditError::Io)?;
        if durable_next != *sequence || durable_head != *previous {
            return Err(AuditError::Corrupt);
        }
        let current = *sequence;
        let entry = AuditEntry {
            schema_version: SCHEMA_VERSION,
            sequence: current,
            occurred_at: bounded_text(&input.occurred_at),
            actor: bounded_id(&input.actor),
            run_id: bounded_id(&input.run_id),
            action_id: input.action_id.as_deref().map(bounded_id),
            approval_id: input.approval_id.as_deref().map(bounded_id),
            policy_rule: bounded_id(&input.policy_rule),
            decision: bounded_id(&input.decision),
            // Do not persist model/tool output.  The byte count is useful for
            // diagnostics and cannot disclose an unknown future token format.
            result_summary: format!("summary_bytes={}", input.result_summary.len()),
            prev_hash: previous.clone(),
            entry_hash: String::new(),
        };
        let hash = hash_entry(&entry)?;
        let mut committed = entry;
        committed.entry_hash = hex_hash(&hash);
        let line = serde_json::to_vec(&committed).map_err(|_| AuditError::InvalidRecord)?;
        file.write_all(&line).map_err(AuditError::Io)?;
        file.write_all(b"\n").map_err(AuditError::Io)?;
        file.sync_data().map_err(AuditError::Io)?;
        *previous = committed.entry_hash.clone();
        *sequence = current.checked_add(1).ok_or(AuditError::Sequence)?;
        Ok(current)
    }

    pub fn verify(&self) -> Result<AuditVerification, AuditError> {
        let mut file = self.file.lock().map_err(|_| AuditError::LockPoisoned)?;
        file.flush().map_err(AuditError::Io)?;
        file.seek(SeekFrom::Start(0)).map_err(AuditError::Io)?;
        let result = scan_reader(BufReader::new(&mut *file));
        file.seek(SeekFrom::End(0)).map_err(AuditError::Io)?;
        result.map(|(next, head)| AuditVerification {
            entries: next.saturating_sub(1),
            head_hash: if head == GENESIS {
                None
            } else {
                decode_hash(&head).ok()
            },
        })
    }
}

fn scan_file(file: &mut File) -> Result<(u64, String), AuditError> {
    file.seek(SeekFrom::Start(0)).map_err(AuditError::Io)?;
    let result = scan_reader(BufReader::new(&mut *file));
    file.seek(SeekFrom::End(0)).map_err(AuditError::Io)?;
    result
}

fn scan_reader<R: BufRead>(reader: R) -> Result<(u64, String), AuditError> {
    let mut expected_sequence = 1u64;
    let mut previous = GENESIS.to_owned();
    for line in reader.lines() {
        let line = line.map_err(AuditError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: AuditEntry = serde_json::from_str(&line).map_err(|_| AuditError::Corrupt)?;
        if entry.schema_version != SCHEMA_VERSION
            || entry.sequence != expected_sequence
            || entry.prev_hash != previous
            || entry.entry_hash.len() != 64
        {
            return Err(AuditError::Corrupt);
        }
        let expected_hash = hex_hash(&hash_entry(&entry)?);
        if expected_hash != entry.entry_hash {
            return Err(AuditError::Corrupt);
        }
        previous = entry.entry_hash;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(AuditError::Sequence)?;
    }
    Ok((expected_sequence, previous))
}

fn hash_entry(entry: &AuditEntry) -> Result<[u8; 32], AuditError> {
    let mut unsigned = entry.clone();
    unsigned.entry_hash.clear();
    let bytes = serde_json::to_vec(&unsigned).map_err(|_| AuditError::InvalidRecord)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

fn hex_hash(hash: &[u8; 32]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hash(value: &str) -> Result<[u8; 32], AuditError> {
    if value.len() != 64 {
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
        .take(128)
        .collect()
}

#[cfg(unix)]
fn set_private_dir(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_private_dir(_path: &Path) {}

#[cfg(unix)]
fn set_private_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_private_file(_path: &Path) {}
