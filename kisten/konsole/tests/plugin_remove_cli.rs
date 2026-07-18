//! Integration coverage for managed plugin removal.

#[path = "support/plugin_fixture.rs"]
mod plugin_fixture;
mod support;

use std::fs;
use std::io::Write;
use std::process::Stdio;

use plugin_fixture::{PluginFixture, copy_repository_plugin};
use support::{stderr, stdout};

#[test]
fn plugin_remove_deletes_only_receipted_managed_packages_and_is_idempotent() {
    let fixture = PluginFixture::new("plugin-remove", false);
    let install = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install before remove");
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));

    let removed = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("remove plugin");
    assert!(removed.status.success(), "stderr:\n{}", stderr(&removed));
    assert!(stdout(&removed).contains("Removed Claude Code 0.1.0"));
    assert!(!fixture.installed_plugin().exists());
    assert!(!fixture.ownership_receipt().exists());

    let repeated = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("repeat plugin removal");
    assert!(repeated.status.success(), "stderr:\n{}", stderr(&repeated));
    assert!(stdout(&repeated).contains("Plugin is not installed"));
}

#[test]
fn plugin_remove_refuses_unreceipted_managed_packages() {
    let fixture = PluginFixture::new("plugin-remove-unreceipted", false);
    copy_repository_plugin(&fixture.installed_plugin());

    let output = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("refuse unreceipted plugin");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("managed plugin ownership could not be verified"));
    assert!(fixture.installed_plugin().join("package.json").is_file());
}

#[test]
fn plugin_remove_refuses_content_drift() {
    let fixture = PluginFixture::new("plugin-remove-drift", false);
    let install = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install before drift");
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));
    fs::OpenOptions::new()
        .append(true)
        .open(fixture.installed_plugin().join("manifests/claude.toml"))
        .unwrap()
        .write_all(b"\n# drift\n")
        .unwrap();

    let output = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("refuse drifted plugin");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("managed plugin ownership could not be verified"));
    assert!(fixture.installed_plugin().exists());
    assert!(fixture.ownership_receipt().exists());
}

#[test]
fn plugin_remove_refuses_receipt_drift() {
    let fixture = PluginFixture::new("plugin-remove-receipt-drift", false);
    let install = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install before receipt drift");
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.ownership_receipt()).unwrap()).unwrap();
    receipt["fingerprint"] = serde_json::Value::String("0".repeat(64));
    fs::write(
        fixture.ownership_receipt(),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();

    let output = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("refuse drifted receipt");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("managed plugin ownership could not be verified"));
    assert!(fixture.installed_plugin().exists());
    assert!(fixture.ownership_receipt().exists());
}

#[test]
fn plugin_remove_cleans_a_stale_receipt_after_interrupted_deletion() {
    let fixture = PluginFixture::new("plugin-remove-stale-receipt", false);
    let install = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install before interrupted deletion");
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));
    fs::remove_dir_all(fixture.installed_plugin()).unwrap();

    let output = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("clean stale receipt");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("Plugin is not installed"));
    assert!(!fixture.ownership_receipt().exists());
}

#[test]
fn plugin_remove_does_not_touch_project_packages() {
    let fixture = PluginFixture::new("plugin-remove-project", false);
    let project_plugin = fixture.project_plugin();
    copy_repository_plugin(&project_plugin);

    let output = fixture
        .command()
        .args(["plugin", "remove", "claude"])
        .output()
        .expect("ignore project plugin");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("Plugin is not installed"));
    assert!(project_plugin.join("package.json").is_file());
}

#[test]
fn interactive_plugins_remove_uses_the_owned_backend() {
    let fixture = PluginFixture::new("interactive-plugin-remove", false);
    let install = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install before interactive remove");
    assert!(install.status.success(), "stderr:\n{}", stderr(&install));
    let mut command = fixture.command();
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start interactive remove");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"/plugins remove claude\n")
        .unwrap();
    drop(child.stdin.take());

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("Removed Claude Code 0.1.0"));
    assert!(!fixture.installed_plugin().exists());
    assert!(!fixture.ownership_receipt().exists());
}
