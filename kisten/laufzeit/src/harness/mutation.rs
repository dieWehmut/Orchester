//! Deterministic source snapshots and mutation generations.
//!
//! A process result never decides whether source changed.  Callers capture the
//! configured source set before and after an operation while holding the
//! workspace lock, then feed both snapshots to [`MutationTracker`].

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use super::governance::{GuardError, GuardErrorKind, WorkspaceGuard, WorkspaceLocks};

// Structural limits are deliberately independent from the byte/file limits
// exposed in the config.  Keeping these hard ceilings here preserves the
// existing config wire shape while ensuring a hostile tree cannot consume
// unbounded memory or recursion depth.
const MAX_SNAPSHOT_ENTRIES: usize = 100_000;
const MAX_SNAPSHOT_DIRECTORIES: usize = 20_000;
const MAX_SNAPSHOT_DEPTH: usize = 128;
const MAX_SNAPSHOT_PATH_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ENTRY_PATH_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_files: usize,
    pub max_bytes: u64,
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWatchConfig {
    pub includes: Vec<PathBuf>,
    pub excludes: Vec<PathBuf>,
    pub limits: SnapshotLimits,
}

impl Default for SourceWatchConfig {
    fn default() -> Self {
        Self {
            includes: vec![PathBuf::from("src")],
            excludes: vec![
                PathBuf::from("target"),
                PathBuf::from(".git"),
                PathBuf::from(".orchester"),
            ],
            limits: SnapshotLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub root_hash: blake3::Hash,
    pub files: BTreeMap<PathBuf, blake3::Hash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotResult {
    Complete(WorkspaceSnapshot),
    LimitExceeded { files: usize, bytes: u64 },
}

impl SnapshotResult {
    pub fn complete(&self) -> Option<&WorkspaceSnapshot> {
        match self {
            Self::Complete(snapshot) => Some(snapshot),
            Self::LimitExceeded { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("source snapshot root is unavailable")]
    RootUnavailable(#[source] io::Error),
    #[error("source snapshot path must stay relative to the workspace: {path}")]
    InvalidRelativePath { path: PathBuf },
    #[error("source snapshot path traverses a link or reparse point: {path}")]
    LinkTraversal { path: PathBuf },
    #[error("source snapshot encountered an unsupported file type: {path}")]
    UnsupportedFileType { path: PathBuf },
    #[error("source snapshot filesystem operation failed at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug)]
pub struct WorkspaceSnapshotter {
    workspace: WorkspaceGuard,
    config: SourceWatchConfig,
}

impl WorkspaceSnapshotter {
    pub fn new(
        root: impl AsRef<Path>,
        mut config: SourceWatchConfig,
    ) -> Result<Self, SnapshotError> {
        let workspace = WorkspaceGuard::new(root.as_ref()).map_err(|error| {
            SnapshotError::RootUnavailable(io::Error::new(io::ErrorKind::PermissionDenied, error))
        })?;
        config.includes = normalize_paths(config.includes)?;
        config.excludes = normalize_paths(config.excludes)?;
        if config.includes.is_empty() {
            config.includes.push(PathBuf::from("."));
        }
        Ok(Self { workspace, config })
    }

    pub fn root(&self) -> &Path {
        self.workspace.root()
    }

    pub fn workspace(&self) -> &WorkspaceGuard {
        &self.workspace
    }

    pub fn capture(&self) -> Result<SnapshotResult, SnapshotError> {
        let mut accumulator = SnapshotAccumulator::new(self.config.limits);
        for include in &self.config.includes {
            match self.visit(include, &mut accumulator, 0) {
                Ok(Some(limit)) => return Ok(limit),
                Ok(None) => {}
                Err(SnapshotError::Io { source, .. })
                    if source.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(SnapshotResult::Complete(accumulator.finish()))
    }

    /// Capture while holding the process-local workspace read lock.  Callers
    /// that surround a mutating operation acquire `WorkspaceLocks::mutate`
    /// themselves and keep that guard across both captures and execution.
    pub async fn capture_locked(
        &self,
        locks: &WorkspaceLocks,
    ) -> Result<SnapshotResult, SnapshotError> {
        let _guard = locks.read(&self.workspace).await;
        self.capture()
    }

    fn visit(
        &self,
        relative: &Path,
        accumulator: &mut SnapshotAccumulator,
        depth: usize,
    ) -> Result<Option<SnapshotResult>, SnapshotError> {
        if self.excluded(relative) {
            return Ok(None);
        }
        if depth > MAX_SNAPSHOT_DEPTH {
            return Ok(Some(accumulator.limit_result()));
        }
        match self.workspace.directory_entries(relative) {
            Ok(children) => {
                if !accumulator.admit_directory(relative) {
                    return Ok(Some(accumulator.limit_result()));
                }
                // Consume the capability-backed stream incrementally.  We do
                // not sort or collect names: the final digest is ordered by
                // BTreeMap, while limits are enforced before the next entry
                // can allocate another recursive frame.
                for child in children {
                    let child = child.map_err(|error| snapshot_guard_error(relative, error))?;
                    let child = relative.join(child);
                    if let Some(limit) = self.visit(&child, accumulator, depth + 1)? {
                        return Ok(Some(limit));
                    }
                }
                return Ok(None);
            }
            Err(error) if error.kind() == GuardErrorKind::NotDirectory => {}
            Err(error) => return Err(snapshot_guard_error(relative, error)),
        }

        let mut file = self
            .workspace
            .open_snapshot_file(relative)
            .map_err(|error| snapshot_guard_error(relative, error))?;
        let metadata = file.metadata().map_err(|source| SnapshotError::Io {
            path: relative.to_path_buf(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(SnapshotError::UnsupportedFileType {
                path: relative.to_path_buf(),
            });
        }
        if !accumulator.admit_file(relative) {
            return Ok(Some(accumulator.file_limit_result()));
        }
        let remaining = accumulator
            .limits
            .max_bytes
            .saturating_sub(accumulator.byte_count);
        let mut content = Vec::with_capacity(remaining.min(64 * 1024) as usize);
        Read::by_ref(&mut file)
            .take(remaining.saturating_add(1))
            .read_to_end(&mut content)
            .map_err(|source| SnapshotError::Io {
                path: relative.to_path_buf(),
                source,
            })?;
        let actual_bytes = accumulator.byte_count.saturating_add(content.len() as u64);
        if actual_bytes > accumulator.limits.max_bytes {
            return Ok(Some(SnapshotResult::LimitExceeded {
                files: accumulator.file_count.saturating_add(1),
                bytes: accumulator.limits.max_bytes.saturating_add(1),
            }));
        }
        accumulator.record_file(relative, &content);
        Ok(None)
    }

    fn excluded(&self, relative: &Path) -> bool {
        self.config
            .excludes
            .iter()
            .any(|excluded| relative == excluded || relative.starts_with(excluded))
    }
}

struct SnapshotAccumulator {
    limits: SnapshotLimits,
    file_count: usize,
    byte_count: u64,
    entry_count: usize,
    directory_count: usize,
    path_bytes: u64,
    entries: BTreeMap<PathBuf, EntryDigest>,
    files: BTreeMap<PathBuf, blake3::Hash>,
}

impl SnapshotAccumulator {
    fn new(limits: SnapshotLimits) -> Self {
        Self {
            limits,
            file_count: 0,
            byte_count: 0,
            entry_count: 0,
            directory_count: 0,
            path_bytes: 0,
            entries: BTreeMap::new(),
            files: BTreeMap::new(),
        }
    }

    fn admit_directory(&mut self, path: &Path) -> bool {
        if !self.admit_entry(path, true) {
            return false;
        }
        self.entries.insert(
            path.to_path_buf(),
            EntryDigest {
                kind: b'd',
                hash: blake3::hash(&[]),
            },
        );
        true
    }

    fn admit_file(&mut self, path: &Path) -> bool {
        self.admit_entry(path, false)
    }

    fn admit_entry(&mut self, path: &Path, directory: bool) -> bool {
        let path_len = path_bytes(path).len();
        if path_len > MAX_ENTRY_PATH_BYTES
            || self.entry_count >= MAX_SNAPSHOT_ENTRIES
            || (directory && self.directory_count >= MAX_SNAPSHOT_DIRECTORIES)
            || (!directory && self.file_count >= self.limits.max_files)
            || self.path_bytes.saturating_add(path_len as u64) > MAX_SNAPSHOT_PATH_BYTES
        {
            return false;
        }
        self.entry_count += 1;
        self.path_bytes = self.path_bytes.saturating_add(path_len as u64);
        if directory {
            self.directory_count += 1;
        }
        true
    }

    fn record_file(&mut self, path: &Path, content: &[u8]) {
        let hash = blake3::hash(content);
        self.files.insert(path.to_path_buf(), hash);
        self.file_count += 1;
        self.byte_count += content.len() as u64;
        self.entries
            .insert(path.to_path_buf(), EntryDigest { kind: b'f', hash });
    }

    fn limit_result(&self) -> SnapshotResult {
        SnapshotResult::LimitExceeded {
            files: self.file_count,
            bytes: self.byte_count,
        }
    }

    fn file_limit_result(&self) -> SnapshotResult {
        SnapshotResult::LimitExceeded {
            files: self.file_count.saturating_add(1),
            bytes: self.byte_count,
        }
    }

    fn finish(self) -> WorkspaceSnapshot {
        let mut root = blake3::Hasher::new();
        root.update(b"orchester-workspace-snapshot-v1\0");
        for (path, entry) in self.entries {
            let path = path_bytes(&path);
            root.update(&(path.len() as u64).to_le_bytes());
            root.update(&path);
            root.update(&[entry.kind]);
            root.update(entry.hash.as_bytes());
        }
        WorkspaceSnapshot {
            root_hash: root.finalize(),
            files: self.files,
        }
    }
}

struct EntryDigest {
    kind: u8,
    hash: blake3::Hash,
}

fn normalize_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, SnapshotError> {
    let mut normalized = Vec::with_capacity(paths.len());
    for path in paths {
        let mut clean = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => clean.push(part),
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(SnapshotError::InvalidRelativePath { path });
                }
            }
        }
        if clean.as_os_str().is_empty() {
            clean.push(".");
        }
        normalized.push(clean);
    }
    normalized.sort_by_key(|left| path_bytes(left));
    normalized.dedup();
    let mut disjoint = Vec::with_capacity(normalized.len());
    for path in normalized {
        if disjoint
            .iter()
            .any(|parent: &PathBuf| path == *parent || path.starts_with(parent))
        {
            continue;
        }
        disjoint.push(path);
    }
    Ok(disjoint)
}

fn path_bytes(path: &Path) -> Vec<u8> {
    let mut normalized = Vec::new();
    for (index, component) in path.components().enumerate() {
        if index > 0 {
            normalized.push(b'/');
        }
        let text = component.as_os_str().to_string_lossy();
        normalized.extend_from_slice(text.as_bytes());
    }
    normalized
}

fn snapshot_guard_error(path: &Path, error: GuardError) -> SnapshotError {
    match error.kind() {
        GuardErrorKind::LinkTraversal => SnapshotError::LinkTraversal {
            path: path.to_path_buf(),
        },
        GuardErrorKind::InvalidPath | GuardErrorKind::Outside | GuardErrorKind::Protected => {
            SnapshotError::InvalidRelativePath {
                path: path.to_path_buf(),
            }
        }
        GuardErrorKind::NotDirectory => SnapshotError::UnsupportedFileType {
            path: path.to_path_buf(),
        },
        GuardErrorKind::NotFound => SnapshotError::Io {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::NotFound, error),
        },
        _ => SnapshotError::Io {
            path: path.to_path_buf(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, error),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutationObservation {
    pub generation: u64,
    pub changed: bool,
    pub uncertain: bool,
}

impl MutationObservation {
    pub fn invalidates_validation(self) -> bool {
        self.changed || self.uncertain
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutationTracker {
    generation: u64,
}

impl MutationTracker {
    pub fn new(generation: u64) -> Self {
        Self { generation }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Advance exactly once when the source state changed or could not be
    /// compared within limits.  Limit uncertainty fails closed and therefore
    /// invalidates every validator pass from the prior generation.
    pub fn observe(
        &mut self,
        before: &SnapshotResult,
        after: &SnapshotResult,
    ) -> MutationObservation {
        let (changed, uncertain) = match (before, after) {
            (SnapshotResult::Complete(before), SnapshotResult::Complete(after)) => {
                (before.root_hash != after.root_hash, false)
            }
            _ => (false, true),
        };
        if changed || uncertain {
            self.generation = self.generation.saturating_add(1);
        }
        MutationObservation {
            generation: self.generation,
            changed,
            uncertain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_snapshot_admission_fails_closed_at_each_hard_limit() {
        let limits = SnapshotLimits {
            max_files: 4,
            max_bytes: 64,
        };

        let mut entries = SnapshotAccumulator::new(limits);
        entries.entry_count = MAX_SNAPSHOT_ENTRIES;
        assert!(!entries.admit_file(Path::new("entry")));

        let mut directories = SnapshotAccumulator::new(limits);
        directories.directory_count = MAX_SNAPSHOT_DIRECTORIES;
        assert!(!directories.admit_directory(Path::new("directory")));

        let mut paths = SnapshotAccumulator::new(limits);
        paths.path_bytes = MAX_SNAPSHOT_PATH_BYTES;
        assert!(!paths.admit_file(Path::new("path")));

        let mut files = SnapshotAccumulator::new(limits);
        files.file_count = limits.max_files;
        assert!(!files.admit_file(Path::new("file")));

        let long_path = PathBuf::from("x".repeat(MAX_ENTRY_PATH_BYTES + 1));
        let mut length = SnapshotAccumulator::new(limits);
        assert!(!length.admit_file(&long_path));
    }
}
