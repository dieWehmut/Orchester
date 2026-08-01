//! Deterministic policy decisions over structured action intents.

use std::ffi::{OsStr, OsString};

use orchester_protokoll::{AgentAction, PolicyDecision};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::command::{classify_command, CommandCategory, CommandIntent};
use crate::harness::run_store::EffectClass;

/// Short name retained for policy APIs and matrix tests.
pub type Decision = PolicyDecision;

/// Relative severity attached to a policy result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

/// Compatibility alias for callers that prefer the longer name.
pub type RiskLevel = Risk;

/// User-owned governance values that may only tighten the built-in policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyConstraints {
    pub network: PolicyDecision,
    pub out_of_workspace: PolicyDecision,
    pub shell_interpreters: PolicyDecision,
}

impl Default for PolicyConstraints {
    fn default() -> Self {
        Self {
            network: PolicyDecision::Ask,
            out_of_workspace: PolicyDecision::Deny,
            shell_interpreters: PolicyDecision::Deny,
        }
    }
}

/// A policy result contains only bounded, static explanations.  It never
/// copies the command arguments, which may contain credentials or source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyResult {
    pub decision: Decision,
    pub rule_id: String,
    pub risk: Risk,
    pub reason: String,
    pub effect: EffectClass,
}

impl PolicyResult {
    /// Accessor using the terminology from the action protocol and run store.
    pub fn effect_class(&self) -> EffectClass {
        self.effect
    }
}

/// Inputs accepted by custom policy rules.  Rules receive the structured
/// vector, not a shell string.
pub enum PolicyInput<'a> {
    Action(&'a AgentAction),
    Command {
        program: &'a OsStr,
        args: &'a [OsString],
    },
}

/// Extension point for future configured rules.  Core invariants are always
/// evaluated by [`PolicyEngine`] before any optional rule can relax a result.
pub trait PolicyRule: Send + Sync {
    fn evaluate(&self, input: &PolicyInput<'_>) -> Option<PolicyResult>;
}

/// Errors reserved for future policy configuration failures.  Runtime command
/// parse errors are represented as a DENY result instead of escaping to callers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PolicyError {
    #[error("policy input is unsupported")]
    UnsupportedInput,
}

/// Stateless deterministic policy engine for the built-in command/action
/// matrix.  It is intentionally cheap to clone and can later carry an
/// immutable policy snapshot.
#[derive(Debug, Clone, Default)]
pub struct PolicyEngine {
    constraints: PolicyConstraints,
}

impl PolicyEngine {
    /// Stable identity for the built-in policy matrix.  A future configured
    /// policy must replace this with its immutable manifest digest.
    pub fn snapshot_hash() -> String {
        Self::new().snapshot_hash_value()
    }

    pub fn snapshot_hash_value(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"orchester-policy-v1\0");
        if self.constraints != PolicyConstraints::default() {
            hasher.update(b"constraints\0");
            hash_decision(&mut hasher, self.constraints.network);
            hash_decision(&mut hasher, self.constraints.out_of_workspace);
            hash_decision(&mut hasher, self.constraints.shell_interpreters);
        }
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_constraints(constraints: PolicyConstraints) -> Self {
        Self { constraints }
    }

    pub fn constraints(&self) -> PolicyConstraints {
        self.constraints
    }

    /// Evaluate a decoded protocol action.
    pub fn evaluate(&self, action: &AgentAction) -> Result<PolicyResult, PolicyError> {
        let result = match action {
            AgentAction::ListFiles { .. }
            | AgentAction::SearchText { .. }
            | AgentAction::ReadFile { .. }
            | AgentAction::Recall { .. } => allow(
                "workspace.read",
                Risk::Low,
                "read-only workspace or memory operation",
                EffectClass::ReadOnlyIdempotent,
            ),
            AgentAction::WriteFile { .. } | AgentAction::ApplyPatch { .. } => allow(
                "workspace.write",
                Risk::Low,
                "workspace mutation is governed by the path barrier",
                EffectClass::WorkspaceMutation,
            ),
            AgentAction::Remember { .. } => allow(
                "memory.write",
                Risk::Low,
                "local memory write",
                EffectClass::WorkspaceMutation,
            ),
            AgentAction::RunChecks { .. } => ask(
                "validator.unconfigured",
                Risk::Medium,
                "checks may execute project code or access external state",
                EffectClass::MayMutate,
            ),
            AgentAction::RunCommand { program, args, .. } => {
                let args = args.iter().map(OsString::from).collect::<Vec<_>>();
                self.evaluate_command(program, &args)
            }
            AgentAction::RequestApproval { .. } => ask(
                "approval.explicit_checkpoint",
                Risk::Medium,
                "explicit approval requests always pause for the run owner",
                EffectClass::ReadOnlyIdempotent,
            ),
            AgentAction::Finish { .. } => allow(
                "run.finish",
                Risk::Low,
                "finish is a control-plane operation",
                EffectClass::ReadOnlyIdempotent,
            ),
        };
        Ok(result)
    }

