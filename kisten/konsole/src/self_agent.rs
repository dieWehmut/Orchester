use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;

use orchester_laufzeit::harness::config::{ConfigError, ConfigLoader};
use orchester_laufzeit::harness::credentials::KeyringCredentialStore;
use orchester_laufzeit::harness::service::{
    build_self_agent_service, ProductionSelfAgentService, SelfAgentBuildError,
    SelfAgentServiceError, SelfAgentTurn,
};
use orchester_protokoll::PolicyDecision;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Error)]
pub enum SelfAgentHostError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Build(#[from] SelfAgentBuildError),
    #[error(transparent)]
    Run(#[from] SelfAgentServiceError),
    #[error("self-agent service initialization failed")]
    Initialization,
}

pub struct SelfAgentHost {
    workspace: PathBuf,
    state_database: PathBuf,
    service: Option<ProductionSelfAgentService>,
}

impl SelfAgentHost {
    pub fn new(workspace: PathBuf, state_database: PathBuf) -> Self {
        Self {
            workspace,
            state_database,
            service: None,
        }
    }

    pub async fn submit(
        &mut self,
        prompt: String,
        cancel: CancellationToken,
    ) -> Result<SelfAgentTurn, SelfAgentHostError> {
        self.ensure_service()?;
        let service = self
            .service
            .as_ref()
            .ok_or(SelfAgentHostError::Initialization)?;
        service.start(prompt, cancel).await.map_err(Into::into)
    }

    fn ensure_service(&mut self) -> Result<(), SelfAgentHostError> {
        if self.service.is_some() {
            return Ok(());
        }
        let config = ConfigLoader::new()?.load_effective(&self.workspace)?;
        let credentials = KeyringCredentialStore::new();
        self.service = Some(build_self_agent_service(
            &config,
            &credentials,
            &self.workspace,
            &self.state_database,
            "local-user",
        )?);
        Ok(())
    }
}

impl fmt::Debug for SelfAgentHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentHost")
            .field("workspace", &"[REDACTED]")
            .field("state_database", &"[REDACTED]")
            .field("initialized", &self.service.is_some())
            .finish()
    }
}

pub fn render_turn(out: &mut impl Write, turn: &SelfAgentTurn) -> io::Result<()> {
    writeln!(out)?;
    match turn {
        SelfAgentTurn::Text { text, .. } => writeln!(out, "{}", safe_terminal_text(text))?,
        SelfAgentTurn::Action { action, policy, .. } => {
            writeln!(
                out,
                "action: {}",
                safe_terminal_text(&action.action_summary())
            )?;
            writeln!(
                out,
                "policy: {} | rule {} | risk {:?}",
                policy_name(policy.decision),
                safe_terminal_text(&policy.rule_id),
                policy.risk
            )?;
            let state = match policy.decision {
                PolicyDecision::Allow => "ready for governed execution",
                PolicyDecision::Ask => "human approval required",
                PolicyDecision::Deny => "blocked by policy",
            };
            writeln!(out, "{DIM}{state}{RESET}")?;
        }
    }
    let usage = turn.usage();
    writeln!(
        out,
        "{DIM}-> model calls {} | tokens in {} / out {}{RESET}",
        turn.model_calls(),
        usage.input_tokens,
        usage.output_tokens
    )?;
    writeln!(out)
}

fn policy_name(decision: PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Ask => "ask",
        PolicyDecision::Deny => "deny",
    }
}

fn safe_terminal_text(text: &str) -> String {
    text.chars()
        .flat_map(|character| match character {
            '\n' | '\t' => vec![character],
            _ if character.is_control() => character.escape_default().collect(),
            _ => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::governance::PolicyEngine;
    use orchester_protokoll::{ActionId, AgentAction, CallId, RunId};

    fn action_turn(action: AgentAction) -> SelfAgentTurn {
        let policy = PolicyEngine::new().evaluate(&action).expect("policy");
        SelfAgentTurn::Action {
            run_id: RunId::from("run-1"),
            action_id: ActionId::from("action-1"),
            call_id: CallId::from("call-1"),
            action,
            policy,
            model_calls: 1,
            usage: Default::default(),
        }
    }

    #[test]
    fn text_turn_rendering_preserves_lines_and_escapes_terminal_controls() {
        let turn = SelfAgentTurn::Text {
            run_id: RunId::from("run-1"),
            text: "first\n\x1b[31msecond".into(),
            model_calls: 1,
            usage: Default::default(),
        };
        let mut output = Vec::new();
        render_turn(&mut output, &turn).expect("render");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("first\n\\u{1b}[31msecond"));
        assert!(!rendered.contains("\x1b[31msecond"));
        assert!(rendered.contains("model calls 1 | tokens in 0 / out 0"));
    }

    #[test]
    fn action_turn_rendering_uses_the_bounded_summary() {
        let turn = action_turn(AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        });
        let mut output = Vec::new();
        render_turn(&mut output, &turn).expect("render");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("action: read_file path_bytes=10 start_line=None end_line=None"));
        assert!(rendered.contains("policy: allow | rule workspace.read | risk Low"));
        assert!(rendered.contains("ready for governed execution"));
    }

    #[test]
    fn ask_policy_rendering_requests_human_approval() {
        let turn = action_turn(AgentAction::RunCommand {
            program: "curl".into(),
            args: vec!["https://example.test".into()],
            cwd: None,
        });
        let mut output = Vec::new();
        render_turn(&mut output, &turn).expect("render");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("policy: ask | rule network.external | risk Medium"));
        assert!(rendered.contains("human approval required"));
    }

    #[test]
    fn deny_policy_rendering_reports_the_policy_block() {
        let turn = action_turn(AgentAction::RunCommand {
            program: "rm".into(),
            args: vec!["-rf".into(), "/".into()],
            cwd: None,
        });
        let mut output = Vec::new();
        render_turn(&mut output, &turn).expect("render");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("policy: deny | rule system.destructive | risk Critical"));
        assert!(rendered.contains("blocked by policy"));
    }
}
