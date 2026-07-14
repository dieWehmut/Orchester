use orchester_laufzeit::harness::config::{
    merge_security, ConfigError, ConfigLoader, PolicyDecision, ProviderConfig, UserConfig,
};
use orchester_laufzeit::harness::credentials::{CredentialStore, InMemoryCredentialStore};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONFIG_DIR: AtomicU64 = AtomicU64::new(0);

struct TempConfigDir(PathBuf);

impl TempConfigDir {
    fn new(name: &str) -> Self {
        let sequence = NEXT_CONFIG_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "orchester-{name}-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempConfigDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_config_cannot_override_credentials_or_relax_policy() {
    let project = r#"{
        "model_providers": { "OpenAI": { "api_key": "x" } },
        "governance": { "out_of_workspace": "allow" }
    }"#;
    let err = ConfigLoader::test().load_project(project).unwrap_err();
    assert!(matches!(err, ConfigError::ForbiddenProjectField { .. }));
}

#[test]
fn resolved_model_profile_contains_transport_settings_but_no_secret() {
    let cfg = ConfigLoader::test()
        .load_user(
            r#"{
                "env": { "FAKE_API_KEY": "${secret:Fake}" },
                "model_provider": "Fake",
                "model": "fake-model",
                "model_reasoning_effort": "high",
                "plan_mode_reasoning_effort": "ultra",
                "disable_response_storage": true,
                "service_tier": "priority",
                "model_providers": {
                    "Fake": {
                        "name": "Fake Provider",
                        "base_url": "https://example.test/v1",
                        "api_key": "${env:FAKE_API_KEY}",
                        "wire_api": "responses",
                        "requires_openai_auth": true
                    }
                }
            }"#,
        )
        .unwrap();

    let profile = cfg.resolve_model_profile().unwrap();
    assert_eq!(profile.provider, "Fake");
    assert_eq!(profile.provider_name, "Fake Provider");
    assert_eq!(profile.model, "fake-model");
    assert_eq!(profile.base_url, "https://example.test/v1");
    assert_eq!(profile.wire_api, "responses");
    assert_eq!(profile.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(profile.plan_mode_reasoning_effort.as_deref(), Some("ultra"));
    assert!(!profile.store);
    assert_eq!(profile.service_tier.as_deref(), Some("priority"));
    assert!(profile.requires_auth);

    let debug = format!("{profile:?}");
    assert!(!debug.contains("do-not-echo"));
    assert!(!debug.contains("FAKE_API_KEY"));
    assert!(!debug.contains("secret:Fake"));
}

#[test]
fn load_effective_merges_workspace_project_before_resolving_profile() {
    let root = TempConfigDir::new("effective-config");
    let user_dir = root.path().join("home").join(".orchester");
    let workspace = root.path().join("workspace");
    let project_dir = workspace.join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::create_dir_all(&project_dir).unwrap();
    let user_path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &user_path,
        r#"{
            "model_provider": "Fake",
            "model": "user-model",
            "model_reasoning_effort": "medium",
            "model_providers": {
                "Fake": {
                    "name": "Fake Provider",
                    "base_url": "http://127.0.0.1:9876/v1",
                    "wire_api": "responses"
                }
            }
        }"#,
    )
    .unwrap();
    std::fs::write(
        project_dir.join("project.jsonc"),
        r#"{
            "model": "project-model",
            "model_reasoning_effort": "high"
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &user_path);

    let effective = ConfigLoader::test()
        .with_user_path(&user_path)
        .load_effective(&workspace)
        .unwrap();
    assert_eq!(effective.model.as_deref(), Some("project-model"));
    assert_eq!(effective.model_reasoning_effort.as_deref(), Some("high"));
    assert_eq!(
        effective.resolve_model_profile().unwrap().base_url,
        "http://127.0.0.1:9876/v1"
    );
}

