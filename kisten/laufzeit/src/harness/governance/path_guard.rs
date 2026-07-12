use std::collections::hash_map::RandomState;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use cap_fs_ext::OpenOptionsExt;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File as CapFile, OpenOptions};
use thiserror::Error;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);
const DEFAULT_READ_LIMIT: u64 = 16 * 1024 * 1024;

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
    LimitExceeded,
    Unsupported,
    Io,
}

/// A path was rejected before a governed operation could use it.
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
    #[error("guarded read exceeds {limit} bytes: {path:?}")]
    LimitExceeded { path: PathBuf, limit: u64 },
    #[error("secure path operations are unsupported on this platform")]
    Unsupported,
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
            Self::LimitExceeded { .. } => GuardErrorKind::LimitExceeded,
            Self::Unsupported => GuardErrorKind::Unsupported,
            Self::Io { .. } => GuardErrorKind::Io,
        }
    }
}

/// Compatibility resolver for diagnostics which do not perform file I/O.
pub trait PathResolver: Send + Sync {
    fn resolve_existing_no_follow(&self, path: &Path) -> Result<PathBuf, GuardError>;
    fn resolve_parent_no_links(&self, path: &Path) -> Result<PathBuf, GuardError>;
}

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

/// An unforgeable lock key derived only from the OS object id of the root
/// handle. Path spelling is deliberately excluded so aliases share a lock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct WorkspaceIdentity {
    object: ObjectIdentity,
}

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ObjectIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ObjectIdentity {
    volume_serial: u32,
    file_index: u64,
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ObjectIdentity;

/// A checked path plus handle-derived identities for write-time revalidation.
#[derive(Debug)]
pub struct ResolvedPath {
    pub requested: PathBuf,
    pub final_path: PathBuf,
    pub canonical_parent: PathBuf,
    relative: PathBuf,
    root_identity: ObjectIdentity,
    parent_identity: ObjectIdentity,
    target_identity: Option<ObjectIdentity>,
    target_kind: Option<NodeKind>,
    mode: ResolveMode,
}

impl ResolvedPath {
    /// Resolve again through directory capabilities and compare OS object ids.
    pub fn revalidate(&self, guard: &WorkspaceGuard) -> Result<ResolvedPath, GuardError> {
        let current = guard.resolve_normalized(&self.requested, self.mode)?;
        if self.relative != current.relative
            || self.final_path != current.final_path
            || self.canonical_parent != current.canonical_parent
            || self.root_identity != current.root_identity
            || self.parent_identity != current.parent_identity
            || self.target_identity != current.target_identity
            || self.target_kind != current.target_kind
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeKind {
    File,
    Directory,
}

/// A workspace root held as an OS directory capability for its full lifetime.
pub struct WorkspaceGuard {
    root: PathBuf,
    root_dir: Dir,
    identity: WorkspaceIdentity,
    protected: Vec<PathBuf>,
}

impl std::fmt::Debug for WorkspaceGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkspaceGuard")
            .field("root", &self.root)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl WorkspaceGuard {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, GuardError> {
        let requested_root = root.as_ref();
        if requested_root.as_os_str().is_empty() || contains_nul(requested_root) {
            return Err(GuardError::InvalidPath {
                path: requested_root.to_path_buf(),
            });
        }
        let lexical_root = absolute_lexical_path(requested_root)?;
        let root_dir = open_root_capability(&lexical_root)?;
        let object = identity_from_dir(&root_dir, &lexical_root)?;
        let root = canonical_display_path(&lexical_root, &object)?;
        let identity = WorkspaceIdentity {
            object: object.clone(),
        };
        Ok(Self {
            root,
            root_dir,
            identity,
            protected: vec![PathBuf::from(".git"), PathBuf::from(".orchester")],
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn identity(&self) -> &WorkspaceIdentity {
        &self.identity
    }

    pub fn resolve_read(&self, requested: &Path) -> Result<ResolvedPath, GuardError> {
        self.resolve_normalized(requested, ResolveMode::Read)
    }

    pub fn resolve_write(&self, requested: &Path) -> Result<ResolvedPath, GuardError> {
        self.resolve_normalized(requested, ResolveMode::Write)
    }

    /// Open a previously resolved file relative to a verified parent handle.
    pub fn open_read(&self, resolved: &ResolvedPath) -> Result<File, GuardError> {
        if resolved.mode != ResolveMode::Read {
            return Err(GuardError::InvalidPath {
                path: resolved.requested.clone(),
            });
        }
        let current = resolved.revalidate(self)?;
        if current.target_kind != Some(NodeKind::File) {
            return Err(GuardError::NotDirectory {
                path: current.requested,
            });
        }
        let (parent, basename) = self.open_parent(&current.relative)?;
        let cap_file = open_file_nofollow(&parent, &basename, false, &current.requested)?;
        let file = cap_file.into_std();
        ensure_regular_file(&file, &current.requested)?;
        let identity = identity_from_file(&file, &current.requested)?;
        if current.target_identity.as_ref() != Some(&identity) {
            return Err(GuardError::Changed {
                path: current.requested,
            });
        }
        Ok(file)
    }

    /// Read a file with a conservative default allocation bound.
    pub fn read_file(&self, requested: &Path) -> Result<Vec<u8>, GuardError> {
        self.read_file_bounded(requested, DEFAULT_READ_LIMIT)
    }

    pub fn read_file_bounded(
        &self,
        requested: &Path,
        max_bytes: u64,
    ) -> Result<Vec<u8>, GuardError> {
        let resolved = self.resolve_read(requested)?;
        let file = self.open_read(&resolved)?;
        read_bounded(file, requested, max_bytes)
    }

    /// Create an unguessable temporary file through the verified parent handle.
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
        if current.target_kind == Some(NodeKind::Directory) {
            return Err(GuardError::NotDirectory {
                path: current.requested,
            });
        }
        let (parent, final_name) = self.open_parent(&current.relative)?;
        let immediate_parent =
            current
                .final_path
                .parent()
                .ok_or_else(|| GuardError::InvalidPath {
                    path: current.requested.clone(),
                })?;
        if immediate_parent != current.canonical_parent {
            return Err(GuardError::NotFound {
                path: immediate_parent.to_path_buf(),
            });
        }

        for _ in 0..32 {
            let temp_name = OsString::from(format!(".orchester-write-{:016x}.tmp", random_token()));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            options.follow(FollowSymlinks::No);
            match parent.open_with(Path::new(&temp_name), &options) {
                Ok(file) => {
                    let display_path = immediate_parent.join(&temp_name);
                    return Ok(AtomicWriteTarget {
                        parent,
                        file: Some(file),
                        temp_name,
                        final_name,
                        display_path,
                        destination_path: current.final_path,
                        expected_target_identity: current.target_identity,
                        committed: false,
                    });
                }
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(map_io(
                        "create atomic write target",
                        &current.requested,
                        source,
                    ));
                }
            }
        }
        Err(GuardError::Io {
            operation: "create atomic write target",
            path: current.requested,
            source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary name collisions"),
        })
    }

    /// Replace a regular file through a same-directory capability rename.
    pub fn write_atomic(&self, requested: &Path, contents: &[u8]) -> Result<(), GuardError> {
        let resolved = self.resolve_write(requested)?;
        let mut target = self.atomic_write_target(&resolved)?;
        target
            .file_mut()
            .write_all(contents)
            .map_err(|source| map_io("write atomic target", requested, source))?;
        target.commit()
    }

    /// Rename one regular file to another without resolving either parent ambiently.
    pub fn rename(&self, from: &Path, to: &Path) -> Result<(), GuardError> {
        let from_relative = self.normalize_request(from, false)?;
        let to_relative = self.normalize_request(to, false)?;
        self.reject_protected(&from_relative, from)?;
        self.reject_protected(&to_relative, to)?;
        let (from_parent, from_name) = self.open_parent(&from_relative)?;
        let (to_parent, to_name) = self.open_parent(&to_relative)?;
        let source = open_file_nofollow(&from_parent, &from_name, false, from)?;
        let source_std = source
            .try_clone()
            .map_err(|source| map_io("clone rename source", from, source))?
            .into_std();
        ensure_regular_file(&source_std, from)?;
        let source_identity = identity_from_file(&source_std, from)?;

        match to_parent.symlink_metadata(Path::new(&to_name)) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(GuardError::LinkTraversal {
                    path: to.to_path_buf(),
                });
            }
            Ok(metadata) if metadata.is_dir() => {
                return Err(GuardError::NotDirectory {
                    path: to.to_path_buf(),
                });
            }
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(GuardError::Unsupported),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(map_io("inspect rename target", to, source)),
        }

        from_parent
            .rename(Path::new(&from_name), &to_parent, Path::new(&to_name))
            .map_err(|source| map_io("rename", from, source))?;
        let renamed = open_file_nofollow(&to_parent, &to_name, false, to)?.into_std();
        ensure_regular_file(&renamed, to)?;
        if identity_from_file(&renamed, to)? != source_identity {
            return Err(GuardError::Changed {
                path: to.to_path_buf(),
            });
        }
        Ok(())
    }

    pub(crate) fn directory_entries(
        &self,
        requested: &Path,
    ) -> Result<DirectoryEntries, GuardError> {
        let relative = self.normalize_request(requested, true)?;
        self.reject_protected(&relative, requested)?;
        let directory = self.open_directory(&relative, requested)?;
        let entries = directory
            .entries()
            .map_err(|source| map_io("read directory", requested, source))?;
        Ok(DirectoryEntries {
            entries,
            requested: requested.to_path_buf(),
        })
    }

    pub(crate) fn open_snapshot_file(&self, requested: &Path) -> Result<File, GuardError> {
        let resolved = self.resolve_read(requested)?;
        self.open_read(&resolved)
    }

    fn resolve_normalized(
        &self,
        requested: &Path,
        mode: ResolveMode,
    ) -> Result<ResolvedPath, GuardError> {
        let relative = self.normalize_request(requested, false)?;
        self.reject_protected(&relative, requested)?;
        let components = normal_components(&relative, requested)?;
        let (basename, parent_components) =
            components
                .split_last()
                .ok_or_else(|| GuardError::InvalidPath {
                    path: requested.to_path_buf(),
                })?;
        let mut directory = self.clone_root()?;
        let mut canonical_parent = self.root.clone();
        let mut missing_parent = false;

        for component in parent_components {
            if missing_parent {
                continue;
            }
            match directory.symlink_metadata(Path::new(component)) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(GuardError::LinkTraversal {
                        path: requested.to_path_buf(),
                    });
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(GuardError::NotDirectory {
                        path: requested.to_path_buf(),
                    });
                }
                Ok(_) => {
                    directory = directory
                        .open_dir_nofollow(Path::new(component))
                        .map_err(|source| map_io("open path component", requested, source))?;
                    canonical_parent.push(component);
                }
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    if mode == ResolveMode::Read {
                        return Err(GuardError::NotFound {
                            path: requested.to_path_buf(),
                        });
                    }
                    missing_parent = true;
                }
                Err(source) => return Err(map_io("inspect path component", requested, source)),
            }
        }

        let parent_identity = identity_from_dir(&directory, requested)?;
        let (target_identity, target_kind) = if missing_parent {
            (None, None)
        } else {
            match directory.symlink_metadata(Path::new(basename)) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(GuardError::LinkTraversal {
                        path: requested.to_path_buf(),
                    });
                }
                Ok(metadata) if metadata.is_dir() => {
                    let target = directory
                        .open_dir_nofollow(Path::new(basename))
                        .map_err(|source| map_io("open target directory", requested, source))?;
                    (
                        Some(identity_from_dir(&target, requested)?),
                        Some(NodeKind::Directory),
                    )
                }
                Ok(metadata) if metadata.is_file() => {
                    let target = open_file_nofollow(&directory, basename, false, requested)?;
                    let target = target.into_std();
                    ensure_regular_file(&target, requested)?;
                    (
                        Some(identity_from_file(&target, requested)?),
                        Some(NodeKind::File),
                    )
                }
                Ok(_) => return Err(GuardError::Unsupported),
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    if mode == ResolveMode::Read {
                        return Err(GuardError::NotFound {
                            path: requested.to_path_buf(),
                        });
                    }
                    (None, None)
                }
                Err(source) => return Err(map_io("inspect target", requested, source)),
            }
        };

        Ok(ResolvedPath {
            requested: requested.to_path_buf(),
            final_path: self.root.join(&relative),
            canonical_parent,
            relative,
            root_identity: self.identity.object.clone(),
            parent_identity,
            target_identity,
            target_kind,
            mode,
        })
    }

    fn open_parent(&self, relative: &Path) -> Result<(Dir, OsString), GuardError> {
        let components = normal_components(relative, relative)?;
        let (basename, parents) =
            components
                .split_last()
                .ok_or_else(|| GuardError::InvalidPath {
                    path: relative.to_path_buf(),
                })?;
        let mut directory = self.clone_root()?;
        for component in parents {
            directory = directory
                .open_dir_nofollow(Path::new(component))
                .map_err(|source| map_io("open parent directory", relative, source))?;
        }
        Ok((directory, basename.clone()))
    }

    fn open_directory(&self, relative: &Path, requested: &Path) -> Result<Dir, GuardError> {
        let mut directory = self.clone_root()?;
        if relative.as_os_str().is_empty() {
            return Ok(directory);
        }
        for component in normal_components(relative, requested)? {
            directory = directory
                .open_dir_nofollow(Path::new(&component))
                .map_err(|source| map_io("open directory", requested, source))?;
        }
        Ok(directory)
    }

    fn clone_root(&self) -> Result<Dir, GuardError> {
        self.root_dir.try_clone().map_err(|source| GuardError::Io {
            operation: "clone workspace root handle",
            path: self.root.clone(),
            source,
        })
    }

    fn normalize_request(&self, requested: &Path, allow_root: bool) -> Result<PathBuf, GuardError> {
        if contains_nul(requested) {
            return Err(GuardError::InvalidPath {
                path: requested.to_path_buf(),
            });
        }
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
                Component::Normal(value) => {
                    if invalid_platform_component(value) {
                        return Err(GuardError::InvalidPath {
                            path: requested.to_path_buf(),
                        });
                    }
                    normalized.push(value);
                }
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
        if normalized.as_os_str().is_empty() && !allow_root {
            return Err(GuardError::InvalidPath {
                path: requested.to_path_buf(),
            });
        }
        Ok(normalized)
    }

    fn reject_protected(&self, relative: &Path, requested: &Path) -> Result<(), GuardError> {
        if self
            .protected
            .iter()
            .any(|protected| path_starts_with(relative, protected))
        {
            Err(GuardError::Protected {
                path: requested.to_path_buf(),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
fn invalid_platform_component(component: &OsStr) -> bool {
    let value = component.to_string_lossy();
    // A colon in a non-prefix component selects an NTFS alternate data
    // stream. Such a handle still reports as a regular file and would bypass
    // the protected-file and content-boundary model.
    // Trailing dots/spaces and DOS device names are Win32 aliases whose
    // ambient rename semantics can disagree with capability-relative opens.
    if value.contains(':')
        || value.ends_with('.')
        || value.ends_with(' ')
        || value
            .chars()
            .any(|character| character.is_control() || r#"<>"|?*"#.contains(character))
    {
        return true;
    }

    let basename = value.split('.').next().unwrap_or_default();
    let uppercase = basename.to_ascii_uppercase();
    matches!(
        uppercase.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || uppercase
        .strip_prefix("COM")
        .or_else(|| uppercase.strip_prefix("LPT"))
        .is_some_and(|number| matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

#[cfg(not(windows))]
fn invalid_platform_component(_component: &OsStr) -> bool {
    false
}

/// A capability-relative temporary file which cleans itself up unless committed.
pub struct AtomicWriteTarget {
    parent: Dir,
    file: Option<CapFile>,
    temp_name: OsString,
    final_name: OsString,
    display_path: PathBuf,
    destination_path: PathBuf,
    expected_target_identity: Option<ObjectIdentity>,
    committed: bool,
}

impl AtomicWriteTarget {
    pub fn path(&self) -> &Path {
        &self.display_path
    }

    pub fn file(&self) -> &CapFile {
        self.file.as_ref().expect("atomic target file is open")
    }

    pub fn file_mut(&mut self) -> &mut CapFile {
        self.file.as_mut().expect("atomic target file is open")
    }

    pub fn commit(mut self) -> Result<(), GuardError> {
        let temp_identity = {
            let file = self.file.as_ref().ok_or_else(|| GuardError::Changed {
                path: self.destination_path.clone(),
            })?;
            let clone = file
                .try_clone()
                .map_err(|source| map_io("clone atomic target", &self.display_path, source))?
                .into_std();
            ensure_regular_file(&clone, &self.display_path)?;
            identity_from_file(&clone, &self.display_path)?
        };
        if let Some(file) = self.file.take() {
            file.sync_all()
                .map_err(|source| map_io("sync atomic target", &self.display_path, source))?;
            drop(file);
        }
        let current_target_identity =
            match self.parent.symlink_metadata(Path::new(&self.final_name)) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(GuardError::LinkTraversal {
                        path: self.destination_path.clone(),
                    });
                }
                Ok(metadata) if metadata.is_dir() => {
                    return Err(GuardError::NotDirectory {
                        path: self.destination_path.clone(),
                    });
                }
                Ok(metadata) if metadata.is_file() => {
                    let file = open_file_nofollow(
                        &self.parent,
                        &self.final_name,
                        false,
                        &self.destination_path,
                    )?
                    .into_std();
                    ensure_regular_file(&file, &self.destination_path)?;
                    Some(identity_from_file(&file, &self.destination_path)?)
                }
                Ok(_) => return Err(GuardError::Unsupported),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(source) => {
                    return Err(map_io(
                        "inspect atomic destination",
                        &self.destination_path,
                        source,
                    ));
                }
            };
        if current_target_identity != self.expected_target_identity {
            return Err(GuardError::Changed {
                path: self.destination_path.clone(),
            });
        }
        self.parent
            .rename(
                Path::new(&self.temp_name),
                &self.parent,
                Path::new(&self.final_name),
            )
            .map_err(|source| map_io("commit atomic write", &self.destination_path, source))?;
        let renamed = open_file_nofollow(
            &self.parent,
            &self.final_name,
            false,
            &self.destination_path,
        )?
        .into_std();
        ensure_regular_file(&renamed, &self.destination_path)?;
        if identity_from_file(&renamed, &self.destination_path)? != temp_identity {
            return Err(GuardError::Changed {
                path: self.destination_path.clone(),
            });
        }
        self.committed = true;
        Ok(())
    }
}

impl Drop for AtomicWriteTarget {
    fn drop(&mut self) {
        let _ = self.file.take();
        if !self.committed {
            let _ = self.parent.remove_file(Path::new(&self.temp_name));
        }
    }
}

/// A directory stream backed by an already-open capability.
///
/// Keeping the iterator instead of collecting names into a `Vec` means a
/// hostile directory cannot force an unbounded allocation before snapshot
/// limits are applied.  The snapshot layer stops consuming this stream as
/// soon as any structural limit is reached.
pub(crate) struct DirectoryEntries {
    entries: cap_std::fs::ReadDir,
    requested: PathBuf,
}

impl Iterator for DirectoryEntries {
    type Item = Result<OsString, GuardError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|entry| {
            entry
                .map(|entry| entry.file_name())
                .map_err(|source| map_io("read directory entry", &self.requested, source))
        })
    }
}

fn open_root_capability(root: &Path) -> Result<Dir, GuardError> {
    let mut anchor = PathBuf::new();
    let mut components = Vec::new();
    for component in root.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(GuardError::InvalidPath {
                    path: root.to_path_buf(),
                })
            }
        }
    }
    if anchor.as_os_str().is_empty() || !root.is_absolute() {
        return Err(GuardError::InvalidPath {
            path: root.to_path_buf(),
        });
    }
    let mut directory = Dir::open_ambient_dir(&anchor, cap_std::ambient_authority())
        .map_err(|source| map_io("open filesystem root", root, source))?;
    for component in components {
        directory = directory
            .open_dir_nofollow(Path::new(&component))
            .map_err(|source| map_io("open workspace root component", root, source))?;
    }
    Ok(directory)
}

fn absolute_lexical_path(requested: &Path) -> Result<PathBuf, GuardError> {
    let absolute = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| map_io("read current directory", requested, source))?
            .join(requested)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(value) => {
                if invalid_platform_component(value) {
                    return Err(GuardError::InvalidPath {
                        path: requested.to_path_buf(),
                    });
                }
                normalized.push(value);
            }
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(GuardError::InvalidPath {
                        path: requested.to_path_buf(),
                    });
                }
            }
        }
    }
    if !normalized.is_absolute() {
        return Err(GuardError::InvalidPath {
            path: requested.to_path_buf(),
        });
    }
    Ok(normalized)
}

