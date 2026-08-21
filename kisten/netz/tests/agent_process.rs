use orchester_netz::{provider_for_process_name, AgentProcessSnapshot, AgentProcessSource};

struct FixedProcessSource {
    names: Vec<&'static str>,
}

impl AgentProcessSource for FixedProcessSource {
    fn snapshot(&self) -> AgentProcessSnapshot {
        AgentProcessSnapshot::from_process_names(self.names.iter().copied())
    }
}

#[test]
fn provider_matching_is_exact_and_case_insensitive() {
    assert_eq!(provider_for_process_name("codex"), Some("codex"));
    assert_eq!(provider_for_process_name("CODEX.EXE"), Some("codex"));
    assert_eq!(provider_for_process_name("Claude.exe"), Some("claude"));
    assert_eq!(provider_for_process_name("deepseek"), Some("deepseek"));
    assert_eq!(provider_for_process_name("OpenCode.EXE"), Some("opencode"));

    assert_eq!(provider_for_process_name("codex-code-mode-host"), None);
    assert_eq!(provider_for_process_name("codex-helper.exe"), None);
    assert_eq!(provider_for_process_name("my-codex-wrapper"), None);
    assert_eq!(provider_for_process_name("node.exe"), None);
}

#[test]
fn snapshot_keeps_only_provider_instance_counts() {
    let snapshot = AgentProcessSnapshot::from_process_names([
        "codex.exe",
        "CODEX",
        "codex-code-mode-host.exe",
        "claude",
        "node.exe",
        "opencode.exe",
        "opencode.exe",
    ]);

    assert_eq!(snapshot.count("codex"), 2);
    assert_eq!(snapshot.count("claude"), 1);
    assert_eq!(snapshot.count("deepseek"), 0);
    assert_eq!(snapshot.count("opencode"), 2);
    assert_eq!(snapshot.count("node"), 0);
    assert_eq!(
        snapshot.provider_counts(),
        [
            ("claude".to_owned(), 1),
            ("codex".to_owned(), 2),
            ("opencode".to_owned(), 2),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn process_sources_are_injectable_without_exposing_raw_metadata() {
    let source: Box<dyn AgentProcessSource> = Box::new(FixedProcessSource {
        names: vec!["codex.exe", "codex.exe", "claude", "unrelated.exe"],
    });

    let snapshot = source.snapshot();
    assert_eq!(snapshot.count("codex"), 2);
    assert_eq!(snapshot.count("claude"), 1);
    assert_eq!(snapshot.provider_counts().len(), 2);
}
