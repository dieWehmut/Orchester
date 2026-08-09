use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, UserConfig};
use orchester_laufzeit::harness::service::{
    load_self_agent_model_catalog, select_self_agent_model_profile, SelfAgentActiveModel,
    SelfAgentModelCatalogError, SelfAgentModelSession,
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
    let current = catalog.active.choice().expect("configured selection");

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

    assert_eq!(catalog.active, SelfAgentActiveModel::NotConfigured);
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
fn catalog_reports_controlled_active_metadata_without_ever_projecting_it() {
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

        let catalog = load_self_agent_model_catalog(&config).expect("controlled metadata");
        let projected = format!("{catalog:?}");

        // The offending field is named so it can be repaired, but the value
        // that failed validation is never carried into the projection.
        let (path, message) = match catalog.active {
            SelfAgentActiveModel::Unresolved { path, message } => (path, message),
            other => panic!("expected an unresolved active model, found {other:?}"),
        };
        assert_eq!(path, expected_path);
        assert_eq!(message, "model catalog metadata is invalid");
        assert!(!projected.contains("gpt-test"));
        assert!(!projected.contains('\u{1b}'));
    }
}

#[test]
fn returning_to_an_unresolvable_active_model_is_refused_with_its_reason() {
    let config = configured(
        r#"{
            "model_provider": "Missing",
            "model": "gpt-default",
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "fast": { "model_provider": "OpenAI", "model": "gpt-fast" }
            }
        }"#,
    );
    let mut session = SelfAgentModelSession::default();
    session
        .select_profile(&config, "fast")
        .expect("select named profile");

    // Listing degrades so the operator can see what is wrong...
    let catalog = load_self_agent_model_catalog(&config).expect("catalog still lists profiles");
    assert!(matches!(
        catalog.active,
        SelfAgentActiveModel::Unresolved { .. }
    ));
    assert_eq!(catalog.profiles.len(), 1);

    // ...but selecting it is an action, so it refuses and keeps the previous
    // working choice rather than adopting a model no turn could use.
    let error = session
        .select_configured(&config)
        .expect_err("an unresolvable active model must not become the selection");

    assert!(matches!(
        error,
        SelfAgentModelCatalogError::Config(ConfigError::Validation { ref path, .. })
            if path == "model_provider"
    ));
    assert_eq!(session.selected_profile(), Some("fast"));
    assert!(!error.to_string().contains("gpt-default"));
}

#[test]
fn model_session_changes_only_future_effective_configuration() {
    let config = configured(
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "review": {
                    "model_provider": "OpenAI",
                    "model": "gpt-review",
                    "model_reasoning_effort": "ultra"
                }
            }
        }"#,
    );
    let mut session = SelfAgentModelSession::default();

    let choice = session
        .select_profile(&config, "review")
        .expect("select session profile");
    let effective = session
        .effective_config(&config)
        .expect("session configuration");
    let catalog = session.catalog(&config).expect("session catalog");

    assert_eq!(choice.profile.as_deref(), Some("review"));
    assert_eq!(effective.model.as_deref(), Some("gpt-review"));
    assert_eq!(config.model.as_deref(), Some("gpt-default"));
    assert!(!format!("{session:?}").contains("review"));
    assert_eq!(
        catalog
            .active
            .choice()
            .and_then(|active| active.profile.as_deref()),
        Some("review")
    );
}

#[test]
fn failed_model_session_selection_preserves_the_previous_choice() {
    let config = configured(
        r#"{
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "fast": { "model_provider": "OpenAI", "model": "gpt-fast" }
            }
        }"#,
    );
    let mut session = SelfAgentModelSession::default();
    session
        .select_profile(&config, "fast")
        .expect("select initial profile");

    let error = session
        .select_profile(&config, "missing-profile")
        .expect_err("missing profile must fail");

    assert_eq!(session.selected_profile(), Some("fast"));
    assert!(!error.to_string().contains("missing-profile"));
    assert_eq!(
        session
            .effective_config(&config)
            .expect("retained configuration")
            .model
            .as_deref(),
        Some("gpt-fast")
    );
}

#[test]
fn model_session_returns_to_configured_choice_atomically() {
    let configured_default = configured(
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "fast": { "model_provider": "OpenAI", "model": "gpt-fast" }
            }
        }"#,
    );
    let mut session = SelfAgentModelSession::default();
    session
        .select_profile(&configured_default, "fast")
        .expect("select named profile");

    let restored = session
        .select_configured(&configured_default)
        .expect("restore configured model");

    assert_eq!(restored.profile, None);
    assert_eq!(restored.model, "gpt-default");
    assert_eq!(session.selected_profile(), None);

    let no_default = configured(
        r#"{
            "model_providers": {
                "OpenAI": { "base_url": "https://example.test/v1" }
            },
            "model_profiles": {
                "fast": { "model_provider": "OpenAI", "model": "gpt-fast" }
            }
        }"#,
    );
    session
        .select_profile(&no_default, "fast")
        .expect("select named-only profile");

    let error = session
        .select_configured(&no_default)
        .expect_err("missing configured model");

    assert!(matches!(
        error,
        SelfAgentModelCatalogError::ConfiguredModelUnavailable
    ));
    assert_eq!(session.selected_profile(), Some("fast"));
}
