use std::io::{self, Write};

use orchester_laufzeit::harness::service::SelfAgentStatus;

use super::render::{policy_name, safe_terminal_text};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_status(out: &mut impl Write, status: &SelfAgentStatus) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Self-agent status{RESET}")?;
    writeln!(
        out,
        "workspace: {}",
        safe_terminal_text(&status.workspace.canonical_root)
    )?;

    if let Some(model) = &status.model {
        writeln!(out, "model: {}", safe_terminal_text(&model.model))?;
        writeln!(
            out,
            "provider: {} ({})",
            safe_terminal_text(&model.provider_name),
            safe_terminal_text(&model.provider)
        )?;
        writeln!(
            out,
            "reasoning: {} | plan {}",
            optional_value(model.reasoning_effort.as_deref()),
            optional_value(model.plan_reasoning_effort.as_deref())
        )?;
        writeln!(
            out,
            "responses: {} | service tier {} | auth {}",
            if model.store_responses {
                "stored"
            } else {
                "not stored"
            },
            optional_value(model.service_tier.as_deref()),
            if model.requires_auth {
                "required"
            } else {
                "not required"
            }
        )?;
    } else {
        writeln!(out, "model: not configured")?;
    }

    writeln!(
        out,
        "permissions: network {} | outside {} | shell {}",
        policy_name(status.governance.network),
        policy_name(status.governance.out_of_workspace),
        policy_name(status.governance.shell_interpreters)
    )?;
    writeln!(
        out,
        "approvals: {} | ttl {}s",
        safe_terminal_text(&status.governance.approval_reviewer),
        status.governance.approval_ttl_seconds
    )?;
    writeln!(
        out,
        "limits: max steps {} | max minutes {} | repeated failures {}",
        status.limits.max_steps, status.limits.max_minutes, status.limits.max_same_failure
    )?;
    writeln!(
        out,
        "observations: max {} bytes",
        status.limits.max_observation_bytes
    )?;

    if status.durable.database_present {
        writeln!(
            out,
            "state: resumable {} | ready {} | approval {} | reconcile {}",
            status.durable.resumable_runs,
            status.durable.ready_to_continue,
            status.durable.awaiting_approval,
            status.durable.reconciliation_required
        )?;
    } else {
        writeln!(out, "state: not created")?;
    }
    writeln!(out, "{DIM}No provider request was made.{RESET}")?;
    writeln!(out)
}

fn optional_value(value: Option<&str>) -> String {
    value
        .map(safe_terminal_text)
        .unwrap_or_else(|| "default".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::service::{
        SelfAgentDurableStatus, SelfAgentGovernanceStatus, SelfAgentLimitStatus,
        SelfAgentModelStatus, WorkspaceIdentitySnapshot,
    };
    use orchester_protokoll::PolicyDecision;

    fn status() -> SelfAgentStatus {
        SelfAgentStatus {
            workspace: WorkspaceIdentitySnapshot {
                project_id: "private-project-id".into(),
                workspace_identity: "private-workspace-id".into(),
                canonical_root: "C:\\workspace\x1b[31m".into(),
                owner_actor_id: "private-owner-id".into(),
            },
            model: Some(SelfAgentModelStatus {
                provider: "OpenAI".into(),
                provider_name: "OpenAI API".into(),
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
                resumable_runs: 4,
                ready_to_continue: 1,
                awaiting_approval: 2,
                reconciliation_required: 1,
            },
        }
    }

    #[test]
    fn rendering_exposes_safe_operational_status_without_internal_ids() {
        let mut output = Vec::new();
        render_status(&mut output, &status()).expect("render status");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("Self-agent status"));
        assert!(rendered.contains("model: gpt-test"));
        assert!(rendered.contains("network ask | outside deny | shell deny"));
        assert!(rendered.contains("max steps 80"));
        assert!(rendered.contains("resumable 4 | ready 1 | approval 2 | reconcile 1"));
        assert!(rendered.contains("\\u{1b}[31m"));
        assert!(!rendered.contains("private-project-id"));
        assert!(!rendered.contains("private-workspace-id"));
        assert!(!rendered.contains("private-owner-id"));
        assert!(!rendered.contains("C:\\workspace\x1b[31m"));
    }

    #[test]
    fn rendering_reports_unconfigured_model_and_absent_database() {
        let mut status = status();
        status.model = None;
        status.durable = SelfAgentDurableStatus::default();
        let mut output = Vec::new();

        render_status(&mut output, &status).expect("render status");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("model: not configured"));
        assert!(rendered.contains("state: not created"));
        assert!(rendered.contains("No provider request was made."));
    }
}
