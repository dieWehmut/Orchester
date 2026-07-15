use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_vertrag::AgentAdapter;
use orchester_verzeichnis::{PluginError, load_agent_plugin};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn repository_plugin(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins")
        .join(name)
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "orchester-plugin-runtime-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("manifests")).unwrap();
        let source = repository_plugin("claude");
        for relative in [
            "package.json",
            "orchester-plugin.json",
            "manifests/claude.toml",
        ] {
            fs::copy(source.join(relative), root.join(relative)).unwrap();
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mutate_json(path: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    mutate(&mut value);
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[test]
fn repository_claude_package_loads_as_a_validated_adapter() {
    let plugin = load_agent_plugin(repository_plugin("claude")).unwrap();

    assert_eq!(plugin.info().name(), "claude");
    assert_eq!(plugin.info().display_name(), "Claude Code");
    assert_eq!(plugin.info().package_name(), "@orchester/claude");
    assert_eq!(plugin.info().version(), "0.1.0");
    assert_eq!(plugin.adapter().name(), "claude");
    assert_eq!(plugin.adapter().native_command(), Some("claude"));
    assert!(!format!("{plugin:?}").contains("stream-json"));
}

#[test]
fn executable_package_fields_and_unknown_descriptor_fields_fail_closed() {
    let package = Fixture::new();
    mutate_json(&package.path().join("package.json"), |value| {
        value["scripts"] = serde_json::json!({ "postinstall": "do-not-echo-canary" });
    });
    let error = load_agent_plugin(package.path()).unwrap_err();
    assert_eq!(error, PluginError::InvalidPackage);
    assert!(!format!("{error:?}").contains("do-not-echo-canary"));

    let descriptor = Fixture::new();
    mutate_json(&descriptor.path().join("orchester-plugin.json"), |value| {
        value["unknown"] = serde_json::json!("do-not-echo-canary");
    });
    let error = load_agent_plugin(descriptor.path()).unwrap_err();
    assert_eq!(error, PluginError::InvalidDescriptor);
    assert!(!error.to_string().contains("do-not-echo-canary"));
}

#[test]
fn extra_members_and_adapter_identity_drift_are_rejected() {
    let extra = Fixture::new();
    fs::write(extra.path().join("install.js"), "do-not-run\n").unwrap();
    assert_eq!(
        load_agent_plugin(extra.path()).unwrap_err(),
        PluginError::InvalidPackage
    );

    let drift = Fixture::new();
    let manifest = drift.path().join("manifests/claude.toml");
    let source = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        source.replacen("command = \"claude\"", "command = \"other\"", 1),
    )
    .unwrap();
    assert_eq!(
        load_agent_plugin(drift.path()).unwrap_err(),
        PluginError::InvalidManifest
    );
}

#[test]
fn bounded_reads_reject_oversize_and_invalid_utf8() {
    let oversized = Fixture::new();
    fs::write(
        oversized.path().join("package.json"),
        vec![b' '; 64 * 1024 + 1],
    )
    .unwrap();
    assert_eq!(
        load_agent_plugin(oversized.path()).unwrap_err(),
        PluginError::LimitExceeded
    );

    let invalid_utf8 = Fixture::new();
    fs::write(
        invalid_utf8.path().join("orchester-plugin.json"),
        b"{\"canary\":\"\xff\"}",
    )
    .unwrap();
    let error = load_agent_plugin(invalid_utf8.path()).unwrap_err();
    assert_eq!(error, PluginError::InvalidPackage);
    assert!(!format!("{error:?}").contains("canary"));
}

#[cfg(unix)]
#[test]
fn package_and_manifest_links_are_rejected() {
    use std::os::unix::fs::symlink;

    let linked_root = Fixture::new();
    let alias = linked_root.path().with_extension("link");
    symlink(linked_root.path(), &alias).unwrap();
    assert_eq!(
        load_agent_plugin(&alias).unwrap_err(),
        PluginError::Unavailable
    );
    fs::remove_file(&alias).unwrap();

    let linked_manifest = Fixture::new();
    let manifest = linked_manifest.path().join("manifests/claude.toml");
    let target = linked_manifest.path().join("target.toml");
    fs::rename(&manifest, &target).unwrap();
    symlink(&target, &manifest).unwrap();
    assert_eq!(
        load_agent_plugin(linked_manifest.path()).unwrap_err(),
        PluginError::InvalidPackage
    );
}
