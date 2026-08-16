use orchester_laufzeit::harness::service::{
    ConfigResolution, SelfAgentConfigView, SelfAgentModelReport, SelfAgentPermissionSnapshot,
    SelfAgentResumeAvailability, SelfAgentResumeCatalog, SelfAgentResumeStage, SelfAgentResumeStep,
    SelfAgentStatus,
};
use orchester_protokoll::PolicyDecision;
use orchester_verzeichnis::{PluginOrigin, RegisteredPlugin};

const INSPECT_FOOTER: &str = "Up/Down select  |  Enter inspect  |  Esc close";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceInspection {
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) entries: Vec<WorkspaceInspectionEntry>,
    pub(crate) footer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceInspectionEntry {
    pub(crate) label: String,
    pub(crate) detail: String,
    pub(crate) current: bool,
    pub(crate) details: Vec<String>,
}

impl WorkspaceInspectionEntry {
    fn new(label: impl Into<String>, detail: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            current: false,
            details,
        }
    }

    fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }
}

fn inspection(
    title: impl Into<String>,
    description: impl Into<String>,
    entries: Vec<WorkspaceInspectionEntry>,
) -> WorkspaceInspection {
    WorkspaceInspection {
        title: title.into(),
        description: description.into(),
        entries,
        footer: INSPECT_FOOTER.into(),
    }
}

pub(crate) fn status(status: &SelfAgentStatus) -> WorkspaceInspection {
    let model = match &status.model {
        SelfAgentModelReport::Configured(model) => WorkspaceInspectionEntry::new(
            "Model",
            format!("{} via {}", model.model, model.provider_name),
            vec![
                format!("model: {}", model.model),
                format!("provider: {} ({})", model.provider_name, model.provider),
                format!(
                    "reasoning: {} | plan {}",
                    optional_label(model.reasoning_effort.as_deref()),
                    optional_label(model.plan_reasoning_effort.as_deref())
                ),
                format!(
                    "responses: {} | service tier {}",
                    if model.store_responses {
                        "stored"
                    } else {
                        "not stored"
                    },
                    optional_label(model.service_tier.as_deref())
                ),
                format!(
                    "authentication: {}",
                    if model.requires_auth {
                        "required"
                    } else {
                        "not required"
                    }
                ),
            ],
        )
        .current(true),
        SelfAgentModelReport::Unresolved { path, message } => {
            WorkspaceInspectionEntry::new("Model", "unresolved", vec![format!("{path}: {message}")])
        }
        SelfAgentModelReport::NotConfigured => WorkspaceInspectionEntry::new(
            "Model",
            "not configured",
            vec!["No self-agent model is configured.".into()],
        ),
    };
    let governance = &status.governance;
    let limits = &status.limits;
    let durable = &status.durable;
    let durable_summary = if durable.database_present {
        format!(
            "{} resumable | {} ready",
            durable.resumable_runs, durable.ready_to_continue
        )
    } else {
        "not created".into()
    };

    inspection(
        "Self-agent status",
        "Select a section to inspect. No provider request is made.",
        vec![
            model,
            WorkspaceInspectionEntry::new(
                "Workspace",
                &status.workspace.canonical_root,
                vec![format!("directory: {}", status.workspace.canonical_root)],
            ),
            WorkspaceInspectionEntry::new(
                "Permissions",
                format!(
                    "network {} | outside {} | shell {}",
                    policy_label(governance.network),
                    policy_label(governance.out_of_workspace),
                    policy_label(governance.shell_interpreters)
                ),
                vec![
                    format!("reviewer: {}", governance.approval_reviewer),
                    format!("approval ttl: {}s", governance.approval_ttl_seconds),
                ],
            ),
            WorkspaceInspectionEntry::new(
                "Limits",
                format!(
                    "{} steps | {} minutes",
                    limits.max_steps, limits.max_minutes
                ),
                vec![
                    format!("maximum steps: {}", limits.max_steps),
                    format!("maximum minutes: {}", limits.max_minutes),
                    format!("repeated failures: {}", limits.max_same_failure),
                    format!("observation bytes: {}", limits.max_observation_bytes),
                ],
            ),
            WorkspaceInspectionEntry::new(
                "Durable state",
                durable_summary,
                vec![
                    format!("resumable: {}", durable.resumable_runs),
                    format!("ready: {}", durable.ready_to_continue),
                    format!("approval required: {}", durable.awaiting_approval),
                    format!("reconciliation: {}", durable.reconciliation_required),
                ],
            ),
        ],
    )
}

