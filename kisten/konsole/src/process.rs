#[cfg(windows)]
mod windows;

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

#[cfg(windows)]
pub use windows::command_invocation;

#[derive(Debug)]
pub struct CommandInvocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub envs: Vec<(OsString, OsString)>,
    shell_backed: bool,
}

impl CommandInvocation {
    pub fn uses_shell(&self) -> bool {
        self.shell_backed
    }
}

#[cfg(not(windows))]
pub fn command_invocation(executable: &Path, extra_args: Vec<OsString>) -> CommandInvocation {
    CommandInvocation {
        program: executable.to_path_buf(),
        args: extra_args,
        envs: Vec::new(),
        shell_backed: false,
    }
}

pub fn resolve_command(command: &str) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 || command_path.is_absolute() {
        return command_path.is_file().then(|| command_path.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    let names = executable_names(command);
    for dir in env::split_paths(&path) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_names(command: &str) -> Vec<OsString> {
    windows::executable_names(command)
}

#[cfg(not(windows))]
fn executable_names(command: &str) -> Vec<OsString> {
    vec![OsString::from(command)]
}

pub fn is_cancelled_status(status: &ExitStatus) -> bool {
    #[cfg(windows)]
    {
        const STATUS_CONTROL_C_EXIT: i32 = 0xC000_013A_u32 as i32;
        matches!(status.code(), Some(130) | Some(STATUS_CONTROL_C_EXIT))
    }

    #[cfg(not(windows))]
    {
        status.code() == Some(130)
    }
}
