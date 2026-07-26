use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader};

fn configured(source: &str) -> orchester_laufzeit::harness::config::UserConfig {
    ConfigLoader::test().load_user(source).expect("user config")
}

#[test]
fn named_profile_selection_changes_only_the_returned_effective_config() {
    let config = configured(
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_reasoning_effort": "medium",
            "service_tier": "priority",
            "model_providers": {
                "OpenAI": {
                    "name": "OpenAI API",
                    "base_url": "https://example.test/v1",
                    "wire_api": "responses"
                }
            },
            "model_profiles": {
                "thorough": {
                    "model_provider": "OpenAI",
                    "model": "gpt-thorough",
                    "model_reasoning_effort": "ultra",
                    "plan_mode_reasoning_effort": "high",
                    "service_tier": "default"
                }
            }
        }"#,
    );

    let selected = config
        .with_model_profile("thorough")
        .expect("select named profile");
    let active = selected.resolve_model_profile().expect("selected profile");

    assert_eq!(active.provider, "OpenAI");
    assert_eq!(active.provider_name, "OpenAI API");
    assert_eq!(active.model, "gpt-thorough");
    assert_eq!(active.reasoning_effort.as_deref(), Some("ultra"));
    assert_eq!(active.plan_mode_reasoning_effort.as_deref(), Some("high"));
    assert_eq!(active.service_tier.as_deref(), Some("default"));
    assert_eq!(config.model.as_deref(), Some("gpt-default"));
    assert_eq!(
        config
            .resolve_model_profile()
            .expect("original profile")
            .reasoning_effort
            .as_deref(),
        Some("medium")
    );
}

#[test]
fn named_profiles_are_stably_ordered_and_transport_free() {
    let config = configured(
        r#"{
            "model_profiles": {
                "z-review": { "model_provider": "OpenAI", "model": "gpt-review" },
                "a-fast": { "model_provider": "OpenAI", "model": "gpt-fast" }
            }
        }"#,
    );

    let names = config
        .model_profiles()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["a-fast", "z-review"]);
    let encoded = serde_json::to_string(config.model_profiles()).expect("serialize profiles");
    assert!(!encoded.contains("base_url"));
    assert!(!encoded.contains("api_key"));
}

#[test]
fn profile_omissions_use_provider_defaults_without_weakening_storage() {
    let config = configured(
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_reasoning_effort": "high",
            "plan_mode_reasoning_effort": "ultra",
            "service_tier": "priority",
            "disable_response_storage": true,
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "provider-defaults": {
                    "model_provider": "OpenAI",
                    "model": "gpt-next"
                }
            }
        }"#,
    );

    let selected = config
        .with_model_profile("provider-defaults")
        .expect("select profile")
        .resolve_model_profile()
        .expect("resolve selection");

    assert_eq!(selected.model, "gpt-next");
    assert_eq!(selected.reasoning_effort, None);
    assert_eq!(selected.plan_mode_reasoning_effort, None);
    assert_eq!(selected.service_tier, None);
    assert!(
        !selected.store,
        "profile selection relaxed response storage"
    );
}

#[test]
fn profile_selection_rejects_unknown_names_and_providers_without_echoing_model_text() {
    let config = configured(
        r#"{
            "model_profiles": {
                "broken": {
                    "model_provider": "Missing",
                    "model": "do-not-echo-model"
                }
            }
        }"#,
    );

    let missing = config
        .with_model_profile("sentinel\u{1b}profile")
        .unwrap_err();
    assert!(matches!(
        missing,
        ConfigError::Validation { ref path, .. } if path == "model_profile"
    ));
    assert!(!missing.to_string().contains("sentinel"));

    let provider = config.with_model_profile("broken").unwrap_err();
    assert!(matches!(
        provider,
        ConfigError::Validation { ref path, .. }
            if path == "model_profiles.broken.model_provider"
    ));
    assert!(!provider.to_string().contains("do-not-echo-model"));
}

#[test]
fn profile_schema_rejects_transport_and_storage_fields() {
    for field in ["base_url", "api_key", "disable_response_storage"] {
        let source = format!(
            r#"{{
                "model_profiles": {{
                    "unsafe": {{
                        "model_provider": "OpenAI",
                        "model": "gpt-test",
                        {field:?}: "do-not-echo"
                    }}
                }}
            }}"#
        );
        let error = ConfigLoader::test().load_user(&source).unwrap_err();
        assert!(matches!(
            error,
            ConfigError::Parse(_) | ConfigError::PlaintextSecret { .. }
        ));
        assert!(!error.to_string().contains("do-not-echo"));
    }
}

#[test]
fn direct_deserialization_cannot_bypass_profile_value_validation() {
    let config: orchester_laufzeit::harness::config::UserConfig = serde_json::from_str(
        r#"{
            "model_profiles": {
                "unsafe": {
                    "model_provider": "OpenAI",
                    "model": "gpt-test\u001b[31m"
                }
            }
        }"#,
    )
    .expect("schema-only deserialize");

    let error = config
        .with_model_profile("unsafe")
        .expect_err("control sequence must remain invalid");
    assert!(matches!(
        error,
        ConfigError::Validation { ref path, .. } if path == "model_profiles.unsafe.model"
    ));
    assert!(!error.to_string().contains("gpt-test"));
}
