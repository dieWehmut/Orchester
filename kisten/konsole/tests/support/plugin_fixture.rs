// Each integration-test crate uses a different subset of the shared fixture.
#![allow(dead_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::support::{orchester, temp_home};

pub struct PluginFixture {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    archive: PathBuf,
    args_log: PathBuf,
    fake_bin: PathBuf,
}

impl PluginFixture {
    pub fn new(name: &str, extra_member: bool) -> Self {
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

    pub fn command(&self) -> std::process::Command {
        let mut command = orchester();
        command
            .current_dir(&self.project)
            .env("ORCHESTER_HOME", &self.home)
            .env("ORCHESTER_TEST_ARCHIVE", &self.archive)
            .env("ORCHESTER_TEST_ARGS_LOG", &self.args_log)
            .env("PATH", prepend_path(&self.fake_bin));
        command
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn args_log(&self) -> &Path {
        &self.args_log
    }

    pub fn fake_bin(&self) -> &Path {
        &self.fake_bin
    }

    pub fn installed_plugin(&self) -> PathBuf {
        self.home.join("plugins/npm/node_modules/@orchester/claude")
    }

    pub fn ownership_receipt(&self) -> PathBuf {
        self.home.join("plugins/npm/.receipts/claude.json")
    }

    pub fn project_plugin(&self) -> PathBuf {
        self.project.join("node_modules/@orchester/claude")
    }
}

impl Drop for PluginFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn copy_repository_plugin(destination: &Path) {
    let source = repository_plugin();
    fs::create_dir_all(destination.join("manifests")).unwrap();
    for relative in [
        "package.json",
        "orchester-plugin.json",
        "manifests/claude.toml",
    ] {
        fs::copy(source.join(relative), destination.join(relative)).unwrap();
    }
}

fn repository_plugin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins/claude")
}

fn prepend_path(first: &Path) -> std::ffi::OsString {
    let existing = env::var_os("PATH").unwrap_or_default();
    env::join_paths(std::iter::once(first.to_path_buf()).chain(env::split_paths(&existing)))
        .unwrap()
}

fn write_plugin_archive(path: &Path, extra_member: bool) {
    let source = repository_plugin();
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
