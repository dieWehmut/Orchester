//! Structured, shell-free process execution inside a validated workspace.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use orchester_protokoll::{AgentAction, PolicyDecision};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::barrier::StartedTool;
use super::governance::{PolicyEngine, WorkspaceGuard};
use super::process::BoundedOutput;
use super::process_tree::ProcessTree;

const READ_BUFFER_BYTES: usize = 8 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 128;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_ENVIRONMENT_ENTRIES: usize = 128;
const MAX_ENVIRONMENT_BYTES: usize = 64 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessLimits {
    pub max_output_bytes: usize,
    pub timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: 8 * 1024 * 1024,
            timeout: Duration::from_secs(60),
            poll_interval: Duration::from_millis(10),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ProcessSpec {
    program: OsString,
    args: Vec<OsString>,
    cwd: PathBuf,
    environment: BTreeMap<OsString, OsString>,
}

impl ProcessSpec {
    fn new(program: impl Into<OsString>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            environment: BTreeMap::new(),
        }
    }

    fn args<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(values.into_iter().map(Into::into));
        self
    }

    #[cfg(test)]
    fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }
}

impl fmt::Debug for ProcessSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessSpec")
            .field("program_bytes", &os_bytes(&self.program))
            .field("argument_count", &self.args.len())
            .field("cwd_bytes", &os_bytes(self.cwd.as_os_str()))
            .field("environment_count", &self.environment.len())
            .finish()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    #[error("process specification is invalid")]
    InvalidSpec,
    #[error("process working directory is outside or unavailable")]
    InvalidCwd,
    #[error("process command is denied by core policy")]
    PolicyDenied,
    #[error("process environment contains a secret-bearing key")]
    SecretEnvironment,
    #[error("process was cancelled before it could start")]
    CancelledBeforeStart,
    #[error("process could not be started")]
    SpawnFailed,
    #[error("process could not be bound to its process tree")]
    ProcessTreeUnavailable,
    #[error("process status could not be observed")]
    WaitFailed,
    #[error("process output could not be captured")]
    OutputFailed,
    #[error("started tool is not a process action")]
    WrongAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTermination {
    Exited(i32),
    Signaled,
    Cancelled,
    TimedOut,
}

pub struct ProcessRunResult {
    pub termination: CommandTermination,
    pub stdout: BoundedOutput,
    pub stderr: BoundedOutput,
    pub elapsed: Duration,
}

impl fmt::Debug for ProcessRunResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessRunResult")
            .field("termination", &self.termination)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("elapsed", &self.elapsed)
            .finish()
    }
}

struct ProcessRunner {
    workspace: WorkspaceGuard,
    limits: ProcessLimits,
}

impl ProcessRunner {
    fn new(root: impl AsRef<Path>, limits: ProcessLimits) -> Result<Self, ProcessError> {
        if limits.max_output_bytes == 0
            || limits.max_output_bytes > MAX_OUTPUT_BYTES
            || limits.timeout.is_zero()
            || limits.timeout > MAX_TIMEOUT
            || limits.poll_interval.is_zero()
            || limits.poll_interval > Duration::from_secs(1)
        {
            return Err(ProcessError::InvalidSpec);
        }
        let workspace = WorkspaceGuard::new(root.as_ref()).map_err(|_| ProcessError::InvalidCwd)?;
        Ok(Self { workspace, limits })
    }