#[test]
fn load_effective_treats_missing_project_config_as_empty() {
    let root = TempConfigDir::new("effective-config-no-project");
    let user_dir = root.path().join("home").join(".orchester");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let user_path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &user_path,
        r#"{
            "model_provider": "Fake",
            "model": "user-model",
            "model_providers": {
                "Fake": {
                    "base_url": "https://example.test/v1",
                    "wire_api": "responses"
                }
            }
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &user_path);

    let effective = ConfigLoader::test()
        .with_user_path(&user_path)
        .load_effective(&workspace)
        .unwrap();
    assert_eq!(effective.model.as_deref(), Some("user-model"));
}

#[test]
fn load_effective_keeps_non_model_settings_without_a_model_profile() {
    let root = TempConfigDir::new("effective-config-without-model");
    let user_dir = root.path().join("home").join(".orchester");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let user_path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &user_path,
        r#"{
            "governance": {
                "tool_network": "deny",
                "approval_ttl_seconds": 45
            },
            "limits": {
                "max_steps": 12,
                "max_minutes": 8,
                "max_same_failure": 2,
                "max_observation_bytes": 4096
            },
            "tui": {
                "status_line": ["current-dir", "permissions"],
                "status_line_use_colors": true
            }
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &user_path);

    let effective = ConfigLoader::test()
        .with_user_path(&user_path)
        .load_effective(&workspace)
        .unwrap();

    assert_eq!(effective.governance.tool_network, PolicyDecision::Deny);
    assert_eq!(effective.governance.approval_ttl_seconds, 45);
    assert_eq!(effective.limits.max_steps, 12);
    assert_eq!(effective.limits.max_minutes, 8);
    assert_eq!(effective.limits.max_same_failure, 2);
    assert_eq!(effective.limits.max_observation_bytes, 4096);
    assert_eq!(effective.tui.status_line, ["current-dir", "permissions"]);
    assert!(effective.tui.status_line_use_colors);
}

#[test]
fn load_effective_propagates_project_path_probe_errors() {
    let root = TempConfigDir::new("effective-config-invalid-project-path");
    let user_dir = root.path().join("home").join(".orchester");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let user_path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &user_path,
        r#"{ "governance": { "tool_network": "deny" } }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &user_path);

    let error = ConfigLoader::test()
        .with_user_path(&user_path)
        .with_project_path(PathBuf::from("\0"))
        .load_effective(&workspace)
        .unwrap_err();

    assert!(matches!(error, ConfigError::Io(_)));
}

#[test]
fn model_profile_rejects_unknown_provider_and_empty_model() {
    let loader = ConfigLoader::test();
    let unknown = loader
        .load_user(r#"{ "model_provider": "Missing", "model": "fake-model" }"#)
        .unwrap()
        .resolve_model_profile()
        .unwrap_err();
    assert!(
        matches!(unknown, ConfigError::Validation { ref path, .. } if path == "model_provider")
    );

    let empty = loader
        .load_user(
            r#"{
                "model_provider": "Fake",
                "model": "  ",
                "model_providers": {
                    "Fake": { "base_url": "https://example.test", "wire_api": "responses" }
                }
            }"#,
        )
        .unwrap()
        .resolve_model_profile()
        .unwrap_err();
    assert!(matches!(empty, ConfigError::Validation { ref path, .. } if path == "model"));
}

#[test]
fn model_profile_accepts_https_paths_ports_and_loopback_http() {
    for base_url in [
        "https://example.test:8443/v1/responses",
        "http://localhost:9876/v1",
        "http://127.0.0.1:9876/v1",
        "http://[::1]:9876/v1",
    ] {
        let source = format!(
            r#"{{
                "model_provider": "Fake",
                "model": "fake-model",
                "model_providers": {{
                    "Fake": {{ "base_url": {base_url:?}, "wire_api": "responses" }}
                }}
            }}"#
        );
        ConfigLoader::test()
            .load_user(&source)
            .unwrap()
            .resolve_model_profile()
            .unwrap();
    }
}

