//! Redaction-safe external agent process aggregation.
//!
//! This module intentionally accepts only process executable names and retains
//! only known provider identifiers with instance counts. PIDs, arguments,
//! executable paths, and window metadata are discarded before a snapshot is
//! returned to the runtime status layer.

use std::collections::BTreeMap;

/// Aggregated instances of known agent executables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentProcessSnapshot {
    provider_counts: BTreeMap<String, u64>,
}

impl AgentProcessSnapshot {
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
