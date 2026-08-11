//! The read-only projection behind `/config`.
//!
//! Every other self-agent command funnels through `load_effective` and
//! propagates its error, so a configuration Orchester refuses to read leaves
//! the human with a bare sentence and no path. This projection exists to break
//! that: it is deliberately infallible, carrying a rejection as data so the
//! rendered view can still name the file, the credential wiring, and the
//! permission diagnostics that explain the refusal.

use std::path::{Path, PathBuf};

use crate::harness::config::{ConfigLoader, PermissionDiagnostic, RedactedConfig, PROJECT_CONFIG};
use crate::harness::credentials::CredentialStore;

/// Where configuration came from, what it resolved to, and why not.
#[derive(Debug, Clone)]
pub struct SelfAgentConfigView {
    /// The resolved user config path. `ORCHESTER_HOME` moves this, so it is
    /// reported rather than described.
    pub user_path: PathBuf,
    pub user_present: bool,
    /// The project layer consulted for this workspace, present or not.
    pub project_path: PathBuf,
    pub project_present: bool,
    pub resolution: ConfigResolution,
    /// Permission findings for the config file and its directory.
    pub diagnostics: Vec<PermissionDiagnostic>,
}

/// The outcome of reading configuration. `Rejected` is a value rather than an
/// error so `/config` keeps working precisely when configuration does not.
#[derive(Debug, Clone)]
pub enum ConfigResolution {
    Loaded(RedactedConfig),
    Rejected { reason: String },
}

impl ConfigResolution {
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }
}

/// Project the effective configuration for display. Never fails: a config that
/// cannot be read is reported through [`ConfigResolution::Rejected`].
pub fn load_self_agent_config_view<S: CredentialStore + ?Sized>(
    loader: &ConfigLoader,
    store: &S,
    workspace: &Path,
) -> SelfAgentConfigView {
    let user_path = loader.user_path().to_path_buf();
    // `load_effective` derives the project layer from the workspace when the
    // loader names none, so the view reports the path that was actually read.
    let project_path = loader
        .project_path()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| workspace.join(PROJECT_CONFIG));

    let resolution = match loader.load_effective(workspace) {
        // The credential-aware view reports each reference as present or
        // absent; the value itself never enters the tree.
        Ok(config) => match config.redacted_with_credentials(store) {
            Ok(redacted) => ConfigResolution::Loaded(redacted),
            Err(error) => ConfigResolution::Rejected {
                reason: error.to_string(),
            },
        },
        Err(error) => ConfigResolution::Rejected {
            reason: error.to_string(),
        },
    };

    SelfAgentConfigView {
        user_present: user_path.exists(),
        project_present: project_path.exists(),
        user_path,
        project_path,
        resolution,
        // Reported even when the config loaded: a readable file with loose
        // permissions is exactly the state worth naming before it is exploited.
        diagnostics: loader.doctor(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::harness::credentials::{CredentialStore, InMemoryCredentialStore};
    use crate::harness::private_fs::{create_private_dir_all, write_private_file};
    use secrecy::SecretString;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    /// A scratch directory that removes itself, so a failing assertion cannot
    /// leave a config behind for the next run to read.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-config-view-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            // The loader refuses a config anyone else could write, so the
            // scratch home is created the same way production creates it.
            create_private_dir_all(&path).expect("create private scratch directory");
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn view_of(root: &TempDir, source: &str) -> SelfAgentConfigView {
        let config_path = root.join("orchester.jsonc");
        write_private_file(&config_path, source).expect("write private config");
        let loader = ConfigLoader::test().with_user_path(&config_path);
        load_self_agent_config_view(&loader, &InMemoryCredentialStore::default(), &root.0)
    }

    /// The reason this projection exists. Every other command propagates the
    /// load error and prints one unactionable sentence; `/config` must still
    /// name the file it refused.
    #[test]
    fn a_configuration_that_cannot_be_read_is_carried_as_data_not_propagated() {
        let root = TempDir::new();

        let view = view_of(&root, "{ this is not configuration }");

        assert!(!view.resolution.is_loaded());
        assert_eq!(view.user_path, root.join("orchester.jsonc"));
        assert!(view.user_present);
    }

    #[test]
    fn a_rejection_is_accompanied_by_permission_findings_for_the_file_and_its_directory() {
        let root = TempDir::new();

        let view = view_of(&root, "{ not configuration }");

        // Naming the reason is not enough: the human needs to know which paths
        // were inspected and what was expected of them.
        let inspected: Vec<&PathBuf> = view.diagnostics.iter().map(|entry| &entry.path).collect();
        assert!(inspected.contains(&&root.0));
        assert!(inspected.contains(&&root.join("orchester.jsonc")));
        assert!(view
            .diagnostics
            .iter()
            .all(|entry| !entry.expected.is_empty()));
    }

    #[test]
    fn a_readable_configuration_resolves_to_a_redacted_view() {
        let root = TempDir::new();

        let view = view_of(
            &root,
            r#"{ "version": 1, "model": "gpt-5.6-sol", "model_provider": "OpenAI" }"#,
        );

        let ConfigResolution::Loaded(redacted) = &view.resolution else {
            panic!("expected a loaded configuration, got {:?}", view.resolution);
        };
        assert!(redacted.json().contains("gpt-5.6-sol"));
    }

    /// The whole point of the redacted view: a stored key must be reported as
    /// reachable without the value appearing anywhere in the projection.
    #[test]
    fn a_resolved_provider_key_is_reported_as_present_without_revealing_it() {
        let root = TempDir::new();
        let store = InMemoryCredentialStore::default();
        store
            .set(
                "OpenAI",
                SecretString::new("sk-live-supersecret".to_owned().into_boxed_str()),
            )
            .expect("store key");
        let config_path = root.join("orchester.jsonc");
        write_private_file(
            &config_path,
            r#"{
                "model_provider": "OpenAI",
                "model_providers": {
                    "OpenAI": {
                        "base_url": "https://agentrouter.org",
                        "api_key": "${secret:OpenAI}"
                    }
                }
            }"#,
        )
        .expect("write private config");
        let loader = ConfigLoader::test().with_user_path(&config_path);

        let view = load_self_agent_config_view(&loader, &store, &root.0);

        let ConfigResolution::Loaded(redacted) = &view.resolution else {
            panic!("expected a loaded configuration, got {:?}", view.resolution);
        };
        let json = redacted.json();
        assert!(!json.contains("sk-live-supersecret"));
        assert!(json.contains("https://agentrouter.org"));
    }

    /// An absent config is a legitimate state, not a failure: the loader falls
    /// back to defaults. `/config` must say so rather than implying a file.
    #[test]
    fn an_absent_configuration_loads_defaults_and_is_reported_as_absent() {
        let root = TempDir::new();
        let loader = ConfigLoader::test().with_user_path(root.join("orchester.jsonc"));

        let view =
            load_self_agent_config_view(&loader, &InMemoryCredentialStore::default(), &root.0);

        assert!(view.resolution.is_loaded());
        assert!(!view.user_present);
        assert!(!view.project_present);
    }

    /// `load_effective` derives the project layer from the workspace when the
    /// loader names none, so the view must report the same path the loader read.
    #[test]
    fn the_project_layer_is_reported_at_the_path_the_loader_consulted() {
        let root = TempDir::new();
        let loader = ConfigLoader::test().with_user_path(root.join("orchester.jsonc"));

        let view =
            load_self_agent_config_view(&loader, &InMemoryCredentialStore::default(), &root.0);

        assert_eq!(view.project_path, root.0.join(PROJECT_CONFIG));
    }
}
