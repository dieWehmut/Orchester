mod archive;
mod store;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use orchester_verzeichnis::{PluginInfo, load_agent_plugin};
use thiserror::Error;

use crate::process::{command_invocation, resolve_command};

const NPM_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InstallError {
    #[error("plugin name is invalid")]
    InvalidName,
    #[error("npm is not available")]
    NpmUnavailable,
    #[error("npm launcher requires an unsafe shell fallback")]
    UnsafeNpmLauncher,
    #[error("plugin staging directory is unavailable")]
    StagingUnavailable,
    #[error("npm pack failed")]
    NpmFailed,
    #[error("npm pack timed out")]
    NpmTimedOut,
    #[error("plugin package archive is invalid")]
    InvalidArchive,
    #[error("plugin package metadata is invalid")]
    InvalidPackage,
    #[error("plugin is already installed")]
    AlreadyInstalled,
    #[error("plugin activation failed")]
    ActivationFailed,
}

pub fn install(orchester_home: &Path, name: &str) -> Result<PluginInfo, InstallError> {
    if !valid_name(name) {
        return Err(InstallError::InvalidName);
    }
    let mut transaction = store::InstallTransaction::new(orchester_home, name)?;
    run_npm_pack(transaction.staging_path(), name)?;
    let archive = find_archive(transaction.staging_path())?;
    let package = transaction.staging_path().join("package");
    archive::materialize(&archive, &package, name).map_err(|_| InstallError::InvalidArchive)?;
    let loaded = load_agent_plugin(&package).map_err(|_| InstallError::InvalidPackage)?;
    let expected_package = format!("@orchester/{name}");
    if loaded.info().name() != name
        || loaded.info().package_name() != expected_package
        || loaded.info().version() != env!("CARGO_PKG_VERSION")
    {
        return Err(InstallError::InvalidPackage);
    }
    let info = loaded.info().clone();
    transaction.activate(&package)?;
    Ok(info)
}

fn run_npm_pack(destination: &Path, name: &str) -> Result<(), InstallError> {
    let executable = resolve_command("npm").ok_or(InstallError::NpmUnavailable)?;
    let package = format!("@orchester/{name}@{}", env!("CARGO_PKG_VERSION"));
    let arguments = vec![
        OsString::from("pack"),
        OsString::from("--ignore-scripts"),
        OsString::from("--no-audit"),
        OsString::from("--no-fund"),
        OsString::from("--pack-destination"),
        destination.as_os_str().to_os_string(),
        OsString::from("--"),
        OsString::from(package),
    ];
    let invocation = command_invocation(&executable, arguments);
    if invocation.uses_shell() {
        return Err(InstallError::UnsafeNpmLauncher);
    }

    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .current_dir(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in &invocation.envs {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|_| InstallError::NpmFailed)?;
    let deadline = Instant::now() + NPM_TIMEOUT;
    loop {
        match child.try_wait().map_err(|_| InstallError::NpmFailed)? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => return Err(InstallError::NpmFailed),
            None if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(InstallError::NpmTimedOut);
            }
        }
    }
}

fn find_archive(staging: &Path) -> Result<PathBuf, InstallError> {
    let entries = fs::read_dir(staging).map_err(|_| InstallError::InvalidArchive)?;
    let mut archive = None;
    for entry in entries {
        let entry = entry.map_err(|_| InstallError::InvalidArchive)?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| InstallError::InvalidArchive)?;
        if archive.is_some()
            || !metadata.is_file()
            || is_link_or_reparse(&metadata)
            || entry.path().extension().and_then(|value| value.to_str()) != Some("tgz")
        {
            return Err(InstallError::InvalidArchive);
        }
        archive = Some(entry.path());
    }
    archive.ok_or(InstallError::InvalidArchive)
}

fn valid_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn plugin_names_are_bounded_package_segments() {
        assert!(valid_name("claude"));
        assert!(valid_name("agent-2"));
        for invalid in ["", "-agent", "agent-", "Agent", "../agent", "a_b"] {
            assert!(!valid_name(invalid));
        }
        assert!(!valid_name(&"a".repeat(65)));
    }
}
