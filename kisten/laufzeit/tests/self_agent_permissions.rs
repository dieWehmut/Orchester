use orchester_laufzeit::harness::config::{ConfigLoader, PolicyDecision};
use orchester_laufzeit::harness::service::{
    load_self_agent_permissions, SelfAgentPermissionSnapshot,
};

fn configured() -> orchester_laufzeit::harness::config::UserConfig {
    ConfigLoader::test()
        .load_user(
            r#"{
                "model_provider": "Loopback",
                "model": "gpt-permissions",
                "model_providers": {
                    "Loopback": {
                        "name": "Local Responses",
                        "base_url": "https://provider-canary.invalid/v1",
                        "wire_api": "responses",
                        "requires_openai_auth": false
                    }
                },
                "governance": {
                    "approval_reviewer": "user",
                    "tool_network": "deny",
                    "out_of_workspace": "allow",
                    "shell_interpreters": "allow",
                    "approval_ttl_seconds": 900
                }
            }"#,
        )
        .expect("config")
}

fn rule<'a>(
    snapshot: &'a SelfAgentPermissionSnapshot,
    id: &str,
) -> &'a orchester_laufzeit::harness::service::SelfAgentPermissionRule {
    snapshot
        .rules
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("missing permission rule {id}"))
}

#[test]
fn permission_snapshot_uses_real_policy_and_keeps_core_boundaries_strict() {
    let snapshot = load_self_agent_permissions(&configured());

    assert_eq!(snapshot.governance.network, PolicyDecision::Deny);
    assert_eq!(snapshot.governance.out_of_workspace, PolicyDecision::Allow);
    assert_eq!(
        snapshot.governance.shell_interpreters,
        PolicyDecision::Allow
    );
    assert_eq!(snapshot.governance.approval_ttl_seconds, 900);
    assert_eq!(
        rule(&snapshot, "network.external").effective,
        PolicyDecision::Deny
    );
    assert_eq!(
        rule(&snapshot, "dependency.install").effective,
        PolicyDecision::Deny
    );
    assert_eq!(
        rule(&snapshot, "path.out_of_workspace").effective,
        PolicyDecision::Deny
    );
    assert_eq!(
        rule(&snapshot, "shell.interpreter").effective,
        PolicyDecision::Deny
    );
    assert_eq!(
        rule(&snapshot, "privilege.escalation").effective,
        PolicyDecision::Deny
    );
    assert_eq!(
        rule(&snapshot, "workspace.read").effective,
        PolicyDecision::Allow
    );
}

#[test]
fn permission_snapshot_reports_unambiguous_approval_and_audit_boundaries() {
    let snapshot = load_self_agent_permissions(&configured());

    assert!(snapshot.approvals.state_machine_present);
    assert!(snapshot.approvals.reviewer_configured);
    assert!(snapshot.approvals.ttl_configured);
    assert!(!snapshot.approvals.cli_resolution_available);
    assert!(snapshot.audit.append_only_hash_chain);
    assert!(snapshot.audit.redacts_before_persistence);
    assert!(!snapshot.audit.inspected_existing_log);
}

#[test]
fn permission_snapshot_never_contains_provider_or_credential_values() {
    let config = serde_json::from_str(
        r#"{
            "env": {
                "OPENAI_API_KEY": "sk-permission-canary-should-never-escape"
            },
            "model_providers": {
                "Loopback": {
                    "base_url": "https://provider-canary.invalid/v1",
                    "api_key": "sk-provider-canary-should-never-escape"
                }
            }
        }"#,
    )
    .expect("adversarial config fixture");
    let snapshot = load_self_agent_permissions(&config);
    let rendered = format!("{snapshot:?}");

    assert!(!rendered.contains("provider-canary.invalid"));
    assert!(!rendered.contains("sk-permission-canary"));
    assert!(!rendered.contains("sk-provider-canary"));
}
