//! Deterministic source snapshots and mutation generations.
//!
//! A process result never decides whether source changed.  Callers capture the
//! configured source set before and after an operation while holding the
//! workspace lock, then feed both snapshots to [`MutationTracker`].

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use super::governance::WorkspaceLocks;

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

#[derive(Debug, Clone)]
pub struct WorkspaceSnapshotter {
    root: PathBuf,
    config: SourceWatchConfig,
}

impl WorkspaceSnapshotter {
    pub fn new(
        root: impl AsRef<Path>,
        mut config: SourceWatchConfig,
    ) -> Result<Self, SnapshotError> {
        let requested_root = root.as_ref();
        let metadata =
            fs::symlink_metadata(requested_root).map_err(SnapshotError::RootUnavailable)?;
        if is_link_or_reparse(&metadata) {
            return Err(SnapshotError::LinkTraversal {
                path: requested_root.to_path_buf(),
            });
        }
        if !metadata.is_dir() {
            return Err(SnapshotError::InvalidRelativePath {
                path: requested_root.to_path_buf(),
            });
        }
        let root = fs::canonicalize(requested_root).map_err(SnapshotError::RootUnavailable)?;
        config.includes = normalize_paths(config.includes)?;
        config.excludes = normalize_paths(config.excludes)?;
        if config.includes.is_empty() {
            config.includes.push(PathBuf::from("."));
        }
        Ok(Self { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn capture(&self) -> Result<SnapshotResult, SnapshotError> {
        let mut accumulator = SnapshotAccumulator::new(self.config.limits);
        for include in &self.config.includes {
            let absolute = self.root.join(include);
            match fs::symlink_metadata(&absolute) {
                Ok(metadata) => {
                    if is_link_or_reparse(&metadata) {
                        return Err(SnapshotError::LinkTraversal {
                            path: include.clone(),
                        });
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(SnapshotError::Io {
                        path: include.clone(),
                        source,
                    })
                }
            }
            self.reject_linked_components(include)?;
            if let Some(limit) = self.visit(include, &mut accumulator)? {
                return Ok(limit);
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
        workspace_identity: &str,
    ) -> Result<SnapshotResult, SnapshotError> {
        let _guard = locks.read(workspace_identity).await;
        self.capture()
    }

    fn visit(
        &self,
        relative: &Path,
        accumulator: &mut SnapshotAccumulator,
    ) -> Result<Option<SnapshotResult>, SnapshotError> {
        if self.excluded(relative) {
            return Ok(None);
        }
        let absolute = self.root.join(relative);
        let metadata = fs::symlink_metadata(&absolute).map_err(|source| SnapshotError::Io {
            path: relative.to_path_buf(),
            source,
        })?;
        if is_link_or_reparse(&metadata) {
            return Err(SnapshotError::LinkTraversal {
                path: relative.to_path_buf(),
            });
        }
        if metadata.is_dir() {
            accumulator.record_directory(relative);
            let entries = fs::read_dir(&absolute).map_err(|source| SnapshotError::Io {
                path: relative.to_path_buf(),
                source,
            })?;
            let mut children = entries
                .map(|entry| {
                    entry
                        .map(|entry| relative.join(entry.file_name()))
                        .map_err(|source| SnapshotError::Io {
                            path: relative.to_path_buf(),
                            source,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            children.sort_by_key(|left| path_bytes(left));
            for child in children {
                if let Some(limit) = self.visit(&child, accumulator)? {
                    return Ok(Some(limit));
                }
            }
            return Ok(None);
        }
        if metadata.is_file() {
            let bytes = metadata.len();
            let files = accumulator.file_count.saturating_add(1);
            let total_bytes = accumulator.byte_count.saturating_add(bytes);
            if files > accumulator.limits.max_files || total_bytes > accumulator.limits.max_bytes {
                return Ok(Some(SnapshotResult::LimitExceeded {
                    files,
                    bytes: total_bytes,
                }));
            }
            let content = fs::read(&absolute).map_err(|source| SnapshotError::Io {
                path: relative.to_path_buf(),
                source,
            })?;
            let actual_bytes = accumulator.byte_count.saturating_add(content.len() as u64);
            if files > accumulator.limits.max_files || actual_bytes > accumulator.limits.max_bytes {
                return Ok(Some(SnapshotResult::LimitExceeded {
                    files,
                    bytes: actual_bytes,
                }));
            }
            accumulator.record_file(relative, &content);
            return Ok(None);
        }
        Err(SnapshotError::UnsupportedFileType {
            path: relative.to_path_buf(),
        })
    }

    fn reject_linked_components(&self, relative: &Path) -> Result<(), SnapshotError> {
        let mut current = self.root.clone();
        let mut checked = PathBuf::new();
        for component in relative.components() {
            if component == Component::CurDir {
                continue;
            }
            current.push(component.as_os_str());
            checked.push(component.as_os_str());
            if !current.exists() {
                break;
            }
            let metadata = fs::symlink_metadata(&current).map_err(|source| SnapshotError::Io {
                path: checked.clone(),
                source,
            })?;
            if is_link_or_reparse(&metadata) {
                return Err(SnapshotError::LinkTraversal { path: checked });
            }
        }
        Ok(())
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
    entries: BTreeMap<PathBuf, EntryDigest>,
    files: BTreeMap<PathBuf, blake3::Hash>,
}

impl SnapshotAccumulator {
    fn new(limits: SnapshotLimits) -> Self {
        Self {
            limits,
            file_count: 0,
            byte_count: 0,
            entries: BTreeMap::new(),
            files: BTreeMap::new(),
        }
    }

    fn record_directory(&mut self, path: &Path) {
        self.entries
            .entry(path.to_path_buf())
            .or_insert(EntryDigest {
                kind: b'd',
                hash: blake3::hash(&[]),
            });
    }

    fn record_file(&mut self, path: &Path, content: &[u8]) {
        let hash = blake3::hash(content);
        if self.files.insert(path.to_path_buf(), hash).is_none() {
            self.file_count += 1;
            self.byte_count += content.len() as u64;
        }
        self.entries
            .insert(path.to_path_buf(), EntryDigest { kind: b'f', hash });
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

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
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
