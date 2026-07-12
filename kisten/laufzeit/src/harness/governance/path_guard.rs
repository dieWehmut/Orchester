use std::collections::hash_map::RandomState;
use std::fs::{self, File, Metadata, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

/// Stable categories callers can use in policy and audit decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardErrorKind {
    InvalidPath,
    Outside,
    LinkTraversal,
    Protected,
    NotFound,
    NotDirectory,
    Changed,
    Io,
}

/// A path was rejected before any governed file operation was opened.
#[derive(Debug, Error)]
pub enum GuardError {
    #[error("path is empty, contains NUL, or has an invalid prefix: {path:?}")]
    InvalidPath { path: PathBuf },
    #[error("path escapes the workspace: {path:?}")]
    Outside { path: PathBuf },
    #[error("path traverses a link or reparse point: {path:?}")]
    LinkTraversal { path: PathBuf },
    #[error("path is protected from agent access: {path:?}")]
    Protected { path: PathBuf },
    #[error("path does not exist: {path:?}")]
    NotFound { path: PathBuf },
    #[error("a non-directory path component was traversed: {path:?}")]
    NotDirectory { path: PathBuf },
    #[error("path identity changed after it was resolved: {path:?}")]
    Changed { path: PathBuf },
    #[error("could not {operation} {path:?}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl GuardError {
    pub fn kind(&self) -> GuardErrorKind {
        match self {
            Self::InvalidPath { .. } => GuardErrorKind::InvalidPath,
            Self::Outside { .. } => GuardErrorKind::Outside,
            Self::LinkTraversal { .. } => GuardErrorKind::LinkTraversal,
            Self::Protected { .. } => GuardErrorKind::Protected,
            Self::NotFound { .. } => GuardErrorKind::NotFound,
            Self::NotDirectory { .. } => GuardErrorKind::NotDirectory,
            Self::Changed { .. } => GuardErrorKind::Changed,
            Self::Io { .. } => GuardErrorKind::Io,
        }
    }
}

/// Filesystem operations are abstracted so resolution can be fault-injected later.
pub trait PathResolver: Send + Sync {
    fn resolve_existing_no_follow(&self, path: &Path) -> Result<PathBuf, GuardError>;
    fn resolve_parent_no_links(&self, path: &Path) -> Result<PathBuf, GuardError>;
}

/// Resolver backed by the host filesystem.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilesystemResolver;

impl PathResolver for FilesystemResolver {
    fn resolve_existing_no_follow(&self, path: &Path) -> Result<PathBuf, GuardError> {
        resolve_existing_path_no_links(path)
    }

    fn resolve_parent_no_links(&self, path: &Path) -> Result<PathBuf, GuardError> {
        let mut candidate = path.parent().ok_or_else(|| GuardError::InvalidPath {
            path: path.to_path_buf(),
        })?;
        loop {
            match fs::symlink_metadata(candidate) {
                Ok(metadata) => {
                    if !metadata.is_dir() {
                        return Err(GuardError::NotDirectory {
                            path: candidate.to_path_buf(),
                        });
                    }
                    return resolve_existing_path_no_links(candidate);
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    candidate = candidate.parent().ok_or_else(|| GuardError::NotFound {
                        path: path.to_path_buf(),
                    })?;
                }
                Err(source) => {
                    return Err(GuardError::Io {
                        operation: "inspect parent",
                        path: candidate.to_path_buf(),
                        source,
                    });
                }
            }
        }
    }
}

/// A checked path plus the filesystem identities needed for write-time revalidation.
#[derive(Debug)]
pub struct ResolvedPath {
    pub requested: PathBuf,
    pub final_path: PathBuf,
    pub canonical_parent: PathBuf,
    root: PathBuf,
    root_identity: PathIdentity,
    parent_identity: PathIdentity,
    target_identity: Option<PathIdentity>,
    mode: ResolveMode,
}

