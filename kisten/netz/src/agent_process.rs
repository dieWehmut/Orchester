//! Redaction-safe external agent process aggregation.
//!
//! Only provider identifiers, counts and observation quality leave this
//! module. PIDs, arguments, paths and WSL distribution names remain private.

use std::collections::BTreeMap;

use orchester_protokoll::{
    AgentProcessEnvironment, AgentProcessSourceState, AgentProcessSourceStatusDto,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[path = "agent_process/identity.rs"]
mod identity;

pub trait AgentProcessSource: Send + Sync {
    fn snapshot(&self) -> AgentProcessSnapshot;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAgentProcessSource;

impl AgentProcessSource for SystemAgentProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        use std::sync::OnceLock;
        static READER: OnceLock<ProcessReadGate> = OnceLock::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        READER
            .get_or_init(ProcessReadGate::default)
            .read(deadline, move || read_system_processes(deadline))
    }
}

#[derive(Default)]
struct ProcessReadGate {
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ProcessReadGate {
    fn read(
        &self,
        deadline: std::time::Instant,
        read: impl FnOnce() -> AgentProcessSnapshot + Send + 'static,
    ) -> AgentProcessSnapshot {
        use std::{
            sync::{atomic::Ordering, mpsc},
            time::Instant,
        };
        if Instant::now() >= deadline
            || self
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return AgentProcessSnapshot::unavailable_system();
        }
        let guard = ProcessReadGuard(std::sync::Arc::clone(&self.busy));
        let (sender, receiver) = mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("agent-process-reader".to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read));
                // Hold the gate until the synchronous OS read really exits. A
                // timed-out caller cannot start another lingering OS worker.
                drop(guard);
                let _ = sender.send(result);
            })
            .is_err()
        {
            return AgentProcessSnapshot::unavailable_system();
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return AgentProcessSnapshot::unavailable_system();
        };
        match receiver.recv_timeout(remaining) {
            Ok(Ok(snapshot)) if Instant::now() < deadline => snapshot,
            // Dropping the receiver discards late safe results. No worker can
            // overwrite the fleet after this caller has reported unavailable.
            _ => AgentProcessSnapshot::unavailable_system(),
        }
    }
}

struct ProcessReadGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for ProcessReadGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}

fn read_system_processes(deadline: std::time::Instant) -> AgentProcessSnapshot {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let environment = if cfg!(windows) {
        AgentProcessEnvironment::Windows
    } else {
        AgentProcessEnvironment::Local
    };
    // A running scanner must itself be observable in a successful OS
    // enumeration. An empty OS table is not evidence of zero providers.
    let mut snapshot = if system.processes().is_empty() {
        AgentProcessSnapshot::unavailable(environment)
    } else {
        read_system_identities(&mut system, environment, deadline)
    };
    snapshot.include_wsl(deadline);
    snapshot
}

