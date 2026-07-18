//! Integration coverage for plugin installation mutations.

#[path = "support/plugin_fixture.rs"]
mod plugin_fixture;
mod support;

use std::fs;

use plugin_fixture::{PluginFixture, copy_repository_plugin};
use support::{stderr, stdout};

#[test]
fn plugin_install_materializes_a_validated_package_without_scripts() {
    let fixture = PluginFixture::new("plugin-install", false);

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("install plugin");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("Installed Claude Code 0.1.0"));
    let installed = fixture.installed_plugin();
    assert!(installed.join("package.json").is_file());
    assert!(installed.join("orchester-plugin.json").is_file());
    assert!(installed.join("manifests/claude.toml").is_file());
    let receipt_source = fs::read_to_string(fixture.ownership_receipt()).unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&receipt_source).unwrap();
    assert_eq!(receipt["schemaVersion"], 1);
    assert_eq!(receipt["name"], "claude");
    assert_eq!(receipt["packageName"], "@orchester/claude");
    assert_eq!(receipt["version"], "0.1.0");
    assert_eq!(receipt["fingerprint"].as_str().unwrap().len(), 64);
    assert!(!receipt_source.contains(fixture.root().to_string_lossy().as_ref()));
    let args = fs::read_to_string(fixture.args_log()).unwrap();
    for expected in [
        "pack",
        "--ignore-scripts",
        "--no-audit",
        "--no-fund",
        "--pack-destination",
        "@orchester/claude@0.1.0",
    ] {
        assert!(args.lines().any(|arg| arg == expected), "npm args:\n{args}");
    }
    assert!(!args.lines().any(|arg| arg == "install"));
}

#[test]
fn plugin_install_rejects_extra_archive_members_before_activation() {
    let fixture = PluginFixture::new("plugin-install-extra", true);

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject plugin archive");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("plugin package archive is invalid"));
    assert!(!fixture.installed_plugin().exists());
    assert!(!fixture.ownership_receipt().exists());
}

#[test]
fn plugin_install_rejects_invalid_names_before_starting_npm() {
    let fixture = PluginFixture::new("plugin-install-name", false);

    let output = fixture
        .command()
        .args(["plugin", "install", "../secret-plugin"])
        .output()
        .expect("reject plugin name");

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("plugin name is invalid"));
    assert!(!err.contains("secret-plugin"));
    assert!(!fixture.args_log().exists());
    assert!(!fixture.installed_plugin().exists());
    assert!(!fixture.ownership_receipt().exists());
}

#[test]
fn plugin_install_rejects_existing_targets_before_starting_npm() {
    let fixture = PluginFixture::new("plugin-install-existing", false);
    copy_repository_plugin(&fixture.installed_plugin());

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject installed plugin");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("plugin is already installed"));
    assert!(!fixture.args_log().exists());
    assert!(fixture.installed_plugin().join("package.json").is_file());
}

#[cfg(windows)]
#[test]
fn plugin_install_rejects_unparsed_cmd_shims() {
    let fixture = PluginFixture::new("plugin-install-shell", false);
    fs::write(fixture.fake_bin().join("npm.cmd"), "@exit /b 0\r\n").unwrap();

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject shell-backed npm");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("npm launcher requires an unsafe shell fallback"));
    assert!(!fixture.args_log().exists());
    assert!(!fixture.installed_plugin().exists());
}