#[test]
fn model_profile_rejects_provider_url_credentials_query_and_fragment_without_echoing() {
    for base_url in [
        "https://do-not-echo@example.test/v1",
        "https://example.test/v1?token=do-not-echo",
        "https://example.test/v1#do-not-echo",
    ] {
        let source = format!(
            r#"{{
                "model_provider": "Fake",
                "model": "fake-model",
                "model_providers": {{
                    "Fake": {{ "base_url": {base_url:?}, "wire_api": "responses" }}
                }}
            }}"#
        );
        let error = ConfigLoader::test()
            .load_user(&source)
            .unwrap()
            .resolve_model_profile()
            .unwrap_err();
        assert!(
            matches!(error, ConfigError::Validation { ref path, .. } if path == "model_providers.Fake.base_url")
        );
        assert!(!error.to_string().contains("do-not-echo"));
    }
}

#[test]
fn model_profile_rejects_malformed_provider_hosts_and_ports_without_echoing() {
    for base_url in [
        "https://",
        "https://[do-not-echo]/v1",
        "https://example.test:99999/do-not-echo",
    ] {
        let source = format!(
            r#"{{
                "model_provider": "Fake",
                "model": "fake-model",
                "model_providers": {{
                    "Fake": {{ "base_url": {base_url:?}, "wire_api": "responses" }}
                }}
            }}"#
        );
        let error = ConfigLoader::test()
            .load_user(&source)
            .unwrap()
            .resolve_model_profile()
            .unwrap_err();
        assert!(
            matches!(error, ConfigError::Validation { ref path, .. } if path == "model_providers.Fake.base_url")
        );
        assert!(!error.to_string().contains("do-not-echo"));
    }
}

#[test]
fn model_profile_rejects_unsupported_wire_api_and_unsafe_url_schemes() {
    for (wire_api, base_url, expected_path) in [
        (
            "chat_completions",
            "https://example.test/v1",
            "model_providers.Fake.wire_api",
        ),
        (
            "responses",
            "file://do-not-echo/secret",
            "model_providers.Fake.base_url",
        ),
        (
            "responses",
            "http://example.test/v1",
            "model_providers.Fake.base_url",
        ),
    ] {
        let source = format!(
            r#"{{
                "model_provider": "Fake",
                "model": "fake-model",
                "model_providers": {{
                    "Fake": {{ "base_url": {base_url:?}, "wire_api": {wire_api:?} }}
                }}
            }}"#
        );
        let error = ConfigLoader::test()
            .load_user(&source)
            .unwrap()
            .resolve_model_profile()
            .unwrap_err();
        assert!(
            matches!(error, ConfigError::Validation { ref path, .. } if path == expected_path),
            "unexpected error: {error}"
        );
        assert!(!error.to_string().contains("do-not-echo"));
    }
}

#[cfg(unix)]
fn make_user_config_permissions_secure(directory: &std::path::Path, file: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[cfg(not(unix))]
fn make_user_config_permissions_secure(_directory: &std::path::Path, _file: &std::path::Path) {}

#[test]
fn project_sensitive_fields_are_rejected_recursively_without_echoing_values() {
    let project = r#"{
        "validators": [{ "id": "unit", "program": "cargo", "args": [],
            "metadata": { "nested": { "authorization_token": "do-not-echo" } } }]
    }"#;
    let err = ConfigLoader::test().load_project(project).unwrap_err();
    let text = err.to_string();
    assert!(matches!(err, ConfigError::ForbiddenProjectField { .. }));
    assert!(!text.contains("do-not-echo"));
}

#[test]
fn project_cannot_self_declare_trust_or_approval_authority() {
    for project in [
        r#"{ "trust_level": "trusted" }"#,
        r#"{ "governance": { "approval_reviewer": "project" } }"#,
    ] {
        assert!(matches!(
            ConfigLoader::test().load_project(project),
            Err(ConfigError::ForbiddenProjectField { .. })
        ));
    }
}

