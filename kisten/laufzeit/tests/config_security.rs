use orchester_laufzeit::harness::config::{
    merge_security, ConfigError, ConfigLoader, PolicyDecision,
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
                        "base_url": "https://example.test/v1?token=do-not-echo",
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
    assert_eq!(
        profile.base_url,
        "https://example.test/v1?token=do-not-echo"
    );
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
fn model_profile_accepts_only_supported_wire_api_and_safe_base_url() {
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
            "http://example.test/v1?token=do-not-echo",
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
    let err = ConfigLoader::test()
        .load_user(r#"{ "env": { "OPENAI_API_KEY": "plaintext-not-a-reference" } }"#)
        .unwrap_err();
    assert!(matches!(err, ConfigError::PlaintextSecret { .. }));
    assert!(!err.to_string().contains("plaintext-not-a-reference"));
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
