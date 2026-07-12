//! Governance primitives shared by the self-agent's file and process tools.

mod path_guard;
mod workspace_lock;

pub use path_guard::{
    AtomicWriteTarget, FilesystemResolver, GuardError, GuardErrorKind, PathResolver, ResolvedPath,
    WorkspaceGuard,
};
pub use workspace_lock::{MutationLease, WorkspaceLocks};
pub mod command;
pub mod policy;

pub use crate::harness::run_store::EffectClass;
pub use command::{classify_command, CommandCategory, CommandIntent, CommandParseError};
pub use policy::{
    Decision, PolicyEngine, PolicyError, PolicyInput, PolicyResult, PolicyRule, Risk, RiskLevel,
};
