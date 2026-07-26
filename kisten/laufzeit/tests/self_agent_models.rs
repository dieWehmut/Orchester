use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig};
use orchester_laufzeit::harness::service::{
    load_self_agent_model_catalog, select_self_agent_model_profile, SelfAgentModelCatalogError,
};

fn configured(source: &str) -> UserConfig {
    ConfigLoader::test().load_user(source).expect("user config")
}

#[test]
fn catalog_projects_configured_and_named_choices_without_transport_details() {
    let config = configured(
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_reasoning_effort": "high",
            "model_providers": {
                "OpenAI": {
                    "name": "OpenAI API",
                    "base_url": "https://catalog-private.example/v1",
                    "api_key": "${secret:CatalogSecret}",
                    "wire_api": "responses",
                    "requires_openai_auth": true
                }
            },
            "model_profiles": {
                "review": {
                    "model_provider": "OpenAI",
                    "model": "gpt-review",
                    "model_reasoning_effort": "ultra"
                },
                "fast": {
                    "model_provider": "OpenAI",
                    "model": "gpt-fast",
                    "model_reasoning_effort": "low"
                }
            }
        }"#,
    );

    let catalog = load_self_agent_model_catalog(&config).expect("model catalog");
    let current = catalog.configured.as_ref().expect("configured selection");

    assert_eq!(current.profile, None);
    assert_eq!(current.provider, "OpenAI");
    assert_eq!(current.provider_name, "OpenAI API");
    assert_eq!(current.model, "gpt-default");
    assert_eq!(current.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(
        catalog
            .profiles
            .iter()
            .map(|choice| choice.profile.as_deref().expect("named profile"))
            .collect::<Vec<_>>(),
        vec!["fast", "review"]
    );
    let rendered = format!("{catalog:?}");
    assert!(!rendered.contains("catalog-private"));
    assert!(!rendered.contains("CatalogSecret"));
    assert!(!rendered.contains("wire_api"));
    assert!(!rendered.contains("requires_auth"));
}

#[test]
fn catalog_lists_named_choices_when_the_configured_default_is_absent() {
    let config = configured(
        r#"{
            "model_providers": {
                "Local": { "base_url": "http://127.0.0.1:4567/v1" }
            },
            "model_profiles": {
                "offline": { "model_provider": "Local", "model": "local-model" }
            }
        }"#,
    );

    let catalog = load_self_agent_model_catalog(&config).expect("model catalog");

    assert!(catalog.configured.is_none());
    assert_eq!(catalog.profiles.len(), 1);
    assert_eq!(catalog.profiles[0].profile.as_deref(), Some("offline"));
    assert_eq!(catalog.profiles[0].model, "local-model");
}

#[test]
fn selecting_a_catalog_profile_returns_a_matching_effective_config() {
    let config = configured(
        r#"{
            "model_providers": {
                "Local": { "base_url": "http://localhost:4567/v1" }
            },
            "model_profiles": {
                "offline": {
                    "model_provider": "Local",
                    "model": "local-model",
                    "model_reasoning_effort": "medium",
                    "plan_mode_reasoning_effort": "high",
                    "service_tier": "default"
                }
            }
        }"#,
    );

    let (selected, choice) =
        select_self_agent_model_profile(&config, "offline").expect("select profile");
    let resolved = selected
        .resolve_model_profile()
        .expect("resolved selection");

    assert_eq!(choice.profile.as_deref(), Some("offline"));
    assert_eq!(choice.model, resolved.model);
    assert_eq!(choice.provider, resolved.provider);
    assert_eq!(choice.reasoning_effort, resolved.reasoning_effort);
    assert_eq!(
        choice.plan_reasoning_effort,
        resolved.plan_mode_reasoning_effort
    );
    assert_eq!(choice.service_tier, resolved.service_tier);
    assert!(config.model.is_none(), "selection mutated the base config");
}

#[test]
fn invalid_named_choices_fail_without_echoing_model_text() {
    let config = configured(
        r#"{
            "model_profiles": {
                "broken": {
                    "model_provider": "Missing",
                    "model": "do-not-echo-catalog-model"
                }
            }
        }"#,
    );

    let error = load_self_agent_model_catalog(&config).expect_err("invalid catalog");

    assert!(matches!(error, SelfAgentModelCatalogError::Config(_)));
    assert!(!error.to_string().contains("do-not-echo-catalog-model"));
}

#[test]
fn catalog_rejects_controlled_configured_metadata_before_projection() {
    for (source, expected_path) in [
        (
            r#"{
                "model_provider": "OpenAI",
                "model": "gpt-test\u001b[31m",
                "model_providers": {
                    "OpenAI": { "name": "OpenAI", "base_url": "https://example.test/v1" }
                }
            }"#,
            "model",
        ),
        (
            r#"{
                "model_provider": "OpenAI",
                "model": "gpt-test",
                "model_providers": {
                    "OpenAI": {
                        "name": "OpenAI\u001b[31m",
                        "base_url": "https://example.test/v1"
                    }
                }
            }"#,
            "model_provider_name",
        ),
    ] {
        let config = configured(source);

        let error = load_self_agent_model_catalog(&config).expect_err("controlled metadata");

        assert!(matches!(
            error,
            SelfAgentModelCatalogError::Config(ConfigError::Validation { ref path, .. })
                if path == expected_path
        ));
        assert!(!error.to_string().contains("gpt-test"));
    }
}
