use orchester_netz::{AgentProcessSource, SystemAgentProcessSource};
use std::time::Instant;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

fn main() {
    let observed_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("UTC timestamp");
    let started = Instant::now();
    let snapshot = SystemAgentProcessSource.snapshot();
    println!(
        "{}",
        serde_json::json!({
            "observed_at_utc": observed_at,
            "operating_system": std::env::consts::OS,
            "production_source": "SystemAgentProcessSource",
            "elapsed_ms": started.elapsed().as_millis(),
            "provider_counts": snapshot.provider_counts(),
            "counts_are_aggregated_across_sources": true,
            "source_availability_and_degradation": "not exposed by the production snapshot API",
            "scope": "only existing process executable names; no agent or model started"
        })
    );
}
