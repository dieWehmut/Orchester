//! Independent background workers keep slow UNC shares off the local API path.
use super::{ActivityDetail, ActivitySource};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Default)]
struct SourceState {
    rows: Vec<(String, ActivityDetail)>,
    status: &'static str,
    updated: Option<Instant>,
}
#[derive(Clone)]
struct Source {
    root: PathBuf,
    environment: String,
    provider: String,
    state: Arc<Mutex<SourceState>>,
}
#[derive(Clone, Default)]
pub(super) struct Discovery(Arc<Mutex<HashMap<String, Source>>>);
impl Discovery {
    pub fn new(enabled: bool) -> Self {
        let this = Self::default();
        #[cfg(windows)]
        if enabled {
            let weak = Arc::downgrade(&this.0);
            std::thread::spawn(move || loop {
                if weak.strong_count() == 0 {
                    break;
                }
                for (environment, provider, root) in roots() {
                    let Some(state) = weak.upgrade() else { return };
                    let key = format!("{environment}:{provider}");
                    let Ok(mut sources) = state.lock() else {
                        return;
                    };
                    if sources.contains_key(&key) {
                        continue;
                    }
                    let source = Source {
                        root,
                        environment,
                        provider,
                        state: Arc::new(Mutex::new(SourceState::default())),
                    };
                    sources.insert(key, source.clone());
                    let weak_source = Arc::downgrade(&source.state);
                    let Source {
                        root,
                        environment,
                        provider,
                        state: _,
                    } = source;
                    std::thread::spawn(move || {
                        let mut cache = HashMap::new();
                        loop {
                            if weak_source.strong_count() == 0 {
                                break;
                            }
                            let active = environment.strip_prefix("WSL · ").is_some_and(|name| {
                                running().iter().any(|running| running == name)
                            });
                            let (status, rows) = if !active {
                                ("unavailable", Vec::new())
                            } else if provider == "opencode" {
                                super::opencode::read_provider(Some(&root))
                            } else {
                                super::files::read_provider(&provider, Some(&root), &mut cache)
                            };
                            let Some(state) = weak_source.upgrade() else {
                                break;
                            };
                            if let Ok(mut state) = state.lock() {
                                apply(&mut state, &environment, status, rows);
                            }
                            drop(state);
                            std::thread::sleep(Duration::from_secs(3));
                        }
                    });
                }
                std::thread::sleep(Duration::from_secs(10));
            });
        }
        #[cfg(not(windows))]
        let _ = enabled;
        this
    }
    pub fn snapshot(&self) -> (Vec<ActivitySource>, Vec<ActivityDetail>) {
        let mut sources = Vec::new();
        let mut rows = Vec::new();
        if let Ok(all) = self.0.lock() {
            for source in all.values() {
                if let Ok(state) = source.state.lock() {
                    sources.push(ActivitySource {
                        provider: source.provider.clone(),
                        environment: Some(source.environment.clone()),
                        status: if state
                            .updated
                            .is_some_and(|t| t.elapsed() < Duration::from_secs(15))
                        {
                            state.status
                        } else {
                            "unavailable"
                        },
                    });
                    rows.extend(state.rows.iter().map(|(_, row)| row.clone()));
                }
            }
        }
        sources.sort_by(|a, b| (&a.environment, &a.provider).cmp(&(&b.environment, &b.provider)));
        (sources, rows)
    }
    pub fn resolve(&self, id: &str) -> Option<(PathBuf, String, String)> {
        for source in self.0.lock().ok()?.values() {
            if let Some((original, _)) = source
                .state
                .lock()
                .ok()?
                .rows
                .iter()
                .find(|(_, row)| row.summary.id == id)
            {
                return Some((
                    source.root.clone(),
                    original.clone(),
                    source.environment.clone(),
                ));
            }
        }
        None
    }
}
#[cfg(any(windows, test))]
fn apply(
    state: &mut SourceState,
    environment: &str,
    status: &'static str,
    rows: Vec<ActivityDetail>,
) {
    state.status = status;
    state.updated = Some(Instant::now());
    if status == "available" || !rows.is_empty() {
        state.rows = rows
            .into_iter()
            .map(|mut row| {
                let original = row.summary.id.clone();
                row.summary.id = super::opaque_id(environment, &original);
                super::apply_environment(&mut row, environment);
                (original, row)
            })
            .collect();
    }
}
#[cfg(windows)]
pub(crate) fn output(args: &[&str]) -> Option<Vec<u8>> {
    output_until(args, Instant::now() + Duration::from_secs(3))
}

#[cfg(windows)]
pub(crate) fn output_until(args: &[&str], deadline: Instant) -> Option<Vec<u8>> {
    output_result_until(args, deadline).ok()
}

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WslReadFailure {
    Timeout,
    CommandFailed,
    Unavailable,
    ReadFailed,
    OutputLimit,
}

#[cfg(windows)]
impl WslReadFailure {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::CommandFailed => "command_failed",
            Self::Unavailable => "unavailable",
            Self::ReadFailed => "read_failed",
            Self::OutputLimit => "output_limit",
        }
    }
}

