//! Side-effect-free projection of the self-agent's effective permissions.
//!
//! The projection intentionally evaluates a fixed, secret-free action matrix
//! through the same [`PolicyEngine`] used by durable execution.  It does not
//! resolve credentials, open the run database, inspect audit files, or copy
//! provider configuration into its public view.

use orchester_protokoll::{AgentAction, PolicyDecision};

use super::super::config::UserConfig;
use super::super::governance::{EffectClass, PolicyConstraints, PolicyEngine, PolicyResult, Risk};

/// A read-only description of the effective self-agent governance surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentPermissionSnapshot {
    pub policy_snapshot_hash: String,
    pub side_effect_free: bool,
    pub governance: SelfAgentPermissionGovernance,
    pub rules: Vec<SelfAgentPermissionRule>,
    pub approvals: SelfAgentApprovalStatus,
    pub audit: SelfAgentAuditStatus,
}

/// User-configured governance values shown alongside the effective rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentPermissionGovernance {
    pub approval_reviewer: String,
    pub network: PolicyDecision,
    pub out_of_workspace: PolicyDecision,
    pub shell_interpreters: PolicyDecision,
    pub approval_ttl_seconds: u64,
}

/// One bounded representative rule in the permission summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentPermissionRule {
    /// Stable summary identifier, independent of configured tightening.
    pub id: String,
    pub label: String,
    /// The configured knob, if this rule has one.  `None` means the rule is a
    /// built-in invariant or uses the built-in approval default.
    pub configured: Option<PolicyDecision>,
    pub effective: PolicyDecision,
    /// The rule selected by the authoritative policy engine.
    pub policy_rule_id: String,
    pub risk: Risk,
    pub effect: EffectClass,
    pub reason: String,
}

/// What the current build can truthfully promise about human approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentApprovalStatus {
    pub state_machine_present: bool,
    pub reviewer_configured: bool,
    pub ttl_configured: bool,
    /// The runtime state machine exists, but this snapshot is not itself an
    /// interactive approval resolver.  The CLI resolver is added separately.
    pub cli_resolution_available: bool,
}

/// Static guarantees of the append-only audit boundary.  No existing audit
/// record is read to build this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentAuditStatus {
    pub append_only_hash_chain: bool,
    pub redacts_before_persistence: bool,
    pub inspected_existing_log: bool,
}

/// Build a permission summary without touching credentials or durable state.
pub fn load_self_agent_permissions(config: &UserConfig) -> SelfAgentPermissionSnapshot {
    let policy = PolicyEngine::with_constraints(PolicyConstraints {
        network: config.governance.tool_network,
        out_of_workspace: config.governance.out_of_workspace,
        shell_interpreters: config.governance.shell_interpreters,
    });

    let rules = vec![
        action_rule(
            &policy,
            "workspace.read",
            "workspace read",
            None,
            AgentAction::ReadFile {
                path: "src/lib.rs".to_owned(),
                start_line: None,
                end_line: None,
            },
        ),
        action_rule(
            &policy,
            "workspace.write",
            "workspace write",
            None,
            AgentAction::WriteFile {
                path: "src/generated.rs".to_owned(),
                content: "x".to_owned(),
            },
        ),
        action_rule(
            &policy,
            "network.external",
            "external network",
            Some(config.governance.tool_network),
            AgentAction::RunCommand {
                program: "curl".to_owned(),
                args: vec!["https://example.test".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "dependency.install",
            "dependency install",
            Some(config.governance.tool_network),
            AgentAction::RunCommand {
                program: "cargo".to_owned(),
                args: vec!["add".to_owned(), "serde".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "filesystem.delete",
            "workspace delete",
            None,
            AgentAction::RunCommand {
                program: "rm".to_owned(),
                args: vec!["-rf".to_owned(), "build".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "git.destructive",
            "destructive Git",
            None,
            AgentAction::RunCommand {
                program: "git".to_owned(),
                args: vec!["reset".to_owned(), "--hard".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "privilege.escalation",
            "privilege escalation",
            None,
            AgentAction::RunCommand {
                program: "sudo".to_owned(),
                args: vec!["id".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "shell.interpreter",
            "shell interpreter",
            Some(config.governance.shell_interpreters),
            AgentAction::RunCommand {
                program: "powershell".to_owned(),
                args: vec!["-Command".to_owned(), "Get-ChildItem".to_owned()],
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "command.unknown",
            "unknown command",
            None,
            AgentAction::RunCommand {
                program: "not-a-real-tool".to_owned(),
                args: Vec::new(),
                cwd: None,
            },
        ),
        action_rule(
            &policy,
            "approval.explicit_checkpoint",
            "explicit approval checkpoint",
            None,
            AgentAction::RequestApproval {
                reason: "fixed permission summary".to_owned(),
            },
        ),
        invariant_rule(
            "path.out_of_workspace",
            "out-of-workspace path",
            Some(config.governance.out_of_workspace),
            PolicyDecision::Deny,
            Risk::High,
            EffectClass::WorkspaceMutation,
            "workspace boundary is always enforced by the path barrier",
        ),
    ];

    SelfAgentPermissionSnapshot {
        policy_snapshot_hash: policy.snapshot_hash_value(),
        side_effect_free: true,
        governance: SelfAgentPermissionGovernance {
            approval_reviewer: config.governance.approval_reviewer.clone(),
            network: config.governance.tool_network,
            out_of_workspace: config.governance.out_of_workspace,
            shell_interpreters: config.governance.shell_interpreters,
            approval_ttl_seconds: config.governance.approval_ttl_seconds,
        },
        rules,
        approvals: SelfAgentApprovalStatus {
            state_machine_present: true,
            reviewer_configured: !config.governance.approval_reviewer.trim().is_empty(),
            ttl_configured: config.governance.approval_ttl_seconds > 0,
            cli_resolution_available: false,
        },
        audit: SelfAgentAuditStatus {
            append_only_hash_chain: true,
            redacts_before_persistence: true,
            inspected_existing_log: false,
        },
    }
}

fn action_rule(
    policy: &PolicyEngine,
    id: &str,
    label: &str,
    configured: Option<PolicyDecision>,
    action: AgentAction,
) -> SelfAgentPermissionRule {
    let result = policy
        .evaluate(&action)
        .expect("fixed permission action must be supported by the policy engine");
    rule_from_result(id, label, configured, result)
}

fn rule_from_result(
    id: &str,
    label: &str,
    configured: Option<PolicyDecision>,
    result: PolicyResult,
) -> SelfAgentPermissionRule {
    SelfAgentPermissionRule {
        id: id.to_owned(),
        label: label.to_owned(),
        configured,
        effective: result.decision,
        policy_rule_id: result.rule_id,
        risk: result.risk,
        effect: result.effect,
        reason: result.reason,
    }
}

fn invariant_rule(
    id: &str,
    label: &str,
    configured: Option<PolicyDecision>,
    effective: PolicyDecision,
    risk: Risk,
    effect: EffectClass,
    reason: &str,
) -> SelfAgentPermissionRule {
    SelfAgentPermissionRule {
        id: id.to_owned(),
        label: label.to_owned(),
        configured,
        effective,
        policy_rule_id: id.to_owned(),
        risk,
        effect,
        reason: reason.to_owned(),
    }
}