fn canonical_display_path(
    lexical_root: &Path,
    expected_identity: &ObjectIdentity,
) -> Result<PathBuf, GuardError> {
    let candidate = fs::canonicalize(lexical_root)
        .map_err(|source| map_io("canonicalize workspace display path", lexical_root, source))?;
    let candidate_dir = open_root_capability(&candidate)?;
    let candidate_identity = identity_from_dir(&candidate_dir, &candidate)?;
    if &candidate_identity != expected_identity {
        return Err(GuardError::Changed {
            path: lexical_root.to_path_buf(),
        });
    }
    Ok(candidate)
}

fn open_file_nofollow(
    parent: &Dir,
    basename: &OsStr,
    write: bool,
    requested: &Path,
) -> Result<CapFile, GuardError> {
    let mut options = OpenOptions::new();
    if write {
        options.write(true);
    } else {
        options.read(true);
    }
    // Opening a FIFO in blocking mode would let an attacker stall the
    // harness before the post-open regular-file check can reject it.
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK);
    options.follow(FollowSymlinks::No);
    parent
        .open_with(Path::new(basename), &options)
        .map_err(|source| map_io("open file without following links", requested, source))
}

fn ensure_regular_file(file: &File, path: &Path) -> Result<(), GuardError> {
    let metadata = file
        .metadata()
        .map_err(|source| map_io("inspect opened file", path, source))?;
    if !metadata.is_file() {
        return Err(GuardError::Unsupported);
    }
    Ok(())
}

