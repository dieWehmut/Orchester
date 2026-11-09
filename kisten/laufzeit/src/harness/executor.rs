//! Permit-bound dispatch for built-in tool implementations.

use super::governance::WorkspaceLocks;
use super::process_runtime::{GovernedProcessRunner, ProcessLimits};
use super::workspace_patch::{GovernedWorkspacePatcher, PatchLimits};
use super::workspace_write::{GovernedWorkspaceWriter, WorkspaceWriteLimits};
use std::fmt;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use orchester_protokoll::AgentAction;
use thiserror::Error;

use super::barrier::StartedTool;
use super::files::{
    FileToolError, FileToolLimits, FileTools, ListResult, ReadResult, SearchResult,
};

pub enum ToolExecution {
    Written(u64),
    Patched(usize, u64),
    Command(super::process_runtime::ProcessRunResult),
    Listed(ListResult),
    Read(ReadResult),
    Searched(SearchResult),
}

impl fmt::Debug for ToolExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Written(bytes) => formatter.debug_tuple("Written").field(bytes).finish(),
            Self::Patched(files, bytes) => formatter
                .debug_tuple("Patched")
                .field(files)
                .field(bytes)
                .finish(),
            Self::Command(result) => formatter.debug_tuple("Command").field(result).finish(),
            Self::Listed(result) => formatter
                .debug_struct("Listed")
                .field("entries", &result.entries.len())
                .finish(),
            Self::Read(result) => formatter.debug_tuple("Read").field(result).finish(),
            Self::Searched(result) => formatter
                .debug_struct("Searched")
                .field("matches", &result.matches.len())
                .field("skipped_oversized_files", &result.skipped_oversized_files)
                .finish(),
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutorError {
    #[error(transparent)]
    File(#[from] FileToolError),
    #[error("started tool action is not supported by this executor")]
    UnsupportedAction,
    #[error("tool execution was cancelled")]
    Cancelled,
    #[error("governed tool execution failed")]
    Execution,
}

pub struct ToolExecutor {
    files: FileTools,
    root: PathBuf,
    mode: Option<super::config::PermissionMode>,
    file_limits: FileToolLimits,
    locks: WorkspaceLocks,
}

impl ToolExecutor {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        file_limits: FileToolLimits,
    ) -> Result<Self, ToolExecutorError> {
        let root = workspace_root.as_ref().to_path_buf();
        Ok(Self {
            root,
            mode: None,
            file_limits,
            locks: WorkspaceLocks::default(),
            files: FileTools::new(workspace_root, file_limits)?,
        })
    }

    pub fn with_permission_mode(mut self, mode: Option<super::config::PermissionMode>) -> Self {
        self.mode = mode;
        self.files = self
            .files
            .with_explicit_outside_access(mode == Some(super::config::PermissionMode::FullAccess));
        self
    }

    /// Called only after the durable approval barrier consumes an exact action grant.
    pub(crate) async fn execute_approved_async(
        &self,
        started: StartedTool,
        cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolExecutorError> {
        let scoped = Self {
            files: FileTools::new(&self.root, self.file_limits)?.with_explicit_outside_access(true),
            root: self.root.clone(),
            mode: Some(super::config::PermissionMode::FullAccess),
            file_limits: self.file_limits,
            locks: self.locks.clone(),
        };
        scoped.execute_async(started, cancel).await
    }

    pub async fn execute_async(
        &self,
        started: StartedTool,
        cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolExecutorError> {
        if cancel.is_cancelled() {
            return Err(ToolExecutorError::Cancelled);
        }
        if matches!(
            started.action(),
            AgentAction::WriteFile { .. } | AgentAction::ApplyPatch { .. }
        ) {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(ToolExecutorError::Cancelled),
                result = self.execute_inner(started, cancel.clone()) => result,
            }
        } else {
            self.execute_inner(started, cancel).await
        }
    }

    async fn execute_inner(
        &self,
        started: StartedTool,
        cancel: CancellationToken,
    ) -> Result<ToolExecution, ToolExecutorError> {
        match started.action() {
            AgentAction::WriteFile { .. } => {
                let writer = GovernedWorkspaceWriter::new(
                    &self.root,
                    WorkspaceWriteLimits::default(),
                    self.locks.clone(),
                )
                .map_err(|_| ToolExecutorError::Execution)?;
                writer
                    .with_explicit_outside_access(
                        self.mode == Some(super::config::PermissionMode::FullAccess),
                    )
                    .execute(started)
                    .await
                    .map(|r| ToolExecution::Written(r.bytes_written))
                    .map_err(|_| ToolExecutorError::Execution)
            }
            AgentAction::ApplyPatch { .. } => {
                let patcher = GovernedWorkspacePatcher::new(
                    &self.root,
                    PatchLimits::default(),
                    self.locks.clone(),
                )
                .map_err(|_| ToolExecutorError::Execution)?;
                patcher
                    .with_explicit_outside_access(
                        self.mode == Some(super::config::PermissionMode::FullAccess),
                    )
                    .execute(started)
                    .await
                    .map(|r| ToolExecution::Patched(r.files_changed, r.bytes_written))
                    .map_err(|_| ToolExecutorError::Execution)
            }
            AgentAction::RunCommand { .. } => {
                let runner = GovernedProcessRunner::new(&self.root, ProcessLimits::default())
                    .map_err(|_| ToolExecutorError::Execution)?;
                runner
                    .with_permission_mode(self.mode)
                    .execute(started, cancel)
                    .await
                    .map(ToolExecution::Command)
                    .map_err(|_| ToolExecutorError::Execution)
            }
            _ => self.execute(started),
        }
    }

    pub fn execute(&self, started: StartedTool) -> Result<ToolExecution, ToolExecutorError> {
        match started.into_action() {
            AgentAction::ListFiles { path, depth } => self
                .files
                .list_files(Path::new(&path), depth)
                .map(ToolExecution::Listed)
                .map_err(Into::into),
            AgentAction::SearchText { path, query } => self
                .files
                .search_text(Path::new(&path), &query)
                .map(ToolExecution::Searched)
                .map_err(Into::into),
            AgentAction::ReadFile {
                path,
                start_line,
                end_line,
            } => self
                .files
                .read_file(Path::new(&path), start_line, end_line)
                .map(ToolExecution::Read)
                .map_err(Into::into),
            _ => Err(ToolExecutorError::UnsupportedAction),
        }
    }
}

impl fmt::Debug for ToolExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolExecutor")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use orchester_protokoll::{ActionId, EventId, HarnessEvent, HarnessEventKind, RunId, StepId};

    #[tokio::test]
    async fn cancellation_while_waiting_for_mutation_lock_never_writes_or_patches() {
        let root = std::env::temp_dir().join(format!("executor-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("value.txt"), "original\n").unwrap();
        let executor = ToolExecutor::new(&root, FileToolLimits::default()).unwrap();
        let guard = super::super::governance::WorkspaceGuard::new(&root).unwrap();
        for action in [
            AgentAction::WriteFile { path: "value.txt".into(), content: "changed".into() },
            AgentAction::ApplyPatch { patch: "*** Begin Patch\n*** Update File: value.txt\n@@\n-original\n+changed\n*** End Patch\n".into() },
        ] {
            let lock = executor.locks.mutate(&guard).await;
            let token = CancellationToken::new();
            let started = StartedTool::new(HarnessEvent::new_for_test(EventId::from("event"), RunId::from("run"), StepId::from("step"), 1, HarnessEventKind::ToolStarted { action_id: ActionId::from("action") }), action);
            let work = executor.execute_async(started, token.clone());
            tokio::pin!(work);
            tokio::select! {
                result = &mut work => panic!("tool bypassed lock: {result:?}"),
                _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {},
            }
            token.cancel();
            assert!(matches!(work.await, Err(ToolExecutorError::Cancelled)));
            drop(lock);
            assert_eq!(std::fs::read_to_string(root.join("value.txt")).unwrap(), "original\n");
        }
        drop((executor, guard));
        std::fs::remove_dir_all(root).unwrap();
    }
}