    async fn run(
        &self,
        spec: ProcessSpec,
        cancellation: CancellationToken,
    ) -> Result<ProcessRunResult, ProcessError> {
        if cancellation.is_cancelled() {
            return Err(ProcessError::CancelledBeforeStart);
        }
        validate_spec(&spec)?;
        self.workspace
            .directory_entries(&spec.cwd)
            .map_err(|_| ProcessError::InvalidCwd)?;
        let policy = PolicyEngine::new().evaluate_command(&spec.program, &spec.args);
        if policy.decision == PolicyDecision::Deny {
            return Err(ProcessError::PolicyDenied);
        }

        let process_tree = ProcessTree::new().map_err(|_| ProcessError::ProcessTreeUnavailable)?;
        let mut command = Command::new(&spec.program);
        process_tree.configure_command(&mut command);
        command
            .args(&spec.args)
            .current_dir(self.workspace.root().join(&spec.cwd))
            .env_clear()
            .envs(&spec.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let started = Instant::now();
        let mut child = command.spawn().map_err(|_| ProcessError::SpawnFailed)?;
        if process_tree.attach(&child).is_err() {
            process_tree.terminate(&mut child);
            let _ = child.wait().await;
            return Err(ProcessError::ProcessTreeUnavailable);
        }
        let stdout = child.stdout.take().ok_or(ProcessError::SpawnFailed)?;
        let stderr = child.stderr.take().ok_or(ProcessError::SpawnFailed)?;
        let output_limit = self.limits.max_output_bytes;
        let stdout_task = tokio::spawn(capture_async(stdout, output_limit));
        let stderr_task = tokio::spawn(capture_async(stderr, output_limit));

        let termination = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    break status
                        .code()
                        .map(CommandTermination::Exited)
                        .unwrap_or(CommandTermination::Signaled)
                }
                Ok(None) => {}
                Err(_) => {
                    terminate_child(&process_tree, &mut child).await;
                    return Err(ProcessError::WaitFailed);
                }
            }
            if cancellation.is_cancelled() {
                terminate_child(&process_tree, &mut child).await;
                break CommandTermination::Cancelled;
            }
            if started.elapsed() >= self.limits.timeout {
                terminate_child(&process_tree, &mut child).await;
                break CommandTermination::TimedOut;
            }
            tokio::time::sleep(self.limits.poll_interval).await;
        };
        let stdout = stdout_task
            .await
            .map_err(|_| ProcessError::OutputFailed)?
            .map_err(|_| ProcessError::OutputFailed)?;
        let stderr = stderr_task
            .await
            .map_err(|_| ProcessError::OutputFailed)?
            .map_err(|_| ProcessError::OutputFailed)?;
        Ok(ProcessRunResult {
            termination,
            stdout,
            stderr,
            elapsed: started.elapsed(),
        })
    }
}

/// Permit-bound process facade. The only accepted action is the durable
/// `RunCommand` returned by `PreExecutionBarrier::start_tool`.
pub struct GovernedProcessRunner {
    runner: ProcessRunner,
}

impl GovernedProcessRunner {
    pub fn new(root: impl AsRef<Path>, limits: ProcessLimits) -> Result<Self, ProcessError> {
        Ok(Self {
            runner: ProcessRunner::new(root, limits)?,
        })
    }

    pub async fn execute(
        &self,
        started: StartedTool,
        cancellation: CancellationToken,
    ) -> Result<ProcessRunResult, ProcessError> {
        let AgentAction::RunCommand { program, args, cwd } = started.into_action() else {
            return Err(ProcessError::WrongAction);
        };
        let spec = ProcessSpec::new(program, cwd.unwrap_or_else(|| ".".into())).args(args);
        self.runner.run(spec, cancellation).await
    }
}

async fn terminate_child(process_tree: &ProcessTree, child: &mut tokio::process::Child) {
    process_tree.terminate(child);
    let _ = child.wait().await;
}

async fn capture_async<R: AsyncRead + Unpin>(
    mut reader: R,
    max_bytes: usize,
) -> std::io::Result<BoundedOutput> {
    let mut retained = Vec::with_capacity(max_bytes.min(READ_BUFFER_BYTES));
    let mut total_bytes = 0_u64;
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes
            .checked_add(u64::try_from(read).map_err(|_| std::io::Error::other("byte count"))?)
            .ok_or_else(|| std::io::Error::other("byte count overflow"))?;
        let remaining = max_bytes.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(BoundedOutput::from_parts(retained, total_bytes))
}

fn validate_spec(spec: &ProcessSpec) -> Result<(), ProcessError> {
    if spec.program.is_empty()
        || os_bytes(&spec.program) > MAX_ARGUMENT_BYTES
        || spec.args.len() > MAX_ARGUMENTS
        || spec
            .args
            .iter()
            .any(|argument| os_bytes(argument) > MAX_ARGUMENT_BYTES || contains_nul(argument))
        || contains_nul(&spec.program)
        || spec.environment.len() > MAX_ENVIRONMENT_ENTRIES
    {
        return Err(ProcessError::InvalidSpec);
    }
    let mut environment_bytes = 0_usize;
    for (key, value) in &spec.environment {
        if key.is_empty() || contains_nul(key) || contains_nul(value) || is_secret_key(key) {
            return Err(ProcessError::SecretEnvironment);
        }
        environment_bytes = environment_bytes
            .saturating_add(os_bytes(key))
            .saturating_add(os_bytes(value));
        if environment_bytes > MAX_ENVIRONMENT_BYTES {
            return Err(ProcessError::InvalidSpec);
        }
    }
    Ok(())
}

fn is_secret_key(key: &OsStr) -> bool {
    let key = key.to_string_lossy().to_ascii_uppercase();
    [
        "API_KEY",
        "APIKEY",
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "AUTHORIZATION",
        "CREDENTIAL",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

fn contains_nul(value: &OsStr) -> bool {
    value.to_string_lossy().contains('\0')
}

fn os_bytes(value: &OsStr) -> usize {
    value.to_string_lossy().len()
}

#[cfg(test)]
#[path = "process_runtime_tests.rs"]
mod tests;