fn read_system_identities(
    system: &mut System,
    environment: AgentProcessEnvironment,
    deadline: std::time::Instant,
) -> AgentProcessSnapshot {
    use sysinfo::UpdateKind;
    let candidates: Vec<_> = system
        .processes()
        .iter()
        .filter(|(_, process)| {
            process_is_live(process.status())
                && identity::is_candidate(&process.name().to_string_lossy())
        })
        .map(|(pid, _)| *pid)
        .take(513)
        .collect();
    if candidates.len() > 512 || std::time::Instant::now() >= deadline {
        return AgentProcessSnapshot::unavailable(environment);
    }
    // Enumerate names first; expensive executable/command reads are restricted
    // to possible runtimes and branded main services.
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    let command_candidates: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|pid| {
            system.process(*pid).is_some_and(|process| {
                identity::classify(
                    &process.name().to_string_lossy(),
                    &process
                        .exe()
                        .map(|value| value.to_string_lossy())
                        .unwrap_or_default(),
                    &[],
                )
                .is_none()
            })
        })
        .collect();
    if std::time::Instant::now() >= deadline {
        return AgentProcessSnapshot::unavailable(environment);
    }
    #[cfg(not(target_os = "linux"))]
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&command_candidates),
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
    let mut command_arguments = BTreeMap::new();
    for pid in &command_candidates {
        if std::time::Instant::now() >= deadline {
            return AgentProcessSnapshot::unavailable(environment);
        }
        #[cfg(target_os = "linux")]
        let arguments = read_linux_args(pid.as_u32());
        #[cfg(not(target_os = "linux"))]
        let arguments = system
            .process(*pid)
            .map(|process| identity::bounded_args(process.cmd()))
            .unwrap_or_else(|| (Vec::new(), false));
        command_arguments.insert(*pid, arguments);
    }
    let relative_candidates: Vec<_> = command_candidates
        .iter()
        .copied()
        .filter(|pid| {
            command_arguments
                .get(pid)
                .is_some_and(|(args, intact)| *intact && identity::relative_entry(args))
        })
        .collect();
    if std::time::Instant::now() >= deadline {
        return AgentProcessSnapshot::unavailable(environment);
    }
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&relative_candidates),
        true,
        ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
    );
    let mut rows = Vec::new();
    let mut complete = true;
    for pid in candidates {
        let Some(process) = system.process(pid) else {
            continue;
        };
        let name = process.name().to_string_lossy();
        let exe = process
            .exe()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        let (mut args, intact) = command_arguments
            .remove(&pid)
            .unwrap_or_else(|| (Vec::new(), true));
        if !process_is_live(process.status()) {
            continue;
        }
        if !intact {
            complete = false;
            continue;
        }
        if identity::relative_entry(&args) {
            let cwd = process
                .cwd()
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            if cwd.is_empty() {
                complete = false;
                continue;
            }
            identity::resolve_relative_entry(&mut args, &cwd);
        }
        match identity::classify(&name, &exe, &args) {
            Some(provider) => rows.push((
                pid.as_u32(),
                process.parent().map(|pid| pid.as_u32()),
                provider,
                identity::instance_key(&name, &exe, &args),
            )),
            None if exe.is_empty() || args.is_empty() => complete = false,
            None => {}
        }
    }
    complete &= std::time::Instant::now() < deadline;
    AgentProcessSnapshot::from_observations(identity_observations(
        environment,
        IdentityCounts {
            counts: identity::count_instances(&rows),
            complete,
        },
        observed_now(),
    ))
}

#[cfg(target_os = "linux")]
fn read_linux_args(pid: u32) -> (Vec<String>, bool) {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(format!("/proc/{pid}/cmdline")) else {
        return (Vec::new(), false);
    };
    let mut bytes = Vec::new();
    if file
        .take(identity::MAX_METADATA_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return (Vec::new(), false);
    }
    let mut intact = bytes.len() <= identity::MAX_METADATA_BYTES && bytes.last() == Some(&0);
    bytes.truncate(identity::MAX_METADATA_BYTES);
    let mut fields: Vec<_> = bytes.split(|byte| *byte == 0).collect();
    if bytes.last() != Some(&0) {
        intact = false;
    }
    fields.pop(); // trailing NUL or truncated final argument
    let args: Vec<_> = fields
        .into_iter()
        .map(|field| std::ffi::OsString::from(String::from_utf8_lossy(field).into_owned()))
        .collect();
    let (args, bounded) = identity::bounded_args(&args);
    (args, intact && bounded)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessObservation {
    environment: AgentProcessEnvironment,
    status: AgentProcessSourceState,
    provider_counts: BTreeMap<String, u64>,
    observed_at: Option<String>,
    failure: Option<&'static str>,
}

impl ProcessObservation {
    fn available(
        environment: AgentProcessEnvironment,
        provider_counts: BTreeMap<String, u64>,
        observed_at: String,
    ) -> Self {
        Self {
            environment,
            status: AgentProcessSourceState::Available,
            provider_counts,
            observed_at: Some(observed_at),
            failure: None,
        }
    }

    fn unavailable(environment: AgentProcessEnvironment, failure: &'static str) -> Self {
        Self {
            environment,
            status: AgentProcessSourceState::Unavailable,
            provider_counts: BTreeMap::new(),
            observed_at: None,
            failure: Some(failure),
        }
    }

    #[cfg(any(windows, test))]
    fn stale(&self, failure: &'static str) -> Self {
        if self.observed_at.is_none() {
            return Self::unavailable(self.environment, failure);
        }
        Self {
            status: AgentProcessSourceState::Stale,
            failure: Some(failure),
            ..self.clone()
        }
    }
}

/// Current successful counts plus independent source quality. Stale data is
/// retained only for explicitly labelled last-successful observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProcessSnapshot {
    provider_counts: BTreeMap<String, u64>,
    sources: Vec<ProcessObservation>,
}

