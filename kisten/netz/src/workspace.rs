use std::fs;
use std::path::Path;

use orchester_anwendung::OrchesterPaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceSelectionError {
    Invalid,
    Unavailable,
    NotDirectory,
}

pub fn select_workspace(
    paths: &OrchesterPaths,
    candidate: impl AsRef<Path>,
) -> Result<OrchesterPaths, WorkspaceSelectionError> {
    let candidate = candidate.as_ref();
    if candidate.as_os_str().is_empty() {
        return Err(WorkspaceSelectionError::Invalid);
    }

    let canonical =
        fs::canonicalize(candidate).map_err(|_| WorkspaceSelectionError::Unavailable)?;
    if !canonical.is_dir() {
        return Err(WorkspaceSelectionError::NotDirectory);
    }

    Ok(paths.with_workspace(canonical))
}
