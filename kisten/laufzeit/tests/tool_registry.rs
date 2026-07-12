use orchester_laufzeit::harness::tools::{
    BoundToolCall, ToolRegistry, ToolRegistryError, ToolSchema,
};
use orchester_modell::ToolDefinition;
use serde_json::json;

fn schema(name: &str, version: u16) -> ToolSchema {
    ToolSchema::new(
        name,
        version,
        ToolDefinition {
            name: name.into(),
            description: format!("{name} description"),
            parameters: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "additionalProperties": false
            }),
        },
    )
    .unwrap()
}

#[test]
fn registry_catalog_and_binding_share_a_stable_generation() {
    let registry = ToolRegistry::new(vec![schema("read_file", 1)]).unwrap();
    let catalog = registry.catalog();
    assert_eq!(catalog.definitions().len(), 1);
    assert_eq!(catalog.generation(), registry.generation());
    assert_eq!(catalog.schema_version("read_file"), Some(1));

    let call = registry
        .bind(
            catalog.generation(),
            "read_file",
            1,
            r#"{"path":"src/lib.rs"}"#,
        )
        .unwrap();
    assert_eq!(call.name(), "read_file");
    assert_eq!(call.schema_version(), 1);
    assert_eq!(call.registry_generation(), registry.generation());
    assert_eq!(
        call.arguments().get("path").and_then(|v| v.as_str()),
        Some("src/lib.rs")
    );
    assert!(call.is_current(&registry));
}

#[test]
fn stale_or_unknown_calls_never_bind() {
    let mut registry = ToolRegistry::new(vec![schema("read_file", 1)]).unwrap();
    let old_generation = registry.generation().to_owned();
    let old = registry
        .bind(&old_generation, "read_file", 1, r#"{"path":"a"}"#)
        .unwrap();
    registry.replace(vec![schema("read_file", 2)]).unwrap();
    assert!(!old.is_current(&registry));
    assert!(matches!(
        registry.bind(&old_generation, "read_file", 1, r#"{"path":"a"}"#),
        Err(ToolRegistryError::GenerationMismatch)
    ));
    assert!(matches!(
        registry.bind(registry.generation(), "missing", 1, "{}"),
        Err(ToolRegistryError::UnknownTool)
    ));
    assert!(matches!(
        registry.bind(registry.generation(), "read_file", 1, r#"{"path":"a"}"#),
        Err(ToolRegistryError::SchemaVersionMismatch)
    ));
}

#[test]
fn registry_rejects_duplicate_or_malformed_definitions_and_bounds_arguments() {
    assert!(matches!(
        ToolRegistry::new(vec![schema("read_file", 1), schema("read_file", 1)]),
        Err(ToolRegistryError::DuplicateTool)
    ));
    assert!(matches!(
        ToolSchema::new(
            "bad\nname",
            1,
            ToolDefinition {
                name: "bad\nname".into(),
                description: "description".into(),
                parameters: json!({}),
            },
        ),
        Err(ToolRegistryError::InvalidDefinition)
    ));

    let registry = ToolRegistry::new(vec![schema("read_file", 1)]).unwrap();
    assert!(matches!(
        registry.bind(registry.generation(), "read_file", 1, "x"),
        Err(ToolRegistryError::InvalidArguments)
    ));
    let oversized = format!(r#"{{"path":"{}"}}"#, "x".repeat(70_000));
    assert!(matches!(
        registry.bind(registry.generation(), "read_file", 1, &oversized),
        Err(ToolRegistryError::ArgumentsTooLarge)
    ));
}

#[test]
fn bound_call_debug_does_not_echo_arguments() {
    let registry = ToolRegistry::new(vec![schema("read_file", 1)]).unwrap();
    let secret = "sk-registry-secret-should-not-appear";
    let call: BoundToolCall = registry
        .bind(
            registry.generation(),
            "read_file",
            1,
            &format!(r#"{{"path":"{secret}"}}"#),
        )
        .unwrap();
    let debug = format!("{call:?}");
    assert!(!debug.contains(secret));
    assert!(debug.contains("arguments_bytes"));
}