pub(crate) fn config(view: &SelfAgentConfigView) -> WorkspaceInspection {
    let mut entries = vec![
        WorkspaceInspectionEntry::new(
            "User configuration",
            if view.user_present {
                "present"
            } else {
                "absent"
            },
            vec![format!("path: {}", view.user_path.display())],
        ),
        WorkspaceInspectionEntry::new(
            "Project configuration",
            if view.project_present {
                "present"
            } else {
                "absent"
            },
            vec![format!("path: {}", view.project_path.display())],
        ),
    ];
    match &view.resolution {
        ConfigResolution::Loaded(config) => entries.push(
            WorkspaceInspectionEntry::new(
                "Resolved configuration",
                "loaded",
                config.json().lines().map(str::to_owned).collect(),
            )
            .current(true),
        ),
        ConfigResolution::Rejected { reason } => entries.push(WorkspaceInspectionEntry::new(
            "Resolved configuration",
            "rejected",
            vec![format!("reason: {reason}")],
        )),
    }
    entries.extend(view.diagnostics.iter().map(|finding| {
        WorkspaceInspectionEntry::new(
            format!("Permissions: {}", finding.path.display()),
            if finding.is_ok() {
                "secure"
            } else {
                "insecure"
            },
            vec![
                format!("expected: {}", finding.expected),
                format!(
                    "actual: {}",
                    finding.actual.as_deref().unwrap_or("not available")
                ),
                finding.message.clone(),
            ],
        )
    }));

    inspection(
        "Self-agent configuration",
        "Select a layer or diagnostic. Secret values remain redacted.",
        entries,
    )
}

pub(crate) fn permissions(permissions: &SelfAgentPermissionSnapshot) -> WorkspaceInspection {
    let mut entries = permissions
        .rules
        .iter()
        .map(|rule| {
            let configured = rule.configured.map(policy_label).unwrap_or("built in");
            WorkspaceInspectionEntry::new(
                &rule.label,
                format!("effective {}", policy_label(rule.effective)),
                vec![
                    format!("configured: {configured}"),
                    format!("effective: {}", policy_label(rule.effective)),
                    format!("policy rule: {}", rule.policy_rule_id),
                    format!("risk: {:?} | effect: {:?}", rule.risk, rule.effect),
                    rule.reason.clone(),
                ],
            )
        })
        .collect::<Vec<_>>();
    entries.push(WorkspaceInspectionEntry::new(
        "Approvals",
        if permissions.approvals.cli_resolution_available {
            "CLI resolution available"
        } else {
            "CLI resolution unavailable"
        },
        vec![
            format!("reviewer: {}", permissions.governance.approval_reviewer),
            format!("ttl: {}s", permissions.governance.approval_ttl_seconds),
            format!(
                "state machine: {}",
                if permissions.approvals.state_machine_present {
                    "present"
                } else {
                    "not present"
                }
            ),
        ],
    ));
    entries.push(WorkspaceInspectionEntry::new(
        "Audit",
        if permissions.audit.append_only_hash_chain && permissions.audit.redacts_before_persistence
        {
            "append-only redacted hash chain"
        } else {
            "guarantees unavailable"
        },
        vec![
            format!(
                "append-only: {}",
                yes_no(permissions.audit.append_only_hash_chain)
            ),
            format!(
                "redacted before persistence: {}",
                yes_no(permissions.audit.redacts_before_persistence)
            ),
            "This projection does not inspect an existing audit log.".into(),
        ],
    ));

    inspection(
        "Self-agent permissions",
        "Select a governed action to inspect its effective policy.",
        entries,
    )
}

