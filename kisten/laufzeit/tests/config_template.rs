//! The shipped `orchester.jsonc` template is the first thing a new user reads,
//! so it is held to the same standard as code: it must parse, it must document
//! every field the schema accepts, it must not ship a weaker security posture
//! than the built-in defaults, and it must never carry a literal credential.

use orchester_laufzeit::harness::config::{
    ConfigLoader, PolicyDecision, UserConfig, USER_CONFIG_TEMPLATE,
};

fn template() -> UserConfig {
    ConfigLoader::test()
        .load_user(USER_CONFIG_TEMPLATE)
        .expect("the shipped template must parse")
}

#[test]
fn the_template_parses_into_a_valid_user_config() {
    let config = template();
    assert_eq!(config.version, 1);
}

/// `deny_unknown_fields` means a stale template would be rejected at load
/// time rather than partially applied, so parsing is the drift alarm. This
/// asserts the reverse direction: every field the schema accepts is at least
/// mentioned, so a newly added knob cannot stay invisible to users.
#[test]
fn the_template_mentions_every_top_level_field() {
    const TOP_LEVEL_FIELDS: &[&str] = &[
        "$schema",
        "version",
        "env",
        "model_provider",
        "approvals_reviewer",
        "model",
        "review_model",
        "model_reasoning_effort",
        "plan_mode_reasoning_effort",
        "disable_response_storage",
        "network_access",
        "windows_wsl_setup_acknowledged",
        "service_tier",
        "model_providers",
        "model_profiles",
        "projects",
        "governance",
        "limits",
        "validators",
        "tui",
        "features",
        "windows",
        "notice",
        "plugins",
    ];
    for field in TOP_LEVEL_FIELDS {
        assert!(
            USER_CONFIG_TEMPLATE.contains(field),
            "the template never mentions '{field}'"
        );
    }
}

#[test]
fn the_template_shows_how_to_reach_a_provider() {
    let config = template();
    let providers = config.model_providers();
    assert!(
        !providers.is_empty(),
        "a user cannot reach a model without a provider entry to copy"
    );
    let (_, provider) = providers.iter().next().expect("one provider");
    assert!(
        provider.base_url.is_some(),
        "the template must show where the base URL goes"
    );
    assert!(
        provider.api_key.is_some(),
        "the template must show where the API key goes"
    );
}

/// The whole point of the template is that a user can copy it and be safe by
/// default, so the credential fields must ship as references rather than as
/// invitations to paste a key into a world-readable file.
#[test]
fn the_template_ships_no_literal_credential() {
    let config = template();
    for (name, provider) in config.model_providers() {
        let key = provider
            .api_key
            .as_deref()
            .unwrap_or_else(|| panic!("provider '{name}' has no api_key field to inspect"));
        assert!(
            key.starts_with("${") && key.ends_with('}'),
            "provider '{name}' ships a literal api_key"
        );
    }
    for (name, value) in config.env() {
        assert!(
            value.starts_with("${") && value.ends_with('}'),
            "env '{name}' ships a literal value where a reference belongs"
        );
    }
}

#[test]
fn the_template_does_not_duplicate_provider_keys_through_env() {
    assert!(template().env().is_empty());
    assert!(!USER_CONFIG_TEMPLATE.contains("OPENAI_API_KEY"));
}

#[test]
fn the_template_does_not_require_the_legacy_authentication_flag() {
    assert!(!USER_CONFIG_TEMPLATE.contains("requires_openai_auth"));
    for provider in template().model_providers().values() {
        assert_eq!(provider.requires_openai_auth, None);
    }
}

/// A template that silently relaxes governance would be worse than no
/// template, because the weakening would arrive disguised as a default.
#[test]
fn the_template_does_not_relax_the_built_in_governance_defaults() {
    let config = template();
    let defaults = UserConfig::default();
    assert!(
        config.governance.out_of_workspace >= defaults.governance.out_of_workspace,
        "the template weakened out_of_workspace"
    );
    assert!(
        config.governance.shell_interpreters >= defaults.governance.shell_interpreters,
        "the template weakened shell_interpreters"
    );
    assert!(
        config.governance.tool_network >= defaults.governance.tool_network,
        "the template weakened tool_network"
    );
    assert_ne!(
        config.governance.out_of_workspace,
        PolicyDecision::Allow,
        "escaping the workspace must never be allowed by default"
    );
}

/// Budgets are a halting mechanism, so a template that shipped unbounded
/// values would remove the stop condition the loop relies on.
#[test]
fn the_template_keeps_bounded_budgets() {
    let config = template();
    assert!(config.limits.max_steps > 0);
    assert!(config.limits.max_minutes > 0);
    assert!(config.limits.max_same_failure > 0);
    assert!(config.limits.max_observation_bytes > 0);
}

/// The feedback dimension is only real if a sensor is wired, so the template
/// must arrive with a runnable validator rather than an empty list.
#[test]
fn the_template_arrives_with_a_feedback_sensor() {
    let config = template();
    assert!(
        !config.validators.is_empty(),
        "the template must show at least one feedback validator"
    );
    for validator in &config.validators {
        assert!(!validator.id.is_empty());
        assert!(!validator.program.is_empty());
    }
}

/// A redacted view is what `/config` prints; the template must survive that
/// path so the command cannot be the thing that leaks a reference.
#[test]
fn the_template_survives_redaction() {
    let json = template().redacted().json();
    assert!(json.contains("model_providers"));
    assert!(json.contains("${secret:OpenAI}"));
}
