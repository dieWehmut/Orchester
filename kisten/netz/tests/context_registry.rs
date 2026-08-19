use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{ServerContext, ServerControl};

struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let workspace = std::env::temp_dir().join(format!("orchester-netz-registry-{nonce}"));
        fs::create_dir_all(workspace.join("manifeste")).expect("workspace");
        Self(workspace)
    }

    fn paths(&self) -> OrchesterPaths {
        OrchesterPaths::new(self.0.join("home"), &self.0)
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn server_context_discovers_project_manifests_into_its_registry() {
    let workspace = TempWorkspace::new();
    fs::write(
        workspace.0.join("manifeste/custom.toml"),
        r#"
name = "custom"
command = "custom-agent"
args = ["{prompt}"]
kinds = ["research"]
supports_resume = false
streaming = false

[parse]
discriminator = "type"
"#,
    )
    .expect("custom manifest");

    let context = ServerContext::new(Some(workspace.paths()), ServerControl::new());

    assert!(context.registry().get("custom").is_some());
    assert_eq!(context.registry().len(), 5);
}

#[test]
fn server_context_binds_a_model_host_only_for_a_selected_workspace() {
    let workspace = TempWorkspace::new();
    let paths = workspace.paths();

    let selected = ServerContext::new(Some(paths.clone()), ServerControl::new());
    let unselected = ServerContext::new(None, ServerControl::new());

    assert!(selected.model_host().is_some());
    assert!(unselected.model_host().is_none());
    let debug = format!("{selected:?}");
    assert!(!debug.contains(paths.home().to_string_lossy().as_ref()));
    assert!(!debug.contains(paths.workspace().to_string_lossy().as_ref()));
}
