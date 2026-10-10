#[cfg(windows)]
mod windows;

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// Build a non-interactive helper without allocating a Windows console.
/// Captured stdout/stderr and exit statuses are preserved. Interactive CLI
/// launches deliberately use their own command builder instead.
pub fn background_command(program: impl AsRef<OsStr>) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    command.stdin(std::process::Stdio::null());
    command
}

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

#[cfg(all(test, windows))]
mod tests {
    use super::background_command;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }

    #[test]
    fn background_child_has_no_console_and_keeps_output() {
        const PROBE: &str = "ORCHESTER_BACKGROUND_CONSOLE_PROBE";
        if std::env::var_os(PROBE).is_some() {
            assert!(unsafe { GetConsoleWindow() }.is_null());
            println!("background-child-output");
            eprintln!("background-child-error");
            return;
        }
        let output = background_command(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process::tests::background_child_has_no_console_and_keeps_output",
                "--nocapture",
            ])
            .env(PROBE, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("background-child-output"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("background-child-error"));
    }
}