pub(crate) fn resume(catalog: &SelfAgentResumeCatalog) -> WorkspaceInspection {
    let entries = catalog
        .entries
        .iter()
        .map(|entry| {
            let availability = resume_availability_label(entry.availability);
            let step = resume_step_label(entry.step);
            WorkspaceInspectionEntry::new(
                &entry.handle,
                format!("{availability} | {step}"),
                vec![
                    format!("availability: {availability}"),
                    format!("next step: {step}"),
                    "Continuation is read-only in this build; no run was changed.".into(),
                ],
            )
            .current(entry.latest)
        })
        .collect();

    inspection(
        "Resumable self-agent runs",
        if catalog.truncated {
            "Newest bounded entries are shown. Select one to inspect."
        } else {
            "Select a run to inspect its continuation state."
        },
        entries,
    )
}

pub(crate) fn plugins(plugins: &[RegisteredPlugin]) -> WorkspaceInspection {
    let entries = plugins
        .iter()
        .map(|plugin| {
            let info = plugin.info();
            let origin = match plugin.origin() {
                PluginOrigin::Managed => "managed",
                PluginOrigin::Project => "project",
            };
            WorkspaceInspectionEntry::new(
                info.display_name(),
                format!("{}@{}", info.package_name(), info.version()),
                vec![
                    format!("agent: {}", info.name()),
                    format!("package: {}", info.package_name()),
                    format!("version: {}", info.version()),
                    format!("origin: {origin}"),
                ],
            )
        })
        .collect();

    inspection(
        "Installed agent plugins",
        "Select a validated plugin to inspect its package and origin.",
        entries,
    )
}

fn optional_label(value: Option<&str>) -> &str {
    value.unwrap_or("default")
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn policy_label(value: PolicyDecision) -> &'static str {
    match value {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Ask => "ask",
        PolicyDecision::Deny => "deny",
    }
}

fn resume_availability_label(value: SelfAgentResumeAvailability) -> &'static str {
    match value {
        SelfAgentResumeAvailability::Ready => "ready",
        SelfAgentResumeAvailability::ApprovalRequired => "approval required",
        SelfAgentResumeAvailability::ReconciliationRequired => "reconciliation required",
    }
}

fn resume_step_label(value: SelfAgentResumeStep) -> String {
    match value {
        SelfAgentResumeStep::StartStep => "start step".into(),
        SelfAgentResumeStep::StartModel => "start model".into(),
        SelfAgentResumeStep::ProcessModelOutput => "process model output".into(),
        SelfAgentResumeStep::EvaluatePolicy => "evaluate policy".into(),
        SelfAgentResumeStep::PrepareExecution => "prepare execution".into(),
        SelfAgentResumeStep::StartNextStep => "start next step".into(),
        SelfAgentResumeStep::ContinueValidation => "continue validation".into(),
        SelfAgentResumeStep::CreateApprovalRequest => "create approval request".into(),
        SelfAgentResumeStep::AwaitApproval => "await approval".into(),
        SelfAgentResumeStep::RecoverApprovalCapability => "recover approval capability".into(),
        SelfAgentResumeStep::ReconcileModelCall => "reconcile model call".into(),
        SelfAgentResumeStep::ReconcileToolOutcome => "reconcile tool outcome".into(),
        SelfAgentResumeStep::ManualReconciliation(stage) => {
            format!("manual reconciliation ({})", resume_stage_label(stage))
        }
    }
}

