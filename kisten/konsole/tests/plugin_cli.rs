mod support;

use std::path::{Path, PathBuf};

use support::{orchester, stderr, stdout, temp_home};

fn install_repository_plugin(scope: &Path, marker: &str) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins/claude");
    let destination = scope.join("claude");
    std::fs::create_dir_all(destination.join("manifests")).unwrap();
    for relative in ["package.json", "orchester-plugin.json"] {
        std::fs::copy(source.join(relative), destination.join(relative)).unwrap();
    }
    let manifest = std::fs::read_to_string(source.join("manifests/claude.toml"))
        .unwrap()
        .replace(
            "kinds = [\"code\", \"chat\"]",
            &format!("kinds = [\"{marker}\"]"),
        );
    std::fs::write(destination.join("manifests/claude.toml"), manifest).unwrap();
}

#[test]
fn list_discovers_project_npm_plugins() {
    let project = temp_home("project-plugin");
    let home = temp_home("project-plugin-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(
        &project.join("node_modules/@orchester"),
        "project-plugin-marker",
    );

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .arg("list")
        .output()
        .expect("list project plugin adapters");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("project-plugin-marker"));
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn list_discovers_managed_npm_plugins() {
    let project = temp_home("managed-plugin-project");
    let home = temp_home("managed-plugin-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(
        &home.join("plugins/npm/node_modules/@orchester"),
        "managed-plugin-marker",
    );

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .arg("list")
        .output()
        .expect("list managed plugin adapters");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("managed-plugin-marker"));
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn relative_orchester_home_fails_without_echoing_the_value() {
    let project = temp_home("relative-home-project");
    std::fs::create_dir_all(&project).unwrap();

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", "relative-secret-home")
        .arg("list")
        .output()
        .expect("reject relative Orchester home");

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("managed plugin home must be an absolute path"));
    assert!(!err.contains("relative-secret-home"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn plugin_list_reports_validated_package_metadata() {
    let project = temp_home("plugin-list-project");
    let home = temp_home("plugin-list-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(&project.join("node_modules/@orchester"), "code");

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .args(["plugin", "list"])
        .output()
        .expect("list installed plugins");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    for expected in [
        "claude",
        "Claude Code",
        "@orchester/claude",
        "0.1.0",
        "project",
    ] {
        assert!(out.contains(expected), "missing {expected} in:\n{out}");
    }
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn plugin_status_can_emit_validated_json() {
    let project = temp_home("plugin-status-project");
    let home = temp_home("plugin-status-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(&project.join("node_modules/@orchester"), "code");

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .args(["plugin", "status", "claude", "--json"])
        .output()
        .expect("show plugin status");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_str(stdout(&output).trim()).unwrap();
    assert_eq!(value["name"], "claude");
    assert_eq!(value["displayName"], "Claude Code");
    assert_eq!(value["packageName"], "@orchester/claude");
    assert_eq!(value["version"], "0.1.0");
    assert_eq!(value["origin"], "project");
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn empty_plugin_json_list_is_empty_jsonl() {
    let project = temp_home("empty-plugin-list-project");
    let home = temp_home("empty-plugin-list-home");
    std::fs::create_dir_all(&project).unwrap();

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .args(["plugin", "list", "--json"])
        .output()
        .expect("list empty plugin registry");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).is_empty());
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn missing_plugin_status_is_redacted_and_fails() {
    let project = temp_home("missing-plugin-project");
    let home = temp_home("missing-plugin-home");
    std::fs::create_dir_all(&project).unwrap();

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .args(["plugin", "status", "secret-plugin-name"])
        .output()
        .expect("show missing plugin status");

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("agent plugin is not installed"));
    assert!(!err.contains("secret-plugin-name"));
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}