impl ResolvedPath {
    /// Resolve again immediately before open or rename and reject any identity change.
    pub fn revalidate<R: PathResolver>(
        &self,
        guard: &WorkspaceGuard<R>,
    ) -> Result<ResolvedPath, GuardError> {
        if self.root != guard.root {
            return Err(GuardError::Changed {
                path: self.requested.clone(),
            });
        }
        let current = guard.resolve(&self.requested, self.mode)?;
        if self.final_path != current.final_path
            || self.canonical_parent != current.canonical_parent
            || self.root_identity != current.root_identity
            || self.parent_identity != current.parent_identity
            || self.target_identity != current.target_identity
        {
            return Err(GuardError::Changed {
                path: self.requested.clone(),
            });
        }
        Ok(current)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolveMode {
    Read,
    Write,
}

/// Resolves agent-supplied paths without allowing workspace or link traversal.
pub struct WorkspaceGuard<R = FilesystemResolver> {
    root: PathBuf,
    root_identity: PathIdentity,
    resolver: R,
    protected: Vec<PathBuf>,
}

impl WorkspaceGuard<FilesystemResolver> {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, GuardError> {
        Self::with_resolver(root, FilesystemResolver)
    }
}

impl<R: PathResolver> WorkspaceGuard<R> {
    pub fn with_resolver(root: impl AsRef<Path>, resolver: R) -> Result<Self, GuardError> {
        let requested_root = root.as_ref();
        if requested_root.as_os_str().is_empty() || contains_nul(requested_root) {
            return Err(GuardError::InvalidPath {
                path: requested_root.to_path_buf(),
            });
        }
        let metadata = symlink_metadata(requested_root)?;
        reject_link(requested_root, &metadata)?;
        if !metadata.is_dir() {
            return Err(GuardError::NotDirectory {
                path: requested_root.to_path_buf(),
            });
        }
        let root = resolver.resolve_existing_no_follow(requested_root)?;
        let root_identity = path_identity(&root)?;
        Ok(Self {
            root,
            root_identity,
            resolver,
            protected: vec![PathBuf::from(".git"), PathBuf::from(".orchester")],
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve_read(&self, requested: &Path) -> Result<ResolvedPath, GuardError> {
        self.resolve(requested, ResolveMode::Read)
    }

    pub fn resolve_write(&self, requested: &Path) -> Result<ResolvedPath, GuardError> {
        self.resolve(requested, ResolveMode::Write)
    }

    /// Create a never-followed temporary file beside a fully existing target parent.
    pub fn atomic_write_target(
        &self,
        resolved: &ResolvedPath,
    ) -> Result<AtomicWriteTarget, GuardError> {
        let current = resolved.revalidate(self)?;
        if current.mode != ResolveMode::Write {
            return Err(GuardError::InvalidPath {
                path: current.requested,
            });
        }
        let final_parent = current
            .final_path
            .parent()
            .ok_or_else(|| GuardError::InvalidPath {
                path: current.final_path.clone(),
            })?;
        if final_parent != current.canonical_parent {
            return Err(GuardError::NotFound {
                path: final_parent.to_path_buf(),
            });
        }
        if current.target_identity.is_some()
            && fs::symlink_metadata(&current.final_path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false)
        {
            return Err(GuardError::NotDirectory {
                path: current.final_path,
            });
        }

        for _ in 0..32 {
            let path = final_parent.join(format!(".orchester-write-{:016x}.tmp", random_token()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok(AtomicWriteTarget { path, file }),
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(GuardError::Io {
                        operation: "create atomic write target",
                        path,
                        source,
                    });
                }
            }
        }
        Err(GuardError::Io {
            operation: "create atomic write target",
            path: final_parent.to_path_buf(),
            source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary name collisions"),
        })
    }

    fn resolve(&self, requested: &Path, mode: ResolveMode) -> Result<ResolvedPath, GuardError> {
        if requested.as_os_str().is_empty() || contains_nul(requested) {
            return Err(GuardError::InvalidPath {
                path: requested.to_path_buf(),
            });
        }
        let current_root_identity = path_identity(&self.root)?;
        if current_root_identity != self.root_identity {
            return Err(GuardError::Changed {
                path: self.root.clone(),
            });
        }

        let relative = self.normalize_request(requested)?;
        if self.is_protected(&relative) {
            return Err(GuardError::Protected {
                path: requested.to_path_buf(),
            });
        }

        let final_path = self.root.join(&relative);
        let mut current = self.root.clone();
        let mut canonical_parent = self.root.clone();
        let components: Vec<_> = relative.components().collect();
        let mut target_identity = None;
        let mut encountered_missing = false;

        for (index, component) in components.iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(GuardError::InvalidPath {
                    path: requested.to_path_buf(),
                });
            };
            current.push(name);
            if encountered_missing {
                continue;
            }
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    reject_link(&current, &metadata)?;
                    let canonical = self.resolver.resolve_existing_no_follow(&current)?;
                    if !canonical.starts_with(&self.root) {
                        return Err(GuardError::Outside {
                            path: requested.to_path_buf(),
                        });
                    }
                    let is_last = index + 1 == components.len();
                    if !is_last && !metadata.is_dir() {
                        return Err(GuardError::NotDirectory { path: current });
                    }
                    if metadata.is_dir() && !is_last {
                        canonical_parent = canonical.clone();
                    }
                    if is_last {
                        target_identity = Some(path_identity(&canonical)?);
                    }
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    if mode == ResolveMode::Read {
                        return Err(GuardError::NotFound {
                            path: requested.to_path_buf(),
                        });
                    }
                    encountered_missing = true;
                }
                Err(source) => {
                    return Err(GuardError::Io {
                        operation: "inspect path",
                        path: current,
                        source,
                    });
                }
            }
        }

        let canonical_final = if encountered_missing {
            final_path
        } else {
            self.resolver.resolve_existing_no_follow(&final_path)?
        };
        let parent_identity = path_identity(&canonical_parent)?;
        Ok(ResolvedPath {
            requested: requested.to_path_buf(),
            final_path: canonical_final,
            canonical_parent,
            root: self.root.clone(),
            root_identity: current_root_identity,
            parent_identity,
            target_identity,
            mode,
        })
    }

    fn normalize_request(&self, requested: &Path) -> Result<PathBuf, GuardError> {
        let relative = if requested.is_absolute() {
            strip_workspace_prefix(requested, &self.root).ok_or_else(|| GuardError::Outside {
                path: requested.to_path_buf(),
            })?
        } else {
            requested.to_path_buf()
        };
        let mut normalized = PathBuf::new();
        for component in relative.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(value) => normalized.push(value),
                Component::ParentDir => {
                    if !normalized.pop() {
                        return Err(GuardError::Outside {
                            path: requested.to_path_buf(),
                        });
                    }
                }
                Component::Prefix(_) | Component::RootDir => {
                    return Err(GuardError::InvalidPath {
                        path: requested.to_path_buf(),
                    });
                }
            }
        }
        if normalized.as_os_str().is_empty() {
            return Err(GuardError::InvalidPath {
                path: requested.to_path_buf(),
            });
        }
        Ok(normalized)
    }

    fn is_protected(&self, relative: &Path) -> bool {
        self.protected
            .iter()
            .any(|protected| path_starts_with(relative, protected))
    }
}