    /// Evaluate a raw structured executable/argument vector.  Every parse
    /// failure returns a stable DENY result (`command.parse`).
    pub fn evaluate_command<P>(&self, program: P, args: &[OsString]) -> PolicyResult
    where
        P: AsRef<OsStr>,
    {
        let intent = match classify_command(program, args) {
            Ok(intent) => intent,
            Err(_) => return deny_parse(),
        };
        self.evaluate_intent(&intent)
    }

    /// Evaluate an already parsed command intent.
    pub fn evaluate_intent(&self, intent: &CommandIntent) -> PolicyResult {
        let categories = &intent.categories;
        let result = if categories.contains(&CommandCategory::ShellInterpreter) {
            deny(
                "shell.interpreter",
                Risk::High,
                "shell and scripting interpreters are disabled",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::PrivilegeEscalation) {
            deny(
                "privilege.escalation",
                Risk::Critical,
                "privilege escalation is disabled",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::UnsupportedWrapper) {
            deny(
                "command.wrapper",
                Risk::High,
                "command wrappers can hide the executable or alter its environment",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::Composite) {
            deny(
                "command.composite",
                Risk::High,
                "shell composition and redirection tokens are not accepted",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::SystemDestructive) {
            deny(
                "system.destructive",
                Risk::Critical,
                "system or root-targeted destructive operation is disabled",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::GitDestructive) {
            deny(
                "git.destructive",
                Risk::High,
                "destructive Git history or repository operation requires denial",
                EffectClass::WorkspaceMutation,
            )
        } else if categories.contains(&CommandCategory::PackageInstall) {
            ask(
                "dependency.install",
                Risk::Medium,
                "dependency installation can execute code and access the network",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::Network) {
            ask(
                "network.external",
                Risk::Medium,
                "external network access requires human approval",
                EffectClass::ExternalEffect,
            )
        } else if categories.contains(&CommandCategory::Delete) {
            ask(
                "filesystem.delete",
                Risk::Medium,
                "workspace deletion requires human approval",
                EffectClass::WorkspaceMutation,
            )
        } else if categories.contains(&CommandCategory::GitWrite) {
            ask(
                "git.write",
                Risk::Medium,
                "Git repository mutation requires human approval",
                EffectClass::WorkspaceMutation,
            )
        } else if categories.contains(&CommandCategory::WorkspaceWrite) {
            ask(
                "command.may_mutate",
                Risk::Medium,
                "the command may execute project code or write generated state",
                EffectClass::MayMutate,
            )
        } else if categories.contains(&CommandCategory::ReadOnly) {
            allow(
                "workspace.read",
                Risk::Low,
                "command is on the explicit read-only allowlist",
                EffectClass::ReadOnlyIdempotent,
            )
        } else {
            deny(
                "command.unknown",
                Risk::High,
                "executable is not in the governed command catalog",
                EffectClass::ExternalEffect,
            )
        };
        self.apply_constraints(categories, result)
    }

    fn apply_constraints(
        &self,
        categories: &std::collections::BTreeSet<CommandCategory>,
        result: PolicyResult,
    ) -> PolicyResult {
        if self.constraints.network > result.decision
            && (categories.contains(&CommandCategory::Network)
                || categories.contains(&CommandCategory::PackageInstall))
        {
            return deny(
                "network.configured_deny",
                result.risk,
                "effective governance denies external network access",
                result.effect,
            );
        }
        result
    }
}

fn hash_decision(hasher: &mut Sha256, decision: PolicyDecision) {
    hasher.update([match decision {
        PolicyDecision::Allow => 0,
        PolicyDecision::Ask => 1,
        PolicyDecision::Deny => 2,
    }]);
}

fn allow(rule_id: &str, risk: Risk, reason: &str, effect: EffectClass) -> PolicyResult {
    PolicyResult {
        decision: PolicyDecision::Allow,
        rule_id: rule_id.to_owned(),
        risk,
        reason: reason.to_owned(),
        effect,
    }
}

fn ask(rule_id: &str, risk: Risk, reason: &str, effect: EffectClass) -> PolicyResult {
    PolicyResult {
        decision: PolicyDecision::Ask,
        rule_id: rule_id.to_owned(),
        risk,
        reason: reason.to_owned(),
        effect,
    }
}

fn deny(rule_id: &str, risk: Risk, reason: &str, effect: EffectClass) -> PolicyResult {
    PolicyResult {
        decision: PolicyDecision::Deny,
        rule_id: rule_id.to_owned(),
        risk,
        reason: reason.to_owned(),
        effect,
    }
}

fn deny_parse() -> PolicyResult {
    deny(
        "command.parse",
        Risk::High,
        "command could not be parsed safely",
        EffectClass::ExternalEffect,
    )
}
