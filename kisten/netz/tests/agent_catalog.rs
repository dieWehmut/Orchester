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
    assert_eq!(names, vec!["claude", "codex", "opencode"]);
    assert!(!names.contains(&"mock"));
    assert!(registry.get("mock").is_none());

    let codex = catalog
        .agents
        .iter()
        .find(|agent| agent.id == "codex")
        .expect("real codex agent");
    assert_eq!(codex.task_kinds, vec!["code", "review", "chat"]);
    assert!(codex.supports_resume);
    assert!(codex.streaming);

    let json = serde_json::to_string(&catalog).expect("catalog JSON");
    assert!(!json.contains("command"));
    assert!(!json.contains("PATH"));

    let value: serde_json::Value = serde_json::from_str(&json).expect("catalog JSON value");
    let serialized_codex = &value["agents"][1];
    assert_eq!(serialized_codex["id"], "codex");
    assert_eq!(
        serialized_codex["task_kinds"],
        serde_json::json!(["code", "review", "chat"])
    );
    assert_eq!(serialized_codex["supports_resume"], true);
    assert_eq!(serialized_codex["streaming"], true);
    // Installation differs across developer machines and CI; verify the DTO's
    // serialization without assuming that any CLI is present on PATH.
    let availability = match codex.availability {
        AgentAvailabilityDto::Available => "available",
        AgentAvailabilityDto::Missing => "missing",
        AgentAvailabilityDto::Unknown => "unknown",
    };
    assert_eq!(serialized_codex["availability"], availability);
}
