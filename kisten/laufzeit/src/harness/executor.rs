//! Permit-bound dispatch for built-in tool implementations.

use std::fmt;
use std::path::Path;

use orchester_protokoll::AgentAction;
use thiserror::Error;

use super::barrier::StartedTool;
use super::files::{
    FileToolError, FileToolLimits, FileTools, ListResult, ReadResult, SearchResult,
};

pub enum ToolExecution {
    Listed(ListResult),
    Read(ReadResult),
    Searched(SearchResult),
}

impl fmt::Debug for ToolExecution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
}

pub struct ToolExecutor {
    files: FileTools,
}

impl ToolExecutor {
    pub fn new(
        workspace_root: impl AsRef<Path>,
        file_limits: FileToolLimits,
    ) -> Result<Self, ToolExecutorError> {
        Ok(Self {
            files: FileTools::new(workspace_root, file_limits)?,
        })
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
