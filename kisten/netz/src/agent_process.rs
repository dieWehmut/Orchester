//! Redaction-safe external agent process aggregation.
//!
//! This module intentionally accepts only process executable names and retains
//! only known provider identifiers with instance counts. PIDs, arguments,
//! executable paths, and window metadata are discarded before a snapshot is
//! returned to the runtime status layer.

use std::collections::BTreeMap;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Supplies already-aggregated process presence to the runtime status layer.
pub trait AgentProcessSource: Send + Sync {
    fn snapshot(&self) -> AgentProcessSnapshot;
}

/// Read-only operating-system process source used by the production server.
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
        let mut snapshot = AgentProcessSnapshot::from_process_names(
            system
                .processes()
                .values()
                .map(|process| process.name().to_string_lossy()),
        );
        snapshot.include_wsl();
        snapshot
    }
}

/// Aggregated instances of known agent executables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentProcessSnapshot {
    provider_counts: BTreeMap<String, u64>,
}

impl AgentProcessSnapshot {
    fn include_wsl(&mut self) {
        #[cfg(windows)]
        {
            use std::sync::OnceLock;
            // This cache owns no worker. WSL reads run on the caller's blocking
            // snapshot task, so stopping the server monitor also stops polling.
            static CACHE: OnceLock<WslProcessCache> = OnceLock::new();
            let rows = CACHE.get_or_init(WslProcessCache::default).snapshot(|| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                let mut next = Self::default();
                for distro in crate::agent_activity::wsl::running_until(deadline) {
                    if let Some(raw) = crate::agent_activity::wsl::output_until(&["--distribution", &distro, "--exec", "sh", "-c", r#"for p in /proc/[0-9]*/comm; do IFS= read -r n < "$p" 2>/dev/null || continue; case "$n" in codex|claude|opencode|deepseek) printf '%s\n' "$n";; esac; done"#], deadline) {
                        let rows = Self::from_process_names(String::from_utf8_lossy(&raw).lines());
                        for (provider, count) in rows.provider_counts {
                            *next.provider_counts.entry(provider).or_default() += count;
                        }
                    }
                }
                next
            });
            for (provider, count) in rows.provider_counts {
                *self.provider_counts.entry(provider).or_default() += count;
            }
        }
    }

    /// Aggregate exact provider executable names from an arbitrary name source.
    pub fn from_process_names<I, S>(process_names: I) -> Self
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
        Self { provider_counts }
    }

    /// Return the observed instance count for one provider.
    pub fn count(&self, provider: &str) -> u64 {
        self.provider_counts
            .get(provider)
            .copied()
            .unwrap_or_default()
    }

    /// Return deterministic provider-to-count data without raw process metadata.
    pub fn provider_counts(&self) -> &BTreeMap<String, u64> {
        &self.provider_counts
    }
}

#[cfg(any(windows, test))]
#[derive(Default)]
struct WslProcessCache {
    value: std::sync::Mutex<(Option<std::time::Instant>, AgentProcessSnapshot)>,
    refreshing: std::sync::Mutex<()>,
}

#[cfg(any(windows, test))]
impl WslProcessCache {
    fn snapshot(&self, refresh: impl FnOnce() -> AgentProcessSnapshot) -> AgentProcessSnapshot {
        use std::time::{Duration, Instant};
        let current = || {
            self.value
                .lock()
                .ok()
                .map(|value| {
                    let age = value.0.map(|at| at.elapsed());
                    let rows = if age.is_some_and(|age| age < Duration::from_secs(15)) {
                        value.1.clone()
                    } else {
                        AgentProcessSnapshot::default()
                    };
                    (age.is_some_and(|age| age < Duration::from_secs(3)), rows)
                })
                .unwrap_or_default()
        };
        let (fresh, cached) = current();
        if fresh {
            return cached;
        }
        let Ok(_refreshing) = self.refreshing.try_lock() else {
            return cached;
        };
        // A concurrent caller may have completed its refresh after the first read.
        let (fresh, cached) = current();
        if fresh {
            return cached;
        }
        let next = refresh();
        if let Ok(mut value) = self.value.lock() {
            *value = (Some(Instant::now()), next.clone());
        }
        next
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

    #[test]
    fn wsl_cache_refreshes_only_on_snapshot_calls() {
        let cache = WslProcessCache::default();
        let calls = AtomicUsize::new(0);
        let refresh = || {
            calls.fetch_add(1, Ordering::SeqCst);
            AgentProcessSnapshot::from_process_names(["codex", "claude"])
        };
        assert_eq!(cache.snapshot(refresh).count("codex"), 1);
        assert_eq!(cache.snapshot(refresh).count("claude"), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        cache.value.lock().unwrap().0 = Some(Instant::now() - Duration::from_secs(4));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(cache.snapshot(refresh).count("codex"), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn wsl_cache_does_not_wait_for_another_refresh_or_keep_expired_counts() {
        let cache = WslProcessCache::default();
        *cache.value.lock().unwrap() = (
            Some(Instant::now() - Duration::from_secs(4)),
            AgentProcessSnapshot::from_process_names(["codex"]),
        );
        let _busy = cache.refreshing.lock().unwrap();
        assert_eq!(
            cache
                .snapshot(|| panic!("refresh is already running"))
                .count("codex"),
            1
        );
        cache.value.lock().unwrap().0 = Some(Instant::now() - Duration::from_secs(16));
        assert_eq!(
            cache
                .snapshot(|| panic!("refresh is already running"))
                .count("codex"),
            0
        );
    }
}