/// A newly created regular file that is removed unless the caller renames it.
pub struct AtomicWriteTarget {
    path: PathBuf,
    file: File,
}

impl AtomicWriteTarget {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}

impl Drop for AtomicWriteTarget {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn path_identity(path: &Path) -> Result<PathIdentity, GuardError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = symlink_metadata(path)?;
    reject_link(path, &metadata)?;
    Ok(PathIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathIdentity {
    attributes: u32,
    created: u64,
    modified: u64,
    size: u64,
}

#[cfg(windows)]
fn path_identity(path: &Path) -> Result<PathIdentity, GuardError> {
    use std::os::windows::fs::MetadataExt;

    let metadata = symlink_metadata(path)?;
    reject_link(path, &metadata)?;
    let modified = if metadata.is_dir() {
        0
    } else {
        metadata.last_write_time()
    };
    Ok(PathIdentity {
        attributes: metadata.file_attributes(),
        created: metadata.creation_time(),
        modified,
        size: metadata.file_size(),
    })
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathIdentity {
    canonical: PathBuf,
}

#[cfg(not(any(unix, windows)))]
fn path_identity(path: &Path) -> Result<PathIdentity, GuardError> {
    let metadata = symlink_metadata(path)?;
    reject_link(path, &metadata)?;
    Ok(PathIdentity {
        canonical: fs::canonicalize(path).map_err(|source| GuardError::Io {
            operation: "canonicalize",
            path: path.to_path_buf(),
            source,
        })?,
    })
}

fn symlink_metadata(path: &Path) -> Result<Metadata, GuardError> {
    fs::symlink_metadata(path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => GuardError::NotFound {
            path: path.to_path_buf(),
        },
        _ => GuardError::Io {
            operation: "inspect path",
            path: path.to_path_buf(),
            source,
        },
    })
}

fn resolve_existing_path_no_links(path: &Path) -> Result<PathBuf, GuardError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match component {
            Component::Prefix(_) => continue,
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(GuardError::InvalidPath {
                    path: path.to_path_buf(),
                });
            }
            Component::RootDir | Component::Normal(_) => {
                let metadata = symlink_metadata(&current)?;
                reject_link(&current, &metadata)?;
            }
        }
    }
    fs::canonicalize(path).map_err(|source| GuardError::Io {
        operation: "canonicalize",
        path: path.to_path_buf(),
        source,
    })
}

