use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{bootstrap_response, ServerContext, ServerControl, ServerState};

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir()
            .join(format!("orchester-bootstrap-private-{nonce}"))
            .join("visible-project");
        fs::create_dir_all(&path).expect("temporary workspace");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

#[test]
fn bootstrap_projects_running_state_and_workspace_basename_without_paths() {
    let workspace = TempDirectory::new();
    let home = workspace.path().parent().unwrap().join("private-home");
    let paths = OrchesterPaths::new(&home, workspace.path());
    let control = ServerControl::new();
    control.start().expect("running server");
    let context = ServerContext::new(Some(paths), control);

    let response = bootstrap_response(&context);
    assert_eq!(response.schema_version, 1);
    assert_eq!(response.service_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(response.server_state, ServerState::Running);
    assert!(response.workspace.selected);
    assert_eq!(response.workspace.name.as_deref(), Some("visible-project"));

    let json = serde_json::to_string(&response).expect("bootstrap JSON");
    assert!(!json.contains(home.to_string_lossy().as_ref()));
    assert!(!json.contains(workspace.path().to_string_lossy().as_ref()));
}

#[test]
fn bootstrap_supports_an_unselected_workspace() {
    let context = ServerContext::new(None, ServerControl::new());

    let response = bootstrap_response(&context);

    assert_eq!(response.server_state, ServerState::Starting);
    assert!(!response.workspace.selected);
    assert_eq!(response.workspace.name, None);
}
