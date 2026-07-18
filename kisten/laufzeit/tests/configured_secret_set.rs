use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader, ConfiguredSecretSet};
use orchester_laufzeit::harness::credentials::InMemoryCredentialStore;

const SECRET_CANARY: &str = "configured-secret-set-canary";

#[test]
fn resolves_and_deduplicates_reference_chains_without_exposure() {
    let config = ConfigLoader::test()
        .load_user(
            r#"{
                "env": {
                    "PRIMARY_KEY": "${secret:Primary}",
                    "KEY_ALIAS": "${env:PRIMARY_KEY}",
                    "SECONDARY_KEY": "${secret:Secondary}",
                    "PUBLIC_MODE": "enabled"
                },
                "model_providers": {
                    "OpenAI": { "api_key": "${env:KEY_ALIAS}" }
                }
            }"#,
        )
        .expect("valid config");
    let credentials = InMemoryCredentialStore::with("Primary", SECRET_CANARY);
    use orchester_laufzeit::harness::credentials::CredentialStore;
    credentials
        .set(
            "Secondary",
            secrecy::SecretString::new("second-secret".into()),
        )
        .expect("second credential");

    let secrets: ConfiguredSecretSet = config
        .resolve_configured_secrets(&credentials)
        .expect("configured secret set");
    assert_eq!(secrets.len(), 2);
    assert!(!secrets.is_empty());
    let rendered = format!("{secrets:?}");
    assert!(rendered.contains("count: 2"));
    assert!(!rendered.contains(SECRET_CANARY));
    assert!(!rendered.contains("second-secret"));
}

#[test]
fn missing_referenced_credentials_fail_closed_without_echoing_values() {
    let config = ConfigLoader::test()
        .load_user(r#"{ "env": { "MISSING_KEY": "${secret:Missing}" } }"#)
        .expect("valid config");
    let error = config
        .resolve_configured_secrets(&InMemoryCredentialStore::default())
        .expect_err("missing reference should fail");
    assert!(matches!(error, ConfigError::SecretUnavailable { .. }));
    assert!(!format!("{error:?} {error}").contains(SECRET_CANARY));
}

#[test]
fn rejects_unbounded_secret_values_and_secret_counts() {
    use orchester_laufzeit::harness::credentials::CredentialStore;
    use secrecy::SecretString;

    let oversized_config = ConfigLoader::test()
        .load_user(r#"{ "env": { "LARGE_KEY": "${secret:Large}" } }"#)
        .expect("valid config");
    let oversized_store = InMemoryCredentialStore::default();
    oversized_store
        .set("Large", SecretString::new("x".repeat(64 * 1024 + 1).into()))
        .expect("credential");
    assert!(matches!(
        oversized_config.resolve_configured_secrets(&oversized_store),
        Err(ConfigError::Validation { .. })
    ));

    let mut env = serde_json::Map::new();
    let many_store = InMemoryCredentialStore::default();
    for index in 0..257 {
        let name = format!("Secret{index}");
        env.insert(
            format!("KEY_{index}"),
            serde_json::Value::String(format!("${{secret:{name}}}")),
        );
        many_store
            .set(&name, SecretString::new(format!("value-{index}").into()))
            .expect("credential");
    }
    let source = serde_json::to_string(&serde_json::json!({ "env": env })).expect("JSON");
    let many_config = ConfigLoader::test()
        .load_user(&source)
        .expect("valid config");
    assert!(matches!(
        many_config.resolve_configured_secrets(&many_store),
        Err(ConfigError::Validation { .. })
    ));
}
