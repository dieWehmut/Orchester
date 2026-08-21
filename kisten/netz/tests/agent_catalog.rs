use orchester_netz::{agent_catalog_response, AgentAvailabilityDto};
use orchester_verzeichnis::Registry;

#[test]
fn agent_catalog_projects_registry_without_commands_or_paths() {
    let mut registry = Registry::new();
    registry.register_builtins();

    let catalog = agent_catalog_response(&registry);

    assert_eq!(catalog.schema_version, 1);
    let names: Vec<_> = catalog
        .agents
        .iter()
        .map(|agent| agent.id.as_str())
        .collect();
    assert_eq!(names, vec!["claude", "codex", "mock", "opencode"]);

    let mock = catalog
        .agents
        .iter()
        .find(|agent| agent.id == "mock")
        .expect("mock agent");
    assert_eq!(mock.task_kinds, vec!["chat"]);
    assert!(!mock.supports_resume);
    assert!(mock.streaming);
    assert_eq!(mock.availability, AgentAvailabilityDto::Available);

    let json = serde_json::to_string(&catalog).expect("catalog JSON");
    assert!(!json.contains("command"));
    assert!(!json.contains("PATH"));

    let value: serde_json::Value = serde_json::from_str(&json).expect("catalog JSON value");
    assert_eq!(value["agents"][2]["availability"], "available");
}