#[test]
fn security_merge_never_allows_project_to_relax_user_ceiling() {
    let err = merge_security(
        PolicyDecision::Allow,
        Some(PolicyDecision::Ask),
        Some(PolicyDecision::Allow),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, ConfigError::SecurityRelaxation));

    assert_eq!(
        merge_security(
            PolicyDecision::Allow,
            Some(PolicyDecision::Allow),
            Some(PolicyDecision::Ask),
            None,
        )
        .unwrap(),
        PolicyDecision::Ask
    );
}

#[test]
fn user_secret_reference_resolves_without_entering_redacted_view() {
    let creds = InMemoryCredentialStore::with("OpenAI", "secret-value");
    let cfg = ConfigLoader::test()
        .load_user(
            r#"{
                // JSONC comments and trailing commas are accepted.
                "env": { "OPENAI_API_KEY": "${secret:OpenAI}", },
                "model": "test-model",
            }"#,
        )
        .unwrap();
    assert_eq!(
        cfg.resolve_secret("OPENAI_API_KEY", &creds)
            .unwrap()
            .expose_for_provider(),
        "secret-value"
    );
    assert!(!cfg.redacted_json().contains("secret-value"));
    assert!(cfg.redacted_json().contains("${secret:OpenAI}"));
}

#[test]
fn provider_api_key_may_chain_through_env_reference_without_exposure() {
    let creds = InMemoryCredentialStore::with("OpenAI", "provider-secret");
    let cfg = ConfigLoader::test()
        .load_user(
            r#"{
                "env": { "OPENAI_API_KEY": "${secret:OpenAI}" },
                "model_providers": {
                    "OpenAI": {
                        "name": "OpenAI",
                        "api_key": "${env:OPENAI_API_KEY}"
                    }
                }
            }"#,
        )
        .unwrap();
    let secret = cfg.resolve_provider_secret("OpenAI", &creds).unwrap();
    assert_eq!(secret.expose_for_provider(), "provider-secret");
    assert!(!format!("{secret:?}").contains("provider-secret"));
}

#[test]
fn credential_aware_redacted_view_reports_only_source_and_presence() {
    let creds = InMemoryCredentialStore::with("OpenAI", "secret-value");
    let cfg = ConfigLoader::test()
        .load_user(r#"{ "env": { "OPENAI_API_KEY": "${secret:OpenAI}" } }"#)
        .unwrap();

    let json = cfg.redacted_with_credentials(&creds).unwrap().json();
    assert!(json.contains("${secret:OpenAI}"));
    assert!(json.contains("\"present\": true"));
    assert!(!json.contains("secret-value"));
}

#[test]
fn codex_style_user_config_aliases_remain_typed_and_safe() {
    let cfg = ConfigLoader::test()
        .load_user(
            r#"{
                "approvals_reviewer": "user",
                "tui": {
                    "status_line": ["current-dir", "model"],
                    "model_availability_nux": { "gpt-test": 3 }
                },
                "plugins": { "example@local": { "enabled": true } }
            }"#,
        )
        .unwrap();
    assert_eq!(cfg.governance.approval_reviewer, "user");
    assert!(cfg.plugins["example@local"].enabled);
}

#[test]
fn plaintext_secret_is_rejected_at_user_config_boundary() {
    for source in [
        r#"{ "env": { "OPENAI_API_KEY": "plaintext-not-a-reference" } }"#,
        r#"{ "model_providers": { "OpenAI": { "api_key": "plaintext-not-a-reference" } } }"#,
    ] {
        let err = ConfigLoader::test().load_user(source).unwrap_err();
        assert!(matches!(err, ConfigError::PlaintextSecret { .. }));
        assert!(!err.to_string().contains("plaintext-not-a-reference"));
    }
}