impl Default for AgentProcessSnapshot {
    fn default() -> Self {
        Self::from_process_names(std::iter::empty::<&str>())
    }
}

impl AgentProcessSnapshot {
    fn include_wsl(&mut self, deadline: std::time::Instant) {
        #[cfg(windows)]
        {
            use std::sync::OnceLock;
            static CACHE: OnceLock<WslProcessCache> = OnceLock::new();
            self.extend(
                CACHE
                    .get_or_init(WslProcessCache::default)
                    .snapshot(|| read_wsl_processes(deadline)),
            );
        }
        #[cfg(not(windows))]
        let _ = deadline;
    }

    /// Aggregate a successful, already-filtered source. The source is captured
    /// once so reusing a snapshot does not invent newer observation timestamps.
    pub fn from_process_names<I, S>(process_names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_source(
            AgentProcessEnvironment::Local,
            process_names,
            observed_now(),
        )
    }

    pub fn from_source<I, S>(
        environment: AgentProcessEnvironment,
        process_names: I,
        observed_at: impl Into<String>,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_observations(vec![ProcessObservation::available(
            environment,
            count_names(process_names),
            observed_at.into(),
        )])
    }

    /// A failed read carries no current count, including no implied zero.
    pub fn unavailable(environment: AgentProcessEnvironment) -> Self {
        Self::from_observations(vec![ProcessObservation::unavailable(
            environment,
            "unavailable",
        )])
    }

    pub(crate) fn unavailable_system() -> Self {
        if cfg!(windows) {
            let mut snapshot = Self::unavailable(AgentProcessEnvironment::Windows);
            snapshot.extend(Self::unavailable(AgentProcessEnvironment::Wsl));
            snapshot
        } else {
            Self::unavailable(AgentProcessEnvironment::Local)
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.sources.extend(other.sources);
        self.recount();
    }

    fn from_observations(sources: Vec<ProcessObservation>) -> Self {
        let mut snapshot = Self {
            provider_counts: BTreeMap::new(),
            sources,
        };
        snapshot.recount();
        snapshot
    }

    fn recount(&mut self) {
        self.provider_counts.clear();
        for source in &self.sources {
            if source.status != AgentProcessSourceState::Available {
                continue;
            }
            for (provider, count) in &source.provider_counts {
                *self.provider_counts.entry(provider.clone()).or_default() += count;
            }
        }
    }

    pub fn count(&self, provider: &str) -> u64 {
        self.provider_counts
            .get(provider)
            .copied()
            .unwrap_or_default()
    }

    pub fn provider_counts(&self) -> &BTreeMap<String, u64> {
        &self.provider_counts
    }

    pub fn is_complete(&self) -> bool {
        !self.sources.is_empty()
            && self
                .sources
                .iter()
                .all(|source| source.status == AgentProcessSourceState::Available)
    }

    pub fn source_statuses(&self, provider: &str) -> Vec<AgentProcessSourceStatusDto> {
        self.sources
            .iter()
            .map(|source| AgentProcessSourceStatusDto {
                environment: source.environment,
                status: source.status,
                count: (source.status != AgentProcessSourceState::Unavailable).then(|| {
                    source
                        .provider_counts
                        .get(provider)
                        .copied()
                        .unwrap_or_default()
                }),
                observed_at: source.observed_at.clone(),
            })
            .collect()
    }

    /// Safe diagnostic categories for acceptance probes; no command text or
    /// operating-system error output is returned.
    pub fn source_failures(&self) -> Vec<(AgentProcessEnvironment, &'static str)> {
        self.sources
            .iter()
            .filter_map(|source| source.failure.map(|failure| (source.environment, failure)))
            .collect()
    }
}

fn count_names<I, S>(process_names: I) -> BTreeMap<String, u64>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut provider_counts = BTreeMap::new();
    for process_name in process_names {
        let Some(provider) = provider_for_process_name(process_name.as_ref()) else {
            continue;
        };
        *provider_counts.entry(provider.to_owned()).or_default() += 1;
    }
    provider_counts
}

fn observed_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamp")
}

#[cfg(any(windows, test))]
struct WslProcessRead {
    enumeration: Result<Vec<String>, &'static str>,
    distributions: BTreeMap<String, Result<IdentityCounts, &'static str>>,
    observed_at: String,
}

