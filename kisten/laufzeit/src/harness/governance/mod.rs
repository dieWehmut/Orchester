//! Governance primitives shared by the self-agent's file and process tools.

mod path_guard;
mod workspace_lock;

pub use path_guard::{
    AtomicWriteTarget, FilesystemResolver, GuardError, GuardErrorKind, PathResolver, ResolvedPath,
    WorkspaceGuard,
};
pub use workspace_lock::{MutationLease, WorkspaceLocks};
