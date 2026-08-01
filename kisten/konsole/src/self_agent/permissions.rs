use std::io::{self, Write};

use orchester_laufzeit::harness::service::{SelfAgentPermissionRule, SelfAgentPermissionSnapshot};

use super::render::{policy_name, safe_terminal_text};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_permissions(
    out: &mut impl Write,
    permissions: &SelfAgentPermissionSnapshot,
) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Self-agent permissions{RESET}")?;
    render_configured_boundary(out, permissions)?;

    writeln!(out, "rules:")?;
    for rule in &permissions.rules {
        writeln!(
            out,
            "  {:<28} {} | rule {} | risk {:?} | effect {:?}",
            safe_terminal_text(&rule.label),
            configured_and_effective(rule),
            safe_terminal_text(&rule.policy_rule_id),
            rule.risk,
            rule.effect
        )?;
        writeln!(out, "    {DIM}{}{RESET}", safe_terminal_text(&rule.reason))?;
    }

    writeln!(
        out,
        "approvals: reviewer {} | ttl {}s | state machine {}",
        safe_terminal_text(&permissions.governance.approval_reviewer),
        permissions.governance.approval_ttl_seconds,
        if permissions.approvals.state_machine_present {
            "present"
        } else {
            "not present"
        }
    )?;
    writeln!(
        out,
        "CLI resolution: {}",
        if permissions.approvals.cli_resolution_available {
            "available"
        } else {
            "not available yet"
        }
    )?;
    writeln!(
        out,
        "audit: {}",
        if permissions.audit.append_only_hash_chain && permissions.audit.redacts_before_persistence
        {
            "append-only redacted hash chain"
        } else {
            "guarantees unavailable"
        }
    )?;
    writeln!(
        out,
        "{DIM}Read-only projection; no provider, credential, run-state, or audit-log access.{RESET}"
    )?;
    writeln!(out)
}

fn render_configured_boundary(
    out: &mut impl Write,
    permissions: &SelfAgentPermissionSnapshot,
) -> io::Result<()> {
    let network = required_rule(permissions, "network.external");
    let outside = required_rule(permissions, "path.out_of_workspace");
    let shell = required_rule(permissions, "shell.interpreter");
    writeln!(
        out,
        "network: configured {} | effective {}",
        policy_name(permissions.governance.network),
        policy_name(network.effective)
    )?;
    writeln!(
        out,
        "outside: configured {} | effective {}",
        policy_name(permissions.governance.out_of_workspace),
        policy_name(outside.effective)
    )?;
    writeln!(
        out,
        "shell: configured {} | effective {}",
        policy_name(permissions.governance.shell_interpreters),
        policy_name(shell.effective)
    )
}

fn required_rule<'a>(
    permissions: &'a SelfAgentPermissionSnapshot,
    id: &str,
) -> &'a SelfAgentPermissionRule {
    permissions
        .rules
        .iter()
        .find(|rule| rule.id == id)
        .expect("permission snapshot must contain its documented core rules")
}

fn configured_and_effective(rule: &SelfAgentPermissionRule) -> String {
    match rule.configured {
        Some(configured) => format!(
            "configured {} | effective {}",
            policy_name(configured),
            policy_name(rule.effective)
        ),
        None => format!("effective {}", policy_name(rule.effective)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::config::ConfigLoader;
    use orchester_laufzeit::harness::service::load_self_agent_permissions;

    #[test]
    fn rendering_separates_configured_choices_from_core_effective_guards() {
        let config = ConfigLoader::test()
            .load_user(
                r#"{
                    "governance": {
                        "approval_reviewer": "user\u001b[31m",
                        "tool_network": "deny",
                        "out_of_workspace": "allow",
                        "shell_interpreters": "allow",
                        "approval_ttl_seconds": 900
                    }
                }"#,
            )
            .expect("config");
        let permissions = load_self_agent_permissions(&config);
        let mut output = Vec::new();

        render_permissions(&mut output, &permissions).expect("render permissions");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("network: configured deny | effective deny"));
        assert!(rendered.contains("outside: configured allow | effective deny"));
        assert!(rendered.contains("shell: configured allow | effective deny"));
        assert!(rendered.contains("CLI resolution: not available yet"));
        assert!(rendered.contains("append-only redacted hash chain"));
        assert!(rendered.contains("user\\u{1b}[31m"));
        assert!(!rendered.contains("user\x1b[31m"));
    }
}