#[cfg(windows)]
fn read_wsl_processes(deadline: std::time::Instant) -> WslProcessRead {
    use crate::agent_activity::wsl::{output_result_until, running_result_until};
    let enumeration = running_result_until(deadline).map_err(|error| error.as_str());
    let mut distributions = BTreeMap::new();
    if let Ok(names) = &enumeration {
        for distro in names {
            let result = output_result_until(
                &[
                    "--distribution",
                    distro,
                    "--exec",
                    "python3",
                    "-c",
                    include_str!("agent_process/wsl_query.py"),
                    identity::RULES_JSON,
                ],
                deadline,
            )
            .map_err(|error| error.as_str())
            .and_then(|raw| parse_identity_counts(&raw));
            distributions.insert(distro.clone(), result);
        }
    }
    WslProcessRead {
        enumeration,
        distributions,
        observed_at: observed_now(),
    }
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct IdentityCounts {
    counts: BTreeMap<String, u64>,
    complete: bool,
}

fn process_is_live(status: sysinfo::ProcessStatus) -> bool {
    !matches!(
        status,
        sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
    )
}

fn identity_observations(
    environment: AgentProcessEnvironment,
    result: IdentityCounts,
    observed_at: String,
) -> Vec<ProcessObservation> {
    let mut sources = Vec::new();
    let counts: BTreeMap<_, _> = result
        .counts
        .into_iter()
        .filter(|(_, count)| result.complete || *count > 0)
        .collect();
    if result.complete || !counts.is_empty() {
        sources.push(ProcessObservation::available(
            environment,
            counts,
            observed_at,
        ));
    }
    if !result.complete {
        sources.push(ProcessObservation::unavailable(
            environment,
            "metadata_unavailable",
        ));
    }
    sources
}

#[cfg(any(windows, test))]
fn parse_identity_counts(raw: &[u8]) -> Result<IdentityCounts, &'static str> {
    if raw.len() > 4096 {
        return Err("output_limit");
    }
    let result: IdentityCounts = serde_json::from_slice(raw).map_err(|_| "read_failed")?;
    if result.counts.keys().any(|provider| {
        !matches!(
            provider.as_str(),
            "claude"
                | "codex"
                | "opencode"
                | "deepseek"
                | "gemini"
                | "antigravity"
                | "cursor"
                | "qwen"
                | "openclaw"
                | "workbuddy"
        )
    }) || result.counts.values().any(|count| *count > 512)
    {
        return Err("read_failed");
    }
    Ok(result)
}

#[cfg(any(windows, test))]
struct WslProcessRows {
    enumeration: ProcessObservation,
    distributions: BTreeMap<String, Vec<ProcessObservation>>,
    last_successful: BTreeMap<String, ProcessObservation>,
}

#[cfg(any(windows, test))]
impl Default for WslProcessRows {
    fn default() -> Self {
        Self {
            enumeration: ProcessObservation::unavailable(
                AgentProcessEnvironment::Wsl,
                "not_observed",
            ),
            distributions: BTreeMap::new(),
            last_successful: BTreeMap::new(),
        }
    }
}

#[cfg(any(windows, test))]
impl WslProcessRows {
    fn reconcile(&self, read: WslProcessRead) -> Self {
        let mut distributions = BTreeMap::new();
        let mut last_successful = BTreeMap::new();
        let enumeration = match read.enumeration {
            Ok(names) => {
                for name in names {
                    let previous = self.last_successful.get(&name);
                    let sources = match read.distributions.get(&name) {
                        Some(Ok(result)) => {
                            let mut sources = identity_observations(
                                AgentProcessEnvironment::Wsl,
                                result.clone(),
                                read.observed_at.clone(),
                            );
                            if result.complete {
                                last_successful.insert(name.clone(), sources[0].clone());
                            } else if let Some(previous) = previous {
                                last_successful.insert(name.clone(), previous.clone());
                                sources.push(previous.stale("metadata_unavailable"));
                            }
                            sources
                        }
                        result => {
                            let failure = result
                                .and_then(|result| result.as_ref().err().copied())
                                .unwrap_or("unavailable");
                            if let Some(previous) = previous {
                                last_successful.insert(name.clone(), previous.clone());
                            }
                            self.distributions
                                .get(&name)
                                .map(|previous| {
                                    previous
                                        .iter()
                                        .map(|source| source.stale(failure))
                                        .collect()
                                })
                                .unwrap_or_else(|| {
                                    vec![ProcessObservation::unavailable(
                                        AgentProcessEnvironment::Wsl,
                                        failure,
                                    )]
                                })
                        }
                    };
                    distributions.insert(name, sources);
                }
                ProcessObservation::available(
                    AgentProcessEnvironment::Wsl,
                    BTreeMap::new(),
                    read.observed_at,
                )
            }
            Err(failure) => {
                distributions.extend(self.distributions.iter().map(|(name, previous)| {
                    (
                        name.clone(),
                        previous
                            .iter()
                            .map(|source| source.stale(failure))
                            .collect(),
                    )
                }));
                last_successful.clone_from(&self.last_successful);
                ProcessObservation::unavailable(AgentProcessEnvironment::Wsl, failure)
            }
        };
        Self {
            enumeration,
            distributions,
            last_successful,
        }
    }