#[cfg(windows)]
pub(crate) fn output_result_until(
    args: &[&str],
    deadline: Instant,
) -> Result<Vec<u8>, WslReadFailure> {
    use std::{os::windows::process::CommandExt, process::Command};
    command_output_result(
        Command::new("wsl.exe")
            .args(args)
            .creation_flags(0x08000000),
        deadline
            .checked_duration_since(Instant::now())
            .ok_or(WslReadFailure::Timeout)?,
        64 * 1024 * 1024,
    )
}

#[cfg(all(test, unix))]
fn command_output(
    command: &mut std::process::Command,
    timeout: Duration,
    limit: u64,
) -> Option<Vec<u8>> {
    command_output_result(command, timeout, limit).ok()
}

#[cfg(any(windows, test))]
fn command_output_result(
    command: &mut std::process::Command,
    timeout: Duration,
    limit: u64,
) -> Result<Vec<u8>, WslReadFailure> {
    use std::{io::Read, process::Stdio};
    let start = Instant::now();
    // A private capture file avoids EOF waits on inherited descendant handles.
    let mut capture = tempfile::NamedTempFile::new().map_err(|_| WslReadFailure::ReadFailed)?;
    if start.elapsed() >= timeout {
        return Err(WslReadFailure::Timeout);
    }
    let command = command
        .stdout(Stdio::from(
            capture.reopen().map_err(|_| WslReadFailure::ReadFailed)?,
        ))
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    #[cfg(windows)]
    let tree =
        orchester_vertrag::process::ProcessTree::new().map_err(|_| WslReadFailure::Unavailable)?;
    #[cfg(windows)]
    let mut child = tree
        .spawn_std(command)
        .map_err(|_| WslReadFailure::Unavailable)?;
    #[cfg(not(windows))]
    let mut child = command.spawn().map_err(|_| WslReadFailure::Unavailable)?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                // A launcher may exit before its delegated wsl.exe/conhost.
                // Stop those job members before taking the finite snapshot.
                #[cfg(windows)]
                tree.terminate_std(&mut child);
                if start.elapsed() >= timeout {
                    return Err(WslReadFailure::Timeout);
                }
                let size = capture
                    .as_file()
                    .metadata()
                    .map_err(|_| WslReadFailure::ReadFailed)?
                    .len();
                if size > limit {
                    return Err(WslReadFailure::OutputLimit);
                }
                let mut data = Vec::new();
                let mut reader = capture.as_file_mut().take(size);
                let mut buffer = [0_u8; 16 * 1024];
                loop {
                    if start.elapsed() >= timeout {
                        return Err(WslReadFailure::Timeout);
                    }
                    let read = reader
                        .read(&mut buffer)
                        .map_err(|_| WslReadFailure::ReadFailed)?;
                    if read == 0 {
                        break;
                    }
                    data.extend_from_slice(&buffer[..read]);
                }
                return Ok(data);
            }
            Ok(Some(_)) => return Err(WslReadFailure::CommandFailed),
            Ok(None) => {
                let failure = match capture.as_file().metadata() {
                    Ok(meta) if meta.len() > limit => Some(WslReadFailure::OutputLimit),
                    Err(_) => Some(WslReadFailure::ReadFailed),
                    _ if start.elapsed() >= timeout => Some(WslReadFailure::Timeout),
                    _ => None,
                };
                if let Some(failure) = failure {
                    #[cfg(windows)]
                    tree.terminate_std(&mut child);
                    #[cfg(not(windows))]
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(failure);
                }
            }
            Err(_) => {
                #[cfg(windows)]
                tree.terminate_std(&mut child);
                #[cfg(not(windows))]
                let _ = child.kill();
                let _ = child.wait();
                return Err(WslReadFailure::ReadFailed);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[cfg(any(windows, test))]
fn names(raw: &[u8]) -> Vec<String> {
    let names = if raw.contains(&0) {
        String::from_utf16_lossy(
            &raw.chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
    } else {
        String::from_utf8_lossy(raw).into_owned()
    };
    names
        .lines()
        .map(|s| s.trim().trim_start_matches('\u{feff}'))
        .filter(|s| !s.is_empty() && !s.contains(['\\', '/']))
        .map(str::to_owned)
        .collect()
}
#[cfg(windows)]
pub(crate) fn running() -> Vec<String> {
    running_until(Instant::now() + Duration::from_secs(3))
}
#[cfg(windows)]
pub(crate) fn running_until(deadline: Instant) -> Vec<String> {
    running_result_until(deadline).unwrap_or_default()
}
#[cfg(windows)]
pub(crate) fn running_result_until(deadline: Instant) -> Result<Vec<String>, WslReadFailure> {
    output_result_until(&["--list", "--running", "--quiet"], deadline).map(|raw| names(&raw))
}
#[cfg(windows)]
fn roots() -> Vec<(String, String, PathBuf)> {
    let mut result = Vec::new();
    for distro in running() {
        let Some(raw) = output(&["--distribution", &distro, "--exec", "sh", "-c", "printf '%s\\n' \"${CODEX_HOME:-$HOME/.codex}/sessions\" \"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/projects\" \"${OPENCODE_DB:-${XDG_DATA_HOME:-$HOME/.local/share}/opencode/opencode.db}\""]) else { continue };
        let Ok(paths) = String::from_utf8(raw) else {
            continue;
        };
        for (provider, path) in ["codex", "claude", "opencode"]
            .into_iter()
            .zip(paths.lines())
        {
            if !path.starts_with('/') || path.contains('\r') {
                continue;
            }
            result.push((
                format!("WSL · {distro}"),
                provider.into(),
                PathBuf::from(format!(
                    "\\\\wsl.localhost\\{distro}{}",
                    path.replace('/', "\\")
                )),
            ));
        }
    }
    result
}
#[cfg(windows)]
pub(super) fn database_snapshot(path: &std::path::Path) -> Option<tempfile::NamedTempFile> {
    use std::io::Write;
    let path = path.to_string_lossy().replace('\\', "/");
    let unc = path
        .strip_prefix("//?/UNC/")
        .or_else(|| path.strip_prefix("//"))?;
    let (host, rest) = unc.split_once('/')?;
    if !host.eq_ignore_ascii_case("wsl.localhost") && !host.eq_ignore_ascii_case("wsl$") {
        return None;
    }
    let (distro, suffix) = rest.split_once('/')?;
    if !running().iter().any(|name| name == distro) {
        return None;
    }
    let data = output(&[
        "--distribution",
        distro,
        "--exec",
        "python3",
        "-c",
        include_str!("wsl_snapshot.py"),
        &format!("/{suffix}"),
    ])?;
    if !data.starts_with(b"SQLite format 3\0") {
        return None;
    }
    let mut file = tempfile::NamedTempFile::new().ok()?;
    file.write_all(&data).ok()?;
    Some(file)
}

#[cfg(all(test, windows))]
#[path = "wsl_lifecycle_tests.rs"]
mod windows_lifecycle_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn completed_command_does_not_wait_for_a_descendant_holding_stdout() {
        let started = Instant::now();
        let output = command_output(
            std::process::Command::new("sh").args(["-c", "printf observed; sleep 1 &"]),
            Duration::from_millis(200),
            1024,
        );
        assert_eq!(output.as_deref(), Some(&b"observed"[..]));
        assert!(started.elapsed() < Duration::from_millis(700));
    }

    #[cfg(unix)]
    #[test]
    fn command_capture_rejects_timeouts_failures_and_excess_output() {
        assert!(command_output(
            std::process::Command::new("sh").args(["-c", "sleep 1"]),
            Duration::from_millis(40),
            1024,
        )
        .is_none());
        assert!(command_output(
            std::process::Command::new("sh").args(["-c", "printf observed; exit 1"]),
            Duration::from_secs(1),
            1024,
        )
        .is_none());
        assert!(command_output(
            std::process::Command::new("sh").args(["-c", "printf oversized"]),
            Duration::from_secs(1),
            4,
        )
        .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_failures_keep_distinct_safe_diagnostics() {
        assert_eq!(
            command_output_result(
                std::process::Command::new("sh").args(["-c", "sleep 1"]),
                Duration::from_millis(40),
                1024,
            ),
            Err(WslReadFailure::Timeout)
        );
        assert_eq!(
            command_output_result(
                std::process::Command::new("sh").args(["-c", "printf observed; exit 1"]),
                Duration::from_secs(1),
                1024,
            ),
            Err(WslReadFailure::CommandFailed)
        );
        assert_eq!(
            command_output_result(
                std::process::Command::new("sh").args(["-c", "printf oversized"]),
                Duration::from_secs(1),
                4,
            ),
            Err(WslReadFailure::OutputLimit)
        );
        assert_eq!(
            command_output_result(
                &mut std::process::Command::new("/orchester-nonexistent-process-query"),
                Duration::from_secs(1),
                1024,
            ),
            Err(WslReadFailure::Unavailable)
        );
    }

    #[test]
    fn decodes_windows_distribution_names() {
        let raw: Vec<u8> = "\u{feff}Ubuntu-22.04\r\nDebian\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(names(&raw), ["Ubuntu-22.04", "Debian"]);
    }
    #[test]
    fn isolates_duplicate_sessions_and_retains_unavailable_history() {
        let row = super::super::empty_detail("codex", "same.jsonl", 1);
        let mut a = SourceState::default();
        let mut b = SourceState::default();
        apply(&mut a, "WSL · Ubuntu", "available", vec![row.clone()]);
        apply(&mut b, "WSL · Debian", "available", vec![row]);
        assert_ne!(a.rows[0].1.summary.id, b.rows[0].1.summary.id);
        apply(&mut a, "WSL · Ubuntu", "unavailable", vec![]);
        assert_eq!(a.rows.len(), 1);
        assert_eq!(a.status, "unavailable");
    }
}
