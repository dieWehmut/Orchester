use orchester_netz::{AgentProcessSource, SystemAgentProcessSource};
use std::time::Instant;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

fn main() {
    let observed_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamp");
    let started = Instant::now();
    let snapshot = SystemAgentProcessSource.snapshot();
    let sources = [
        "codex",
        "claude",
        "opencode",
        "deepseek",
        "gemini",
        "antigravity",
        "cursor",
    ]
    .into_iter()
    .map(|provider| (provider, snapshot.source_statuses(provider)))
    .collect::<std::collections::BTreeMap<_, _>>();
    println!(
        "{}",
        serde_json::json!({
            "observed_at_utc": observed_at,
            "operating_system": std::env::consts::OS,
            "production_source": "SystemAgentProcessSource",
            "elapsed_ms": started.elapsed().as_millis(),
            "provider_counts": snapshot.provider_counts(),
            "counts_are_aggregated_across_sources": true,
            "sources_complete": snapshot.is_complete(),
            "source_statuses_by_provider": sources,
            "source_failure_categories": snapshot.source_failures(),
            "scope": "read-only process identity matching; no agent or model started"
        })
    );
}
