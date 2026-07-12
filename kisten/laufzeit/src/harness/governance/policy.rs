//! Deterministic policy decisions over structured action intents.

use std::ffi::{OsStr, OsString};

use orchester_protokoll::{AgentAction, PolicyDecision};
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
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn new() -> Self {
        Self
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
            AgentAction::RequestApproval { .. } => allow(
                "approval.request",
                Risk::Low,
                "approval request is a control-plane operation",
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
        if categories.contains(&CommandCategory::ShellInterpreter) {
            return deny(
                "shell.interpreter",
                Risk::High,
                "shell and scripting interpreters are disabled",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::PrivilegeEscalation) {
            return deny(
                "privilege.escalation",
                Risk::Critical,
                "privilege escalation is disabled",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::UnsupportedWrapper) {
            return deny(
                "command.wrapper",
                Risk::High,
                "command wrappers can hide the executable or alter its environment",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::Composite) {
            return deny(
                "command.composite",
                Risk::High,
                "shell composition and redirection tokens are not accepted",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::SystemDestructive) {
            return deny(
                "system.destructive",
                Risk::Critical,
                "system or root-targeted destructive operation is disabled",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::GitDestructive) {
            return deny(
                "git.destructive",
                Risk::High,
                "destructive Git history or repository operation requires denial",
                EffectClass::WorkspaceMutation,
            );
        }
        if categories.contains(&CommandCategory::PackageInstall) {
            return ask(
                "dependency.install",
                Risk::Medium,
                "dependency installation can execute code and access the network",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::Network) {
            return ask(
                "network.external",
                Risk::Medium,
                "external network access requires human approval",
                EffectClass::ExternalEffect,
            );
        }
        if categories.contains(&CommandCategory::Delete) {
            return ask(
                "filesystem.delete",
                Risk::Medium,
                "workspace deletion requires human approval",
                EffectClass::WorkspaceMutation,
            );
        }
        if categories.contains(&CommandCategory::GitWrite) {
            return ask(
                "git.write",
                Risk::Medium,
                "Git repository mutation requires human approval",
                EffectClass::WorkspaceMutation,
            );
        }
        if categories.contains(&CommandCategory::WorkspaceWrite) {
            return ask(
                "command.may_mutate",
                Risk::Medium,
                "the command may execute project code or write generated state",
                EffectClass::MayMutate,
            );
        }
        if categories.contains(&CommandCategory::ReadOnly) {
            return allow(
                "workspace.read",
                Risk::Low,
                "command is on the explicit read-only allowlist",
                EffectClass::ReadOnlyIdempotent,
            );
        }
        deny(
            "command.unknown",
            Risk::High,
            "executable is not in the governed command catalog",
            EffectClass::ExternalEffect,
        )
    }
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