#[test]
fn protected_user_file_resolves_direct_credentials_and_redacts_every_view() {
    let root = TempConfigDir::new("protected-credentials");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &path,
        r#"{
            "env": {
                "OPENAI_API_KEY": "sk-protected-test-env",
                "BUILD_CHANNEL": "sk-protected-test-arbitrary-env",
                "OPENAI_KEY_ALIAS": "${env:OPENAI_API_KEY}"
            },
            "model_providers": {
                "OpenAI": {
                    "name": "OpenAI",
                    "base_url": "https://debug-base-url-marker.example/v1",
                    "api_key": "sk-protected-test-provider"
                }
            }
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &path);

    let config = ConfigLoader::test().load_user_file(&path).unwrap();
    let store = InMemoryCredentialStore::default();
    assert_eq!(config.env["OPENAI_API_KEY"], "<redacted>");
    assert_eq!(config.env["BUILD_CHANNEL"], "<redacted>");
    assert_eq!(config.env["OPENAI_KEY_ALIAS"], "${env:OPENAI_API_KEY}");
    assert_eq!(
        config.model_providers["OpenAI"].api_key.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        config
            .resolve_secret("OPENAI_API_KEY", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-env"
    );
    assert_eq!(
        config
            .resolve_provider_secret("OpenAI", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-provider"
    );
    assert_eq!(
        config
            .resolve_secret("BUILD_CHANNEL", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-arbitrary-env"
    );
    assert_eq!(
        config
            .resolve_secret("OPENAI_KEY_ALIAS", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-env"
    );

    let config_debug = format!("{:?}", config);
    let provider_debug = format!("{:?}", config.model_providers["OpenAI"]);
    for rendered in [&config_debug, &provider_debug] {
        assert!(!rendered.contains("debug-base-url-marker"), "{rendered}");
        assert!(!rendered.contains("sk-protected-test-env"), "{rendered}");
        assert!(!rendered.contains("sk-protected-test-arbitrary-env"));
        assert!(
            !rendered.contains("sk-protected-test-provider"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
    }

    let serialized = serde_json::to_string(&config).unwrap();
    assert!(serialized.contains("debug-base-url-marker"));
    for rendered in [
        serialized,
        config.redacted_json(),
        config.redacted_with_credentials(&store).unwrap().json(),
        serde_json::to_string(&config.model_providers["OpenAI"]).unwrap(),
    ] {
        assert!(!rendered.contains("sk-protected-test-env"), "{rendered}");
        assert!(!rendered.contains("sk-protected-test-arbitrary-env"));
        assert!(
            !rendered.contains("sk-protected-test-provider"),
            "{rendered}"
        );
    }
}

#[test]
fn public_config_mutation_cannot_inherit_protected_literal_authority() {
    let root = TempConfigDir::new("protected-mutation");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &path,
        r#"{
            "env": { "OPENAI_API_KEY": "sk-protected-test-original" },
            "model_providers": {
                "OpenAI": { "api_key": "sk-protected-test-provider-original" }
            }
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &path);

    let mut config = ConfigLoader::test().load_user_file(&path).unwrap();
    config.env.insert(
        "INJECTED_API_KEY".to_owned(),
        "sk-protected-test-injected".to_owned(),
    );
    config.env.insert(
        "OPENAI_API_KEY".to_owned(),
        "sk-protected-test-replaced".to_owned(),
    );
    config.env.insert(
        "PUBLIC_MUTATION".to_owned(),
        "opaque-protected-test-mutation".to_owned(),
    );
    let injected_provider = ProviderConfig {
        api_key: Some("sk-protected-test-provider-injected".to_owned()),
        ..ProviderConfig::default()
    };
    config
        .model_providers
        .insert("Injected".to_owned(), injected_provider);

    let store = InMemoryCredentialStore::default();
    for result in [
        config.resolve_secret("INJECTED_API_KEY", &store),
        config.resolve_secret("OPENAI_API_KEY", &store),
        config.resolve_provider_secret("Injected", &store),
    ] {
        assert!(matches!(result, Err(ConfigError::PlaintextSecret { .. })));
    }

    for rendered in [
        serde_json::to_string(&config).unwrap(),
        format!("{config:?}"),
        config.redacted_json(),
        config.redacted_with_credentials(&store).unwrap().json(),
    ] {
        assert!(!rendered.contains("opaque-protected-test-mutation"));
    }
}

#[test]
fn serialized_input_cannot_forge_protected_credential_provenance() {
    let source = r#"{
        "credential_source": "ProtectedUserFile",
        "env": { "OPENAI_API_KEY": "sk-protected-test-forged" }
    }"#;
    let loader_error = ConfigLoader::test().load_user(source).unwrap_err();
    assert!(!loader_error
        .to_string()
        .contains("sk-protected-test-forged"));

    let serde_error = serde_json::from_str::<UserConfig>(source).unwrap_err();
    assert!(!serde_error.to_string().contains("sk-protected-test-forged"));
}

#[test]
fn user_config_type_errors_do_not_echo_rejected_values() {
    let marker = "type-shape-marker-must-not-be-echoed";
    let source = format!(r#"{{ "env": {marker:?} }}"#);
    let error = ConfigLoader::test().load_user(&source).unwrap_err();
    assert!(matches!(error, ConfigError::Parse(_)));
    assert!(!error.to_string().contains(marker));
}

#[test]
fn protected_user_file_supports_dotted_credential_names() {
    let root = TempConfigDir::new("protected-dotted-credentials");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &path,
        r#"{
            "env": { "OPENAI.API_KEY": "sk-protected-test-dotted-env" },
            "model_providers": {
                "OpenAI.compat": { "api_key": "sk-protected-test-dotted-provider" }
            }
        }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &path);

    let config = ConfigLoader::test().load_user_file(&path).unwrap();
    let store = InMemoryCredentialStore::default();
    assert_eq!(
        config
            .resolve_secret("OPENAI.API_KEY", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-dotted-env"
    );
    assert_eq!(
        config
            .resolve_provider_secret("OpenAI.compat", &store)
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-dotted-provider"
    );
}

#[test]
fn protected_user_file_provenance_survives_project_merge() {
    let root = TempConfigDir::new("protected-merge");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &path,
        r#"{ "env": { "OPENAI_API_KEY": "sk-protected-test-merge" } }"#,
    )
    .unwrap();
    make_user_config_permissions_secure(&user_dir, &path);

    let loader = ConfigLoader::test();
    let user = loader.load_user_file(&path).unwrap();
    let project = loader
        .load_project(r#"{ "model": "project-model" }"#)
        .unwrap();
    let merged = loader.merge_project(&user, &project).unwrap();
    assert_eq!(merged.model.as_deref(), Some("project-model"));
    assert_eq!(
        merged
            .resolve_secret("OPENAI_API_KEY", &InMemoryCredentialStore::default())
            .unwrap()
            .expose_for_provider(),
        "sk-protected-test-merge"
    );
}

#[test]
fn protected_user_file_still_rejects_malformed_secret_references() {
    let root = TempConfigDir::new("protected-malformed-reference");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(&path, r#"{ "env": { "OPENAI_API_KEY": "${secret:}" } }"#).unwrap();
    make_user_config_permissions_secure(&user_dir, &path);

    assert!(matches!(
        ConfigLoader::test().load_user_file(&path),
        Err(ConfigError::InvalidSecretReference { .. })
    ));
}

#[cfg(unix)]
#[test]
fn insecure_user_file_cannot_enable_plaintext_credentials() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempConfigDir::new("protected-insecure");
    let user_dir = root.path().join("home").join(".orchester");
    std::fs::create_dir_all(&user_dir).unwrap();
    let path = user_dir.join("orchester.jsonc");
    std::fs::write(
        &path,
        r#"{ "env": { "OPENAI_API_KEY": "sk-protected-test-insecure" } }"#,
    )
    .unwrap();
    std::fs::set_permissions(&user_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let error = ConfigLoader::test().load_user_file(&path).unwrap_err();
    assert!(matches!(error, ConfigError::InsecurePermissions { .. }));
    assert!(!error.to_string().contains("sk-protected-test-insecure"));
}

#[test]
fn in_memory_credential_store_supports_set_get_and_clear_without_debug_leak() {
    let store = InMemoryCredentialStore::default();
    store
        .set(
            "OpenAI",
            secrecy::SecretString::new("secret-value".to_owned().into_boxed_str()),
        )
        .unwrap();
    let loaded = store.get("OpenAI").unwrap().unwrap();
    assert_eq!(
        secrecy::ExposeSecret::expose_secret(&loaded),
        "secret-value"
    );
    assert!(!format!("{loaded:?}").contains("secret-value"));
    store.clear("OpenAI").unwrap();
    assert!(store.get("OpenAI").unwrap().is_none());
}

#[test]
fn malformed_secret_reference_is_not_treated_as_a_literal() {
    let cfg = ConfigLoader::test()
        .load_user(r#"{ "env": { "OPENAI_API_KEY": "${secret:}" } }"#)
        .unwrap_err();
    assert!(matches!(cfg, ConfigError::InvalidSecretReference { .. }));
}

#[test]
fn unsupported_config_version_is_rejected_before_harness_use() {
    assert!(matches!(
        ConfigLoader::test().load_user(r#"{ "version": 99 }"#),
        Err(ConfigError::Validation { .. })
    ));
}

#[test]
fn malformed_json_error_does_not_echo_nearby_secret_text() {
    let err = ConfigLoader::test()
        .load_user("{\n\"env\": {\"OPENAI_API_KEY\": \"do-not-echo\"},\nthis is invalid\n}")
        .unwrap_err();
    assert!(!err.to_string().contains("do-not-echo"));
}

#[test]
fn project_merge_preserves_unspecified_user_security_and_selects_known_validators() {
    let loader = ConfigLoader::test();
    let user = loader
        .load_user(
            r#"{
                "governance": {
                    "tool_network": "deny",
                    "out_of_workspace": "allow"
                },
                "validators": [
                    { "id": "unit", "program": "cargo", "args": ["test"], "required": true }
                ]
            }"#,
        )
        .unwrap();
    let project = loader
        .load_project(
            r#"{
                "governance": { "out_of_workspace": "ask" },
                "validators": ["unit"]
            }"#,
        )
        .unwrap();

    let merged = loader.merge_project(&user, &project).unwrap();
    assert_eq!(merged.governance.tool_network, PolicyDecision::Deny);
    assert_eq!(merged.governance.out_of_workspace, PolicyDecision::Ask);
    assert_eq!(merged.validators.len(), 1);
    assert_eq!(merged.validators[0].id, "unit");
}

#[test]
fn project_merge_rejects_policy_relaxation_and_unknown_validator_selection() {
    let loader = ConfigLoader::test();
    let user = loader
        .load_user(r#"{ "governance": { "out_of_workspace": "deny" } }"#)
        .unwrap();
    let relaxed = loader
        .load_project(r#"{ "governance": { "out_of_workspace": "ask" } }"#)
        .unwrap();
    assert!(matches!(
        loader.merge_project(&user, &relaxed),
        Err(ConfigError::SecurityRelaxation)
    ));

    let unknown = loader
        .load_project(r#"{ "validators": ["missing"] }"#)
        .unwrap();
    assert!(matches!(
        loader.merge_project(&user, &unknown),
        Err(ConfigError::Validation { .. })
    ));
}

#[cfg(windows)]
#[test]
fn windows_permission_diagnostic_reports_owner_without_modifying_acl() {
    let path = std::env::temp_dir().join(format!("orchester-config-acl-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let before = std::fs::metadata(&path).unwrap().permissions().readonly();

    let findings = orchester_laufzeit::harness::config::check_permissions(&path);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].expected.to_ascii_lowercase().contains("owner"));
    assert!(findings[0]
        .actual
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("owner="));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().readonly(),
        before
    );

    std::fs::remove_dir_all(path).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_user_config_requires_directory_0700_and_file_0600() {
    use std::os::unix::fs::PermissionsExt;

    let root = std::env::temp_dir().join(format!("orchester-config-mode-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("orchester.jsonc");
    std::fs::write(&path, "{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let loader = ConfigLoader::test();
    assert!(matches!(
        loader.load_user_file(&path),
        Err(ConfigError::InsecurePermissions { .. })
    ));

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    loader.load_user_file(&path).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
