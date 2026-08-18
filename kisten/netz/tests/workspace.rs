use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{select_workspace, WorkspaceSelectionError};

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("orchester-netz-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("temporary directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn selection_canonicalizes_directory_and_keeps_the_same_home() {
    let root = TempDirectory::new("directory");
    let paths = OrchesterPaths::new("user-home", "old-workspace");
    let selected = select_workspace(&paths, root.path()).expect("workspace selection");

    assert_eq!(selected.home(), paths.home());
    assert_eq!(
        selected.workspace(),
        fs::canonicalize(root.path()).expect("canonical workspace")
    );
}

#[test]
fn selection_rejects_a_missing_workspace_without_echoing_the_path() {
    let root = TempDirectory::new("missing");
    let paths = OrchesterPaths::new("user-home", "old-workspace");
    let missing = root.path().join("not-created");

    assert_eq!(
        select_workspace(&paths, &missing),
        Err(WorkspaceSelectionError::Unavailable)
    );
}

#[test]
fn selection_rejects_a_regular_file_as_workspace() {
    let root = TempDirectory::new("file");
    let file = root.path().join("not-a-directory");
    fs::write(&file, b"file").expect("temporary file");
    let paths = OrchesterPaths::new("user-home", "old-workspace");

    assert_eq!(
        select_workspace(&paths, &file),
        Err(WorkspaceSelectionError::NotDirectory)
    );
}

#[test]
fn selection_rejects_an_empty_workspace_path() {
    let paths = OrchesterPaths::new("user-home", "old-workspace");

    assert_eq!(
        select_workspace(&paths, ""),
        Err(WorkspaceSelectionError::Invalid)
    );
}