fn read_bounded(mut file: File, requested: &Path, max_bytes: u64) -> Result<Vec<u8>, GuardError> {
    let limit = max_bytes.saturating_add(1);
    let mut content = Vec::with_capacity(max_bytes.min(64 * 1024) as usize);
    Read::by_ref(&mut file)
        .take(limit)
        .read_to_end(&mut content)
        .map_err(|source| map_io("read file", requested, source))?;
    if content.len() as u64 > max_bytes {
        return Err(GuardError::LimitExceeded {
            path: requested.to_path_buf(),
            limit: max_bytes,
        });
    }
    Ok(content)
}

fn identity_from_dir(directory: &Dir, path: &Path) -> Result<ObjectIdentity, GuardError> {
    let file = directory
        .try_clone()
        .map_err(|source| map_io("clone directory handle", path, source))?
        .into_std_file();
    identity_from_file(&file, path)
}

#[cfg(unix)]
fn identity_from_file(file: &File, path: &Path) -> Result<ObjectIdentity, GuardError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .map_err(|source| map_io("inspect open handle", path, source))?;
    if metadata.file_type().is_symlink() {
        return Err(GuardError::LinkTraversal {
            path: path.to_path_buf(),
        });
    }
    Ok(ObjectIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn identity_from_file(file: &File, path: &Path) -> Result<ObjectIdentity, GuardError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut information) };
    if succeeded == 0 {
        return Err(map_io(
            "query open handle identity",
            path,
            io::Error::last_os_error(),
        ));
    }
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(GuardError::LinkTraversal {
            path: path.to_path_buf(),
        });
    }
    Ok(ObjectIdentity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64,
    })
}