fn reject_link(path: &Path, metadata: &Metadata) -> Result<(), GuardError> {
    if metadata_is_link(metadata) {
        Err(GuardError::LinkTraversal {
            path: path.to_path_buf(),
        })
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn metadata_is_link(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_link(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn contains_nul(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().contains(&0)
}

#[cfg(windows)]
fn contains_nul(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().any(|unit| unit == 0)
}

#[cfg(not(any(unix, windows)))]
fn contains_nul(path: &Path) -> bool {
    path.to_string_lossy().contains('\0')
}

#[cfg(windows)]
fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    let path_parts: Vec<_> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    let prefix_parts: Vec<_> = prefix
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    path_parts.starts_with(&prefix_parts)
}

#[cfg(not(windows))]
fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    path.starts_with(prefix)
}

#[cfg(windows)]
fn strip_workspace_prefix(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let path_components: Vec<_> = path.components().collect();
    let prefix_components: Vec<_> = prefix.components().collect();
    if path_components.len() < prefix_components.len()
        || !path_components
            .iter()
            .zip(&prefix_components)
            .all(|(path, prefix)| windows_component_key(path) == windows_component_key(prefix))
    {
        return None;
    }

    let mut relative = PathBuf::new();
    for component in path_components.iter().skip(prefix_components.len()) {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir => relative.push(".."),
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    Some(relative)
}

#[cfg(windows)]
fn windows_component_key(component: &Component<'_>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let mut key: Vec<u16> = component.as_os_str().encode_wide().collect();
    let verbatim_prefix: Vec<u16> = "\\\\?\\".encode_utf16().collect();
    if key.starts_with(&verbatim_prefix) {
        key.drain(..verbatim_prefix.len());
    }
    for unit in &mut key {
        if (b'A' as u16..=b'Z' as u16).contains(unit) {
            *unit += b'a' as u16 - b'A' as u16;
        }
    }
    key
}

#[cfg(not(windows))]
fn strip_workspace_prefix(path: &Path, prefix: &Path) -> Option<PathBuf> {
    path.strip_prefix(prefix).ok().map(PathBuf::from)
}

fn random_token() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    std::process::id().hash(&mut hasher);
    NEXT_TEMP_FILE
        .fetch_add(1, Ordering::Relaxed)
        .hash(&mut hasher);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    hasher.finish()
}
