//! Redaction-safe external agent process aggregation.
//!
//! Only known executable names, counts and observation quality leave this
//! module. PIDs, arguments, paths and WSL distribution names remain private.

use std::collections::BTreeMap;

use orchester_protokoll::{
    AgentProcessEnvironment, AgentProcessSourceState, AgentProcessSourceStatusDto,
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub trait AgentProcessSource: Send + Sync {
    fn snapshot(&self) -> AgentProcessSnapshot;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAgentProcessSource;

impl AgentProcessSource for SystemAgentProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
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
            AgentProcessSnapshot::from_source(
                environment,
                system
                    .processes()
                    .values()
                    .map(|process| process.name().to_string_lossy()),
                observed_now(),
            )
        };
        snapshot.include_wsl();
        snapshot
    }
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
    fn include_wsl(&mut self) {
        #[cfg(windows)]
        {
            use std::sync::OnceLock;
            static CACHE: OnceLock<WslProcessCache> = OnceLock::new();
            self.extend(
                CACHE
                    .get_or_init(WslProcessCache::default)
                    .snapshot(read_wsl_processes),
            );
        }
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
    distributions: BTreeMap<String, Result<BTreeMap<String, u64>, &'static str>>,
    observed_at: String,
}

#[cfg(windows)]
fn read_wsl_processes() -> WslProcessRead {
    use crate::agent_activity::wsl::{output_result_until, running_result_until};
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let enumeration = running_result_until(deadline).map_err(|error| error.as_str());
    let mut distributions = BTreeMap::new();
    if let Ok(names) = &enumeration {
        for distro in names {
            let result = output_result_until(&["--distribution", distro, "--exec", "sh", "-c", r#"for p in /proc/[0-9]*/comm; do IFS= read -r n < "$p" 2>/dev/null || continue; case "$n" in codex|claude|opencode|deepseek) printf '%s\n' "$n";; esac; done"#], deadline)
                .map(|raw| count_names(String::from_utf8_lossy(&raw).lines()))
                .map_err(|error| error.as_str());
            distributions.insert(distro.clone(), result);
        }
    }
    WslProcessRead {
        enumeration,
        distributions,
        observed_at: observed_now(),
    }
}

#[cfg(any(windows, test))]
struct WslProcessRows {
    enumeration: ProcessObservation,
    distributions: BTreeMap<String, ProcessObservation>,
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
        }
    }
}

#[cfg(any(windows, test))]
impl WslProcessRows {
    fn reconcile(&self, read: WslProcessRead) -> Self {
        let mut distributions = BTreeMap::new();
        let enumeration = match read.enumeration {
            Ok(names) => {
                for name in names {
                    let source = match read.distributions.get(&name) {
                        Some(Ok(counts)) => ProcessObservation::available(
                            AgentProcessEnvironment::Wsl,
                            counts.clone(),
                            read.observed_at.clone(),
                        ),
                        result => {
                            let failure = result
                                .and_then(|result| result.as_ref().err().copied())
                                .unwrap_or("unavailable");
                            self.distributions
                                .get(&name)
                                .map(|previous| previous.stale(failure))
                                .unwrap_or_else(|| {
                                    ProcessObservation::unavailable(
                                        AgentProcessEnvironment::Wsl,
                                        failure,
                                    )
                                })
                        }
                    };
                    distributions.insert(name, source);
                }
                // Explicit successful enumeration distinguishes no running WSL
                // distributions from an enumeration failure.
                ProcessObservation::available(
                    AgentProcessEnvironment::Wsl,
                    BTreeMap::new(),
                    read.observed_at,
                )
            }
            Err(failure) => {
                distributions.extend(
                    self.distributions
                        .iter()
                        .map(|(name, previous)| (name.clone(), previous.stale(failure))),
                );
                ProcessObservation::unavailable(AgentProcessEnvironment::Wsl, failure)
            }
        };
        Self {
            enumeration,
            distributions,
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
            .chain(self.distributions.values())
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
                .map(|(name, result)| ((*name).to_owned(), result.map(count_names)))
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
}