fn resume_stage_label(value: SelfAgentResumeStage) -> &'static str {
    match value {
        SelfAgentResumeStage::MissingStep => "missing step",
        SelfAgentResumeStage::ModelCall => "model call",
        SelfAgentResumeStage::ToolOutcome => "tool outcome",
        SelfAgentResumeStage::UnboundApproval => "unbound approval",
        SelfAgentResumeStage::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use orchester_laufzeit::harness::config::ConfigLoader;
    use orchester_laufzeit::harness::service::{
        load_self_agent_permissions, SelfAgentDurableStatus, SelfAgentGovernanceStatus,
        SelfAgentLimitStatus, SelfAgentModelStatus, SelfAgentResumeEntry,
        WorkspaceIdentitySnapshot,
    };

    #[test]
    fn status_groups_operational_sections_without_internal_identity_fields() {
        let snapshot = SelfAgentStatus {
            workspace: WorkspaceIdentitySnapshot {
                project_id: "private-project".into(),
                workspace_identity: "private-workspace".into(),
                canonical_root: "D:\\workspace".into(),
                owner_actor_id: "private-owner".into(),
            },
            model: SelfAgentModelReport::Configured(SelfAgentModelStatus {
                provider: "openai".into(),
                provider_name: "OpenAI".into(),
                model: "gpt-test".into(),
                reasoning_effort: Some("high".into()),
                plan_reasoning_effort: None,
                store_responses: false,
                service_tier: Some("default".into()),
                requires_auth: true,
            }),
            governance: SelfAgentGovernanceStatus {
                approval_reviewer: "user".into(),
                network: PolicyDecision::Ask,
                out_of_workspace: PolicyDecision::Deny,
                shell_interpreters: PolicyDecision::Deny,
                approval_ttl_seconds: 60,
            },
            limits: SelfAgentLimitStatus {
                max_steps: 80,
                max_minutes: 30,
                max_same_failure: 3,
                max_observation_bytes: 65_536,
            },
            durable: SelfAgentDurableStatus {
                database_present: true,
                resumable_runs: 2,
                ready_to_continue: 1,
                awaiting_approval: 1,
                reconciliation_required: 0,
            },
        };

        let view = status(&snapshot);

        assert_eq!(
            view.entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Model",
                "Workspace",
                "Permissions",
                "Limits",
                "Durable state"
            ]
        );
        assert!(view.entries[0].current);
        assert!(view.entries[0]
            .details
            .iter()
            .any(|line| line.contains("reasoning: high")));
        let rendered = format!("{view:?}");
        assert!(!rendered.contains("private-project"));
        assert!(!rendered.contains("private-workspace"));
        assert!(!rendered.contains("private-owner"));
    }

    #[test]
    fn config_permissions_and_resume_are_typed_inspection_lists() {
        let config_view = SelfAgentConfigView {
            user_path: PathBuf::from("D:\\home\\orchester.jsonc"),
            user_present: true,
            project_path: PathBuf::from("D:\\repo\\.orchester\\project.jsonc"),
            project_present: false,
            resolution: ConfigResolution::Rejected {
                reason: "invalid field".into(),
            },
            diagnostics: Vec::new(),
        };
        let config = config(&config_view);
        assert_eq!(config.entries[0].label, "User configuration");
        assert_eq!(config.entries[2].label, "Resolved configuration");
        assert_eq!(config.entries[2].detail, "rejected");

        let user_config = ConfigLoader::test().load_user("{}").expect("config");
        let permissions = permissions(&load_self_agent_permissions(&user_config));
        assert!(permissions
            .entries
            .iter()
            .any(|entry| entry.label == "external network"));
        assert!(permissions
            .entries
            .iter()
            .any(|entry| entry.label == "Approvals"));

        let catalog = SelfAgentResumeCatalog {
            database_present: true,
            truncated: false,
            entries: vec![SelfAgentResumeEntry {
                handle: "r-safe".into(),
                availability: SelfAgentResumeAvailability::Ready,
                step: SelfAgentResumeStep::StartModel,
                latest: true,
            }],
        };
        let resume = resume(&catalog);
        assert_eq!(resume.entries[0].label, "r-safe");
        assert!(resume.entries[0].current);
        assert!(resume.entries[0]
            .details
            .iter()
            .any(|line| line.contains("read-only")));

        let plugins = plugins(&[]);
        assert_eq!(plugins.title, "Installed agent plugins");
        assert!(plugins.entries.is_empty());
    }
}
