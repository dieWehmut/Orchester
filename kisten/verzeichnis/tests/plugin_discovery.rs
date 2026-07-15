use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_verzeichnis::{
    PluginOrigin, PluginRoot, PluginRootError, Registry, standard_plugin_roots,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "orchester-plugin-discovery-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn plugin_scope(&self) -> PathBuf {
        self.0.join("node_modules/@orchester")
    }

    fn install_repository_plugin(&self, name: &str) {
        self.install_repository_plugin_at(&self.plugin_scope(), name);
    }

    fn install_repository_plugin_at(&self, scope: &Path, name: &str) {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("npm/plugins")
            .join(name);
        let destination = scope.join(name);
        fs::create_dir_all(destination.join("manifests")).unwrap();
        for relative in [
            "package.json",
            "orchester-plugin.json",
            "manifests/claude.toml",
        ] {
            fs::copy(source.join(relative), destination.join(relative)).unwrap();
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn missing_manifests(root: &Path) -> PathBuf {
    root.join("missing-manifests")
}

#[test]
fn validated_project_plugin_is_registered_with_redacted_source_metadata() {
    let fixture = Fixture::new();
    fixture.install_repository_plugin("claude");

    let registry = Registry::discover_with_plugin_roots(
        missing_manifests(&fixture.0),
        [PluginRoot::project(fixture.plugin_scope())],
    );
    let plugins = registry.plugins();

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info().name(), "claude");
    assert_eq!(plugins[0].origin(), PluginOrigin::Project);
    assert!(registry.get("claude").is_some());
    let debug = format!("{:?}", plugins[0]);
    assert!(!debug.contains(fixture.0.to_string_lossy().as_ref()));
}

#[test]
fn invalid_packages_are_skipped_without_removing_builtin_fallbacks() {
    let fixture = Fixture::new();
    fixture.install_repository_plugin("claude");
    fs::write(
        fixture.plugin_scope().join("claude/install.js"),
        "do-not-run\n",
    )
    .unwrap();

    let registry = Registry::discover_with_plugin_roots(
        missing_manifests(&fixture.0),
        [PluginRoot::project(fixture.plugin_scope())],
    );

    assert!(registry.plugins().is_empty());
    assert!(registry.get("claude").is_some());
}

#[test]
fn later_project_root_overrides_managed_plugin_metadata() {
    let managed = Fixture::new();
    managed.install_repository_plugin("claude");
    set_version(&managed.plugin_scope().join("claude"), "0.1.1");
    let project = Fixture::new();
    project.install_repository_plugin("claude");
    set_version(&project.plugin_scope().join("claude"), "0.2.0");

    let registry = Registry::discover_with_plugin_roots(
        missing_manifests(&managed.0),
        [
            PluginRoot::managed(managed.plugin_scope()),
            PluginRoot::project(project.plugin_scope()),
        ],
    );
    let plugins = registry.plugins();

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info().version(), "0.2.0");
    assert_eq!(plugins[0].origin(), PluginOrigin::Project);
}

#[test]
fn standard_layout_discovers_managed_then_project_plugins() {
    let managed = Fixture::new();
    let managed_scope = managed.0.join("plugins/npm/node_modules/@orchester");
    managed.install_repository_plugin_at(&managed_scope, "claude");
    set_version(&managed_scope.join("claude"), "0.1.1");

    let project = Fixture::new();
    let project_scope = project.0.join("node_modules/@orchester");
    project.install_repository_plugin_at(&project_scope, "claude");
    set_version(&project_scope.join("claude"), "0.2.0");

    let roots = standard_plugin_roots(&managed.0, &project.0).unwrap();
    let registry = Registry::discover_with_plugin_roots(missing_manifests(&project.0), roots);
    let plugins = registry.plugins();

    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].info().version(), "0.2.0");
    assert_eq!(plugins[0].origin(), PluginOrigin::Project);
}

#[test]
fn standard_layout_rejects_relative_bases_without_echoing_them() {
    let fixture = Fixture::new();

    let managed_error = standard_plugin_roots("relative-secret-home", &fixture.0).unwrap_err();
    assert_eq!(managed_error, PluginRootError::ManagedHomeNotAbsolute);
    assert!(!format!("{managed_error:?}").contains("relative-secret-home"));

    let project_error = standard_plugin_roots(&fixture.0, "relative-secret-project").unwrap_err();
    assert_eq!(project_error, PluginRootError::ProjectDirectoryNotAbsolute);
    assert!(!format!("{project_error:?}").contains("relative-secret-project"));
}

#[test]
fn oversized_scope_fails_closed_before_loading_a_valid_entry() {
    let fixture = Fixture::new();
    fixture.install_repository_plugin("claude");
    for index in 0..64 {
        fs::create_dir_all(fixture.plugin_scope().join(format!("empty-{index}"))).unwrap();
    }

    let registry = Registry::discover_with_plugin_roots(
        missing_manifests(&fixture.0),
        [PluginRoot::project(fixture.plugin_scope())],
    );

    assert!(registry.plugins().is_empty());
}

fn set_version(package: &Path, version: &str) {
    for file in ["package.json", "orchester-plugin.json"] {
        let path = package.join(file);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["version"] = serde_json::Value::String(version.to_owned());
        fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn linked_plugin_scope_is_not_traversed() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.install_repository_plugin("claude");
    let alias = fixture.0.with_extension("scope-link");
    symlink(fixture.plugin_scope(), &alias).unwrap();

    let registry = Registry::discover_with_plugin_roots(
        missing_manifests(&fixture.0),
        [PluginRoot::project(&alias)],
    );
    assert!(registry.plugins().is_empty());

    fs::remove_file(alias).unwrap();
}