    fn snapshot(&self, stale: bool) -> AgentProcessSnapshot {
        // Enumeration is a zero-count process source only when it confirms no
        // running distributions. Otherwise display the actual distro reads.
        let marker = (self.distributions.is_empty()
            || self.enumeration.status == AgentProcessSourceState::Unavailable)
            .then_some(&self.enumeration);
        let observations = marker
            .into_iter()
            .chain(self.distributions.values().flatten())
            .map(|source| {
                if stale {
                    source.stale("refresh_in_progress")
                } else {
                    source.clone()
                }
            })
            .collect();
        AgentProcessSnapshot::from_observations(observations)
    }
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct WslProcessCache {
    value: std::sync::Mutex<(Option<std::time::Instant>, WslProcessRows)>,
    refreshing: std::sync::Mutex<()>,
}

#[cfg(any(windows, test))]
impl WslProcessCache {
    fn snapshot(&self, refresh: impl FnOnce() -> WslProcessRead) -> AgentProcessSnapshot {
        use std::time::{Duration, Instant};
        let current = || {
            self.value
                .lock()
                .map(|value| {
                    let fresh = value
                        .0
                        .is_some_and(|at| at.elapsed() < Duration::from_secs(3));
                    (fresh, value.1.snapshot(!fresh))
                })
                .unwrap_or_else(|_| {
                    (
                        false,
                        AgentProcessSnapshot::unavailable(AgentProcessEnvironment::Wsl),
                    )
                })
        };
        let (fresh, cached) = current();
        if fresh {
            return cached;
        }
        let Ok(_refreshing) = self.refreshing.try_lock() else {
            return cached;
        };
        let (fresh, cached) = current();
        if fresh {
            return cached;
        }
        let read = refresh();
        let Ok(mut value) = self.value.lock() else {
            return AgentProcessSnapshot::unavailable(AgentProcessEnvironment::Wsl);
        };
        value.1 = value.1.reconcile(read);
        value.0 = Some(Instant::now());
        value.1.snapshot(false)
    }
}

/// Match only primary provider executable names, never helpers or wrappers.
pub fn provider_for_process_name(process_name: &str) -> Option<&'static str> {
    let normalized = process_name.trim().to_ascii_lowercase();
    let executable = normalized.strip_suffix(".exe").unwrap_or(&normalized);
    match executable {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "deepseek" => Some("deepseek"),
        "opencode" => Some("opencode"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };

    fn read(distributions: &[(&str, Result<&[&str], &'static str>)]) -> WslProcessRead {
        WslProcessRead {
            enumeration: Ok(distributions
                .iter()
                .map(|(name, _)| (*name).to_owned())
                .collect()),
            distributions: distributions
                .iter()
                .map(|(name, result)| {
                    (
                        (*name).to_owned(),
                        result.map(|names| IdentityCounts {
                            counts: count_names(names),
                            complete: true,
                        }),
                    )
                })
                .collect(),
            observed_at: "2026-11-10T05:00:00Z".to_owned(),
        }
    }

    fn expire(cache: &WslProcessCache, seconds: u64) {
        cache.value.lock().unwrap().0 = Some(Instant::now() - Duration::from_secs(seconds));
    }

    #[test]
    fn wsl_cache_refreshes_only_on_snapshot_calls() {
        let cache = WslProcessCache::default();
        let calls = AtomicUsize::new(0);
        let refresh = || {
            calls.fetch_add(1, Ordering::SeqCst);
            read(&[("A", Ok(&["codex", "claude"]))])
        };
        assert_eq!(cache.snapshot(refresh).count("codex"), 1);
        assert_eq!(cache.snapshot(refresh).count("claude"), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        expire(&cache, 4);
        assert_eq!(cache.snapshot(refresh).count("codex"), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn wsl_cache_busy_or_expired_marks_old_counts_stale_instead_of_live_or_zero_complete() {
        let cache = WslProcessCache::default();
        cache.snapshot(|| read(&[("A", Ok(&["codex"]))]));
        expire(&cache, 4);
        let _busy = cache.refreshing.lock().unwrap();
        for seconds in [4, 16] {
            expire(&cache, seconds);
            let snapshot = cache.snapshot(|| panic!("refresh is already running"));
            assert_eq!(snapshot.count("codex"), 0);
            assert!(!snapshot.is_complete());
            let sources = snapshot.source_statuses("codex");
            assert_eq!(sources[0].status, AgentProcessSourceState::Stale);
            assert_eq!(sources[0].count, Some(1));
            assert_eq!(
                sources[0].observed_at.as_deref(),
                Some("2026-11-10T05:00:00Z")
            );
        }
    }

    #[test]
    fn wsl_failure_preserves_last_success_and_recovery_empty_confirms_zero() {
        let cache = WslProcessCache::default();
        cache.snapshot(|| read(&[("A", Ok(&["codex", "codex"]))]));
        expire(&cache, 4);
        let failed = cache.snapshot(|| WslProcessRead {
            enumeration: Err("timeout"),
            distributions: BTreeMap::new(),
            observed_at: observed_now(),
        });
        assert_eq!(failed.count("codex"), 0);
        assert!(!failed.is_complete());
        assert_eq!(failed.source_statuses("codex")[0].count, None);
        assert_eq!(failed.source_statuses("codex")[1].count, Some(2));
        assert_eq!(failed.source_failures()[0].1, "timeout");
        expire(&cache, 4);
        let recovered = cache.snapshot(|| read(&[]));
        assert!(recovered.is_complete());
        assert_eq!(recovered.count("codex"), 0);
        assert_eq!(recovered.source_statuses("codex").len(), 1);
    }

    #[test]
    fn failed_distro_does_not_discard_other_current_counts() {
        let cache = WslProcessCache::default();
        let snapshot = cache
            .snapshot(|| read(&[("A", Ok(&["codex", "codex"])), ("B", Err("command_failed"))]));
        assert_eq!(snapshot.count("codex"), 2);
        assert!(!snapshot.is_complete());
        assert_eq!(
            snapshot.source_statuses("codex")[1].status,
            AgentProcessSourceState::Unavailable
        );
        assert_eq!(snapshot.source_statuses("codex")[1].count, None);
    }
    #[test]
    fn bounded_identity_query_failure_never_confirms_zero_or_accepts_metadata() {
        assert_eq!(
            parse_identity_counts(br#"{"counts":{},"complete":false}"#),
            Ok(IdentityCounts {
                counts: BTreeMap::new(),
                complete: false
            })
        );
        assert_eq!(
            parse_identity_counts(br#"{"counts":{"gemini":1},"complete":true,"pid":123}"#),
            Err("read_failed")
        );
        assert_eq!(
            parse_identity_counts(br#"{"counts":{"node":1},"complete":true}"#),
            Err("read_failed")
        );
        assert_eq!(
            parse_identity_counts(br#"{"counts":{"gemini":513},"complete":true}"#),
            Err("read_failed")
        );
        assert_eq!(
            parse_identity_counts(br#"{"counts":{},"complete":true}"#),
            Ok(IdentityCounts {
                counts: BTreeMap::new(),
                complete: true
            })
        );
    }
    #[test]
    fn production_reader_discards_late_results_at_the_source_deadline() {
        let gate = ProcessReadGate::default();
        let snapshot = gate.read(Instant::now() + Duration::from_millis(20), || {
            std::thread::sleep(Duration::from_millis(80));
            AgentProcessSnapshot::from_process_names(["codex"])
        });
        assert!(!snapshot.is_complete());
        assert_eq!(snapshot.count("codex"), 0);
        assert_eq!(snapshot.source_statuses("codex")[0].count, None);
    }
    #[test]
    fn production_reader_keeps_one_worker_after_timeout_and_accepts_fresh_recovery() {
        use std::sync::{atomic::Ordering, mpsc, Arc};
        let gate = ProcessReadGate::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_calls = Arc::clone(&calls);
        let (release, blocked) = mpsc::sync_channel(1);
        let timed_out = gate.read(Instant::now() + Duration::from_millis(20), move || {
            worker_calls.fetch_add(1, Ordering::SeqCst);
            let _ = blocked.recv();
            AgentProcessSnapshot::from_process_names(["codex", "codex"])
        });
        assert!(!timed_out.is_complete());
        let extra_calls = Arc::clone(&calls);
        let busy = gate.read(Instant::now() + Duration::from_millis(20), move || {
            extra_calls.fetch_add(1, Ordering::SeqCst);
            AgentProcessSnapshot::default()
        });
        assert!(!busy.is_complete());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        release.send(()).unwrap();
        let wait_until = Instant::now() + Duration::from_secs(1);
        while gate.busy.load(Ordering::Acquire) && Instant::now() < wait_until {
            std::thread::yield_now();
        }
        assert!(!gate.busy.load(Ordering::Acquire));
        let recovered = gate.read(Instant::now() + Duration::from_secs(1), || {
            AgentProcessSnapshot::from_process_names(["claude"])
        });
        assert!(recovered.is_complete());
        assert_eq!(
            recovered.count("codex"),
            0,
            "late result cannot replace the new observation"
        );
        assert_eq!(recovered.count("claude"), 1);
    }
    #[test]
    fn production_reader_releases_its_gate_after_a_synthetic_panic() {
        let gate = ProcessReadGate::default();
        let failed = gate.read(Instant::now() + Duration::from_secs(1), || {
            panic!("synthetic reader panic")
        });
        assert!(!failed.is_complete());
        let recovered = gate.read(
            Instant::now() + Duration::from_secs(1),
            AgentProcessSnapshot::default,
        );
        assert!(recovered.is_complete());
    }
    #[test]
    fn partial_identity_read_preserves_current_positive_counts_with_an_unknown_marker() {
        use orchester_protokoll::AgentActivityState;
        for environment in [
            AgentProcessEnvironment::Local,
            AgentProcessEnvironment::Windows,
            AgentProcessEnvironment::Wsl,
        ] {
            let result =
                parse_identity_counts(br#"{"counts":{"codex":3,"gemini":0},"complete":false}"#)
                    .expect("bounded partial output is usable");
            let snapshot = AgentProcessSnapshot::from_observations(identity_observations(
                environment,
                result,
                observed_now(),
            ));
            assert_eq!(snapshot.count("codex"), 3);
            assert!(!snapshot.is_complete());
            let sources = snapshot.source_statuses("codex");
            assert_eq!(sources.len(), 2);
            assert!(sources
                .iter()
                .all(|source| source.environment == environment));
            assert_eq!(sources[0].status, AgentProcessSourceState::Available);
            assert_eq!(sources[0].count, Some(3));
            assert_eq!(sources[1].status, AgentProcessSourceState::Unavailable);
            assert_eq!(sources[1].count, None);
            assert_eq!(snapshot.count("gemini"), 0);

            let mut registry = orchester_verzeichnis::Registry::new();
            registry.register_builtins();
            let store =
                crate::AgentRuntimeStatusStore::new(crate::agent_status_response(&registry))
                    .unwrap();
            store
                .reconcile_external_processes(&snapshot, "2026-11-10T05:00:01Z")
                .unwrap();
            let fleet = store.snapshot().unwrap();
            for (provider, activity, count) in [
                ("codex", AgentActivityState::Running, 3),
                ("gemini", AgentActivityState::Unknown, 0),
            ] {
                let row = fleet
                    .agents
                    .iter()
                    .find(|agent| agent.provider == provider)
                    .unwrap();
                assert_eq!(row.activity, activity);
                assert_eq!(row.active_windows, count);
            }
        }
    }

    #[test]
    fn incomplete_identity_read_without_positives_never_confirms_zero() {
        for raw in [
            br#"{"counts":{},"complete":false}"#.as_slice(),
            br#"{"counts":{"codex":0},"complete":false}"#.as_slice(),
        ] {
            let snapshot = AgentProcessSnapshot::from_observations(identity_observations(
                AgentProcessEnvironment::Local,
                parse_identity_counts(raw).unwrap(),
                observed_now(),
            ));
            assert_eq!(snapshot.count("codex"), 0);
            assert!(!snapshot.is_complete());
            let sources = snapshot.source_statuses("codex");
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].status, AgentProcessSourceState::Unavailable);
            assert_eq!(sources[0].count, None);
        }
    }

    #[test]
    fn new_monitor_identities_preserve_positive_counts_and_incomplete_sources() {
        use orchester_protokoll::AgentActivityState;
        for environment in [
            AgentProcessEnvironment::Local,
            AgentProcessEnvironment::Windows,
            AgentProcessEnvironment::Wsl,
        ] {
            let snapshot = AgentProcessSnapshot::from_observations(identity_observations(
                environment,
                parse_identity_counts(
                    br#"{"counts":{"qwen":1,"openclaw":2,"workbuddy":1},"complete":false}"#,
                )
                .unwrap(),
                "2026-11-11T06:00:00Z".into(),
            ));
            assert!(!snapshot.is_complete());
            let mut registry = orchester_verzeichnis::Registry::new();
            registry.register_builtins();
            let store =
                crate::AgentRuntimeStatusStore::new(crate::agent_status_response(&registry))
                    .unwrap();
            store
                .reconcile_external_processes(&snapshot, "2026-11-11T06:00:01Z")
                .unwrap();
            let fleet = store.snapshot().unwrap();
            for (provider, count) in [("qwen", 1), ("openclaw", 2), ("workbuddy", 1)] {
                let row = fleet
                    .agents
                    .iter()
                    .find(|row| row.provider == provider)
                    .unwrap();
                assert_eq!(row.activity, AgentActivityState::Running);
                assert_eq!(row.active_windows, count);
                assert_eq!(row.active_sessions, 0);
                assert_eq!(row.active_runs, 0);
                let sources = row.process_sources.as_ref().unwrap();
                assert_eq!(sources.len(), 2);
                assert_eq!(sources[0].count, Some(count));
                assert_eq!(sources[0].status, AgentProcessSourceState::Available);
                assert_eq!(sources[1].count, None);
                assert_eq!(sources[1].status, AgentProcessSourceState::Unavailable);
            }
            assert_eq!(
                fleet
                    .agents
                    .iter()
                    .find(|row| row.provider == "gemini")
                    .unwrap()
                    .activity,
                AgentActivityState::Unknown
            );
        }
    }

    #[test]
    fn partial_wsl_refresh_keeps_current_counts_separate_from_cached_stale_counts() {
        let cache = WslProcessCache::default();
        cache.snapshot(|| read(&[("A", Ok(&["codex", "codex"]))]));
        expire(&cache, 4);
        let partial_read = || WslProcessRead {
            enumeration: Ok(vec!["A".to_owned()]),
            distributions: BTreeMap::from([(
                "A".to_owned(),
                Ok(IdentityCounts {
                    counts: BTreeMap::from([("codex".to_owned(), 1)]),
                    complete: false,
                }),
            )]),
            observed_at: "2026-11-10T05:00:02Z".to_owned(),
        };
        for _ in 0..2 {
            let partial = cache.snapshot(partial_read);
            assert_eq!(partial.count("codex"), 1);
            assert!(!partial.is_complete());
            let sources = partial.source_statuses("codex");
            assert_eq!(sources.len(), 3);
            assert_eq!(sources[0].status, AgentProcessSourceState::Available);
            assert_eq!(sources[0].count, Some(1));
            assert_eq!(sources[1].status, AgentProcessSourceState::Unavailable);
            assert_eq!(sources[1].count, None);
            assert_eq!(sources[2].status, AgentProcessSourceState::Stale);
            assert_eq!(sources[2].count, Some(2));
            assert_eq!(
                sources[2].observed_at.as_deref(),
                Some("2026-11-10T05:00:00Z")
            );
            expire(&cache, 4);
        }
        {
            let _busy = cache.refreshing.lock().unwrap();
            let stale = cache.snapshot(|| panic!("refresh is already running"));
            assert_eq!(stale.count("codex"), 0);
            assert!(!stale.is_complete());
            assert!(stale
                .source_statuses("codex")
                .iter()
                .all(|source| source.status != AgentProcessSourceState::Available));
        }
        let failed = cache.snapshot(|| read(&[("A", Err("command_failed"))]));
        assert_eq!(failed.count("codex"), 0);
        assert!(!failed.is_complete());
        expire(&cache, 4);
        let recovered = cache.snapshot(|| read(&[("A", Ok(&[]))]));
        assert_eq!(recovered.count("codex"), 0);
        assert!(recovered.is_complete());
        assert_eq!(recovered.source_statuses("codex").len(), 1);
    }
}