#[cfg(not(any(unix, windows)))]
fn identity_from_file(_file: &File, _path: &Path) -> Result<ObjectIdentity, GuardError> {
    Err(GuardError::Unsupported)
}

fn normal_components(path: &Path, requested: &Path) -> Result<Vec<OsString>, GuardError> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_os_string()),
            _ => Err(GuardError::InvalidPath {
                path: requested.to_path_buf(),
            }),
        })
        .collect()
}

fn resolve_existing_path_no_links(path: &Path) -> Result<PathBuf, GuardError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match component {
            Component::Prefix(_) | Component::CurDir => continue,
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
    fs::canonicalize(path).map_err(|source| map_io("canonicalize", path, source))
}

fn symlink_metadata(path: &Path) -> Result<Metadata, GuardError> {
    fs::symlink_metadata(path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => GuardError::NotFound {
            path: path.to_path_buf(),
        },
        _ => map_io("inspect path", path, source),
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

fn map_io(operation: &'static str, path: &Path, source: io::Error) -> GuardError {
    if is_link_error(&source) {
        return GuardError::LinkTraversal {
            path: path.to_path_buf(),
        };
    }
    match source.kind() {
        io::ErrorKind::NotFound => GuardError::NotFound {
            path: path.to_path_buf(),
        },
        io::ErrorKind::NotADirectory | io::ErrorKind::IsADirectory => GuardError::NotDirectory {
            path: path.to_path_buf(),
        },
        io::ErrorKind::Unsupported => GuardError::Unsupported,
        _ => GuardError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        },
    }
}

fn is_link_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    if error.raw_os_error() == Some(40) {
        return true;
    }
    #[cfg(windows)]
    if matches!(error.raw_os_error(), Some(681 | 1920)) {
        return true;
    }
    false
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
        .map(|part| windows_component_key(&part))
        .collect();
    let prefix_parts: Vec<_> = prefix
        .components()
        .map(|part| windows_component_key(&part))
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
            *unit += (b'a' - b'A') as u16;
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
