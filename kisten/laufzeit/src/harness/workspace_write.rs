//! Permit-bound atomic writes inside a capability workspace.

use std::fmt;
use std::io::Write;
use std::path::Path;

use orchester_modell::{MAX_CONTENT_BYTES, MAX_PATH_BYTES};
use orchester_protokoll::AgentAction;
use thiserror::Error;

use super::barrier::StartedTool;
use super::governance::{GuardError, GuardErrorKind, WorkspaceGuard, WorkspaceLocks};

const MAX_CONFIGURED_CONTENT_BYTES: u64 = MAX_CONTENT_BYTES as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceWriteLimits {
    pub max_content_bytes: u64,
}

impl Default for WorkspaceWriteLimits {
    fn default() -> Self {
        Self {
            max_content_bytes: MAX_CONFIGURED_CONTENT_BYTES,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceWriteError {
    #[error("workspace path operation was rejected: {0:?}")]
    Guard(GuardErrorKind),
    #[error("file content exceeds the configured write limit")]
    LimitExceeded,
    #[error("workspace write input is invalid")]
    InvalidInput,
    #[error("workspace filesystem operation failed")]
    Io,
    #[error("started tool is not a write-file action")]
    WrongAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceWriteResult {
    pub bytes_written: u64,
}

pub struct GovernedWorkspaceWriter {
    workspace: WorkspaceGuard,
    limits: WorkspaceWriteLimits,
    locks: WorkspaceLocks,
    explicit_outside_access: bool,
}

impl fmt::Debug for GovernedWorkspaceWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedWorkspaceWriter")
            .field("workspace", &self.workspace)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl GovernedWorkspaceWriter {
    pub fn new(
        root: impl AsRef<Path>,
        limits: WorkspaceWriteLimits,
        locks: WorkspaceLocks,
    ) -> Result<Self, WorkspaceWriteError> {
        if limits.max_content_bytes == 0 || limits.max_content_bytes > MAX_CONFIGURED_CONTENT_BYTES
        {
            return Err(WorkspaceWriteError::InvalidInput);
        }
        let workspace = WorkspaceGuard::new(root.as_ref()).map_err(map_guard)?;
        Ok(Self {
            workspace,
            limits,
            locks,
            explicit_outside_access: false,
        })
    }

    pub fn with_explicit_outside_access(mut self, enabled: bool) -> Self {
        self.explicit_outside_access = enabled;
        self
    }

    pub async fn execute(
        &self,
        started: StartedTool,
    ) -> Result<WorkspaceWriteResult, WorkspaceWriteError> {
        let AgentAction::WriteFile { path, content } = started.into_action() else {
            return Err(WorkspaceWriteError::WrongAction);
        };
        if path.is_empty() || path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
            return Err(WorkspaceWriteError::InvalidInput);
        }
        let bytes_written =
            u64::try_from(content.len()).map_err(|_| WorkspaceWriteError::LimitExceeded)?;
        if bytes_written > self.limits.max_content_bytes {
            return Err(WorkspaceWriteError::LimitExceeded);
        }

        let outside = if self.explicit_outside_access {
            self.workspace
                .scope_for_explicit_path(Path::new(&path))
                .map_err(map_guard)?
        } else {
            None
        };
        let (workspace, requested) = outside
            .as_ref()
            .map(|(guard, path)| (guard, path.as_path()))
            .unwrap_or((&self.workspace, Path::new(&path)));
        let lease = self
            .locks
            .resolve_mutation(workspace, requested)
            .await
            .map_err(map_guard)?;
        let mut target = workspace
            .atomic_write_target(lease.resolved())
            .map_err(map_guard)?;
        target
            .file_mut()
            .write_all(content.as_bytes())
            .map_err(|_| WorkspaceWriteError::Io)?;
        target.commit().map_err(map_guard)?;

        Ok(WorkspaceWriteResult { bytes_written })
    }
}

fn map_guard(error: GuardError) -> WorkspaceWriteError {
    match error {
        GuardError::LimitExceeded { .. } => WorkspaceWriteError::LimitExceeded,
        GuardError::Io { .. } => WorkspaceWriteError::Io,
        other => WorkspaceWriteError::Guard(other.kind()),
    }
}
