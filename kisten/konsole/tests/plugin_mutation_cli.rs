//! Integration coverage for plugin installation and removal mutations.

mod support;

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;
use support::{orchester, stderr, stdout, temp_home};

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    archive: PathBuf,
    args_log: PathBuf,
    fake_bin: PathBuf,
}

impl Fixture {
    fn new(name: &str, extra_member: bool) -> Self {
        let root = temp_home(name);
        let home = root.join("home");
        let project = root.join("project");
        let fake_bin = root.join("bin");
        let archive = root.join("plugin.tgz");
        let args_log = root.join("npm-args.txt");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&fake_bin).unwrap();
        write_plugin_archive(&archive, extra_member);
        write_fake_npm(&fake_bin);
        Self {
            root,
            home,
            project,
            archive,
            args_log,
            fake_bin,
        }
    }

    fn command(&self) -> std::process::Command {
        let mut command = orchester();
        command
            .current_dir(&self.project)
            .env("ORCHESTER_HOME", &self.home)
            .env("ORCHESTER_TEST_ARCHIVE", &self.archive)
            .env("ORCHESTER_TEST_ARGS_LOG", &self.args_log)
            .env("PATH", prepend_path(&self.fake_bin));
        command
    }

    fn installed_plugin(&self) -> PathBuf {
        self.home.join("plugins/npm/node_modules/@orchester/claude")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn plugin_install_materializes_a_validated_package_without_scripts() {
    let fixture = Fixture::new("plugin-install", false);

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
    let args = fs::read_to_string(&fixture.args_log).unwrap();
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
    let fixture = Fixture::new("plugin-install-extra", true);

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject plugin archive");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("plugin package archive is invalid"));
    assert!(!fixture.installed_plugin().exists());
}

#[test]
fn plugin_install_rejects_invalid_names_before_starting_npm() {
    let fixture = Fixture::new("plugin-install-name", false);

    let output = fixture
        .command()
        .args(["plugin", "install", "../secret-plugin"])
        .output()
        .expect("reject plugin name");

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("plugin name is invalid"));
    assert!(!err.contains("secret-plugin"));
    assert!(!fixture.args_log.exists());
    assert!(!fixture.installed_plugin().exists());
}

#[test]
fn plugin_install_rejects_existing_targets_before_starting_npm() {
    let fixture = Fixture::new("plugin-install-existing", false);
    copy_repository_plugin(&fixture.installed_plugin());

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject installed plugin");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("plugin is already installed"));
    assert!(!fixture.args_log.exists());
    assert!(fixture.installed_plugin().join("package.json").is_file());
}

#[cfg(windows)]
#[test]
fn plugin_install_rejects_unparsed_cmd_shims() {
    let fixture = Fixture::new("plugin-install-shell", false);
    fs::write(fixture.fake_bin.join("npm.cmd"), "@exit /b 0\r\n").unwrap();

    let output = fixture
        .command()
        .args(["plugin", "install", "claude"])
        .output()
        .expect("reject shell-backed npm");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("npm launcher requires an unsafe shell fallback"));
    assert!(!fixture.args_log.exists());
    assert!(!fixture.installed_plugin().exists());
}

fn prepend_path(first: &Path) -> std::ffi::OsString {
    let existing = env::var_os("PATH").unwrap_or_default();
    env::join_paths(std::iter::once(first.to_path_buf()).chain(env::split_paths(&existing)))
        .unwrap()
}

fn write_plugin_archive(path: &Path, extra_member: bool) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins/claude");
    let file = fs::File::create(path).unwrap();
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = tar::Builder::new(encoder);
    for relative in [
        "package.json",
        "orchester-plugin.json",
        "manifests/claude.toml",
    ] {
        append_archive_file(
            &mut archive,
            &format!("package/{relative}"),
            &fs::read(source.join(relative)).unwrap(),
        );
    }
    if extra_member {
        append_archive_file(&mut archive, "package/install.js", b"do not execute\n");
    }
    archive.into_inner().unwrap().finish().unwrap();
}

fn append_archive_file<W: Write>(archive: &mut tar::Builder<W>, path: &str, contents: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, path, contents).unwrap();
}

fn copy_repository_plugin(destination: &Path) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins/claude");
    fs::create_dir_all(destination.join("manifests")).unwrap();
    for relative in [
        "package.json",
        "orchester-plugin.json",
        "manifests/claude.toml",
    ] {
        fs::copy(source.join(relative), destination.join(relative)).unwrap();
    }
}

#[cfg(windows)]
fn write_fake_npm(bin: &Path) {
    fs::write(
        bin.join("npm-cli.js"),
        r#"const fs = require("fs");
const path = require("path");
const args = process.argv.slice(2);
const destinationIndex = args.indexOf("--pack-destination");
if (destinationIndex < 0 || !args[destinationIndex + 1]) process.exit(2);
fs.copyFileSync(
  process.env.ORCHESTER_TEST_ARCHIVE,
  path.join(args[destinationIndex + 1], "orchester-claude-0.1.0.tgz"),
);
fs.writeFileSync(process.env.ORCHESTER_TEST_ARGS_LOG, args.join("\n") + "\n");
"#,
    )
    .unwrap();
    fs::write(
        bin.join("npm.cmd"),
        "@ECHO off\r\nnode \"%dp0%\\npm-cli.js\" %*\r\n",
    )
    .unwrap();
}

#[cfg(unix)]
fn write_fake_npm(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let npm = bin.join("npm");
    fs::write(
        &npm,
        r#"#!/bin/sh
set -eu
destination=
previous=
for argument in "$@"; do
  if [ "$previous" = "--pack-destination" ]; then destination=$argument; fi
  previous=$argument
done
test -n "$destination"
cp "$ORCHESTER_TEST_ARCHIVE" "$destination/orchester-claude-0.1.0.tgz"
printf '%s\n' "$@" > "$ORCHESTER_TEST_ARGS_LOG"
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&npm).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(npm, permissions).unwrap();
}
