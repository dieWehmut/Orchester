use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::OrchesterPaths;
use orchester_laufzeit::{SessionRecord, SessionStore};
use orchester_netz::{ServerContext, ServerControl};
use orchester_protokoll::{Outcome, Usage};

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

#[test]
fn server_context_binds_delegate_history_to_the_selected_paths() {
    let workspace = TempWorkspace::new();
    let paths = workspace.paths();
    SessionStore::new(paths.session_log())
        .append(&SessionRecord {
            recorded_at_unix: 1_800_000_000,
            agent: "codex".to_owned(),
            session_id: Some("native-private".to_owned()),
            prompt: "context-bound history".to_owned(),
            cwd: paths.workspace().to_path_buf(),
            model: Some("gpt-5.6".to_owned()),
            outcome: Outcome::Success,
            final_text: "done".to_owned(),
            usage: Usage::default(),
        })
        .expect("session record");

    let selected = ServerContext::new(Some(paths), ServerControl::new());
    let page = selected
        .session_history()
        .expect("selected history")
        .page(None, 10)
        .expect("history page");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "context-bound history");
    assert!(ServerContext::new(None, ServerControl::new())
        .session_history()
        .is_none());
}
