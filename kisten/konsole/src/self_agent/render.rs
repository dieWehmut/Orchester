use std::io::{self, Write};

use orchester_laufzeit::harness::execution::GovernedToolOutcome;
use orchester_laufzeit::harness::service::{SelfAgentRunOutcome, SelfAgentTurn};
use orchester_protokoll::{Observation, PolicyDecision};
use serde_json::Value;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_outcome(out: &mut impl Write, outcome: &SelfAgentRunOutcome) -> io::Result<()> {
    let usage = outcome.usage();
    render_parts(
        out,
        outcome.tool_steps().iter().map(|step| step.outcome()),
        outcome.final_turn(),
        outcome.model_calls(),
        usage.input_tokens,
        usage.output_tokens,
    )
}

/// Capture the human-readable outcome for the full-screen transcript.
///
/// The normal renderer writes to the terminal and uses only the local DIM and
/// RESET styles. The transcript stores plain text so the frame renderer can
/// apply its own role colors without leaking terminal control sequences.
pub fn render_outcome_transcript(outcome: &SelfAgentRunOutcome) -> io::Result<String> {
    let mut rendered = Vec::new();
    render_outcome(&mut rendered, outcome)?;
    let rendered = String::from_utf8(rendered).map_err(io::Error::other)?;
    Ok(strip_render_styles(&rendered).trim().to_owned())
}

fn strip_render_styles(text: &str) -> String {
    text.replace(DIM, "").replace(RESET, "")
}

fn render_parts<'a>(
    out: &mut impl Write,
    tools: impl IntoIterator<Item = &'a GovernedToolOutcome>,
    final_turn: &SelfAgentTurn,
    model_calls: u32,
    input_tokens: u64,
    output_tokens: u64,
) -> io::Result<()> {
    writeln!(out)?;
    for tool in tools {
        render_tool_outcome(out, tool)?;
        writeln!(out)?;
    }
    render_model_turn(out, final_turn)?;

    writeln!(
        out,
        "{DIM}-> model calls {} | tokens in {} / out {}{RESET}",
        model_calls, input_tokens, output_tokens
    )?;
    writeln!(out)
}

fn render_tool_outcome(out: &mut impl Write, outcome: &GovernedToolOutcome) -> io::Result<()> {
    match outcome {
        GovernedToolOutcome::Completed(observation) => render_observation(out, observation),
        GovernedToolOutcome::Failed(feedback) => {
            writeln!(out, "tool failed")?;
            writeln!(out, "{}", safe_terminal_text(&feedback.summary))?;
            writeln!(
                out,
                "{DIM}retryable: {}{RESET}",
                if feedback.retryable { "yes" } else { "no" }
            )
        }
    }
}

fn render_model_turn(out: &mut impl Write, turn: &SelfAgentTurn) -> io::Result<()> {
    match turn {
        SelfAgentTurn::Text { text, .. } => writeln!(out, "{}", safe_terminal_text(text)),
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
            writeln!(out, "{DIM}{state}{RESET}")
        }
    }
}

fn render_observation(out: &mut impl Write, observation: &Observation) -> io::Result<()> {
    writeln!(out, "tool: {}", safe_terminal_text(&observation.kind))?;
    writeln!(
        out,
        "{DIM}{}{RESET}",
        safe_terminal_text(&observation.summary)
    )?;

    match observation.kind.as_str() {
        "read_file" => render_content_lines(out, &observation.data),
        "list_files" => render_file_entries(out, &observation.data),
        "search_text" => render_search_matches(out, &observation.data),
        _ => render_json(out, &observation.data),
    }
}

fn render_content_lines(out: &mut impl Write, data: &Value) -> io::Result<()> {
    let Some(lines) = data.get("content_lines").and_then(Value::as_array) else {
        return render_json(out, data);
    };
    for line in lines.iter().filter_map(Value::as_str) {
        writeln!(out, "{}", safe_terminal_text(line))?;
    }
    Ok(())
}

fn render_file_entries(out: &mut impl Write, data: &Value) -> io::Result<()> {
    let Some(entries) = data.get("entries").and_then(Value::as_array) else {
        return render_json(out, data);
    };
    for entry in entries {
        let kind = entry.get("kind").and_then(Value::as_str).unwrap_or("entry");
        let path = entry.get("path").and_then(Value::as_str).unwrap_or("?");
        writeln!(
            out,
            "{:<9} {}",
            safe_terminal_text(kind),
            safe_terminal_text(path)
        )?;
    }
    Ok(())
}

fn render_search_matches(out: &mut impl Write, data: &Value) -> io::Result<()> {
    let Some(matches) = data.get("matches").and_then(Value::as_array) else {
        return render_json(out, data);
    };
    for found in matches {
        let path = found.get("path").and_then(Value::as_str).unwrap_or("?");
        let line = found.get("line").and_then(Value::as_u64).unwrap_or(0);
        let text = found.get("text").and_then(Value::as_str).unwrap_or("");
        writeln!(
            out,
            "{}:{} {}",
            safe_terminal_text(path),
            line,
            safe_terminal_text(text)
        )?;
    }
    Ok(())
}

fn render_json(out: &mut impl Write, data: &Value) -> io::Result<()> {
    let encoded = serde_json::to_string_pretty(data).map_err(io::Error::other)?;
    writeln!(out, "{}", safe_terminal_text(&encoded))
}

pub(super) fn policy_name(decision: PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::Allow => "allow",
        PolicyDecision::Ask => "ask",
        PolicyDecision::Deny => "deny",
    }
}

pub(super) fn safe_terminal_text(text: &str) -> String {
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
    use orchester_protokoll::{
        ActionId, AgentAction, CallId, FeedbackReport, ObservationId, RunId,
    };

    fn render_model(turn: SelfAgentTurn) -> String {
        let mut output = Vec::new();
        let usage = turn.usage();
        render_parts(
            &mut output,
            std::iter::empty(),
            &turn,
            turn.model_calls(),
            usage.input_tokens,
            usage.output_tokens,
        )
        .expect("render");
        String::from_utf8(output).expect("UTF-8")
    }

    fn render_tool(outcome: GovernedToolOutcome) -> String {
        let mut output = Vec::new();
        let final_turn = SelfAgentTurn::Text {
            run_id: RunId::from("run-1"),
            text: "done".into(),
            model_calls: 2,
            usage: Default::default(),
        };
        render_parts(
            &mut output,
            std::iter::once(&outcome),
            &final_turn,
            final_turn.model_calls(),
            final_turn.usage().input_tokens,
            final_turn.usage().output_tokens,
        )
        .expect("render");
        String::from_utf8(output).expect("UTF-8")
    }

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
        let rendered = render_model(turn);

        assert!(rendered.contains("first\n\\u{1b}[31msecond"));
        assert!(!rendered.contains("\x1b[31msecond"));
        assert!(rendered.contains("model calls 1 | tokens in 0 / out 0"));
    }

    #[test]
    fn transcript_style_stripping_removes_renderer_codes_only() {
        let rendered = format!("{DIM}status{RESET} value");
        assert_eq!(strip_render_styles(&rendered), "status value");
    }

    #[test]
    fn action_turn_rendering_uses_the_bounded_summary() {
        let turn = action_turn(AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        });
        let rendered = render_model(turn);

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
        let rendered = render_model(turn);

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
        let rendered = render_model(turn);

        assert!(rendered.contains("policy: deny | rule system.destructive | risk Critical"));
        assert!(rendered.contains("blocked by policy"));
    }

    #[test]
    fn completed_read_rendering_shows_the_sanitized_content_lines() {
        let outcome = GovernedToolOutcome::Completed(Observation {
            observation_id: ObservationId::from("observation-1"),
            call_id: CallId::from("call-1"),
            kind: "read_file".into(),
            summary: "read bytes=12 lines=2".into(),
            data: serde_json::json!({"content_lines": ["first", "second"]}),
        });
        let rendered = render_tool(outcome);

        assert!(rendered.contains("tool: read_file"));
        assert!(rendered.contains("first\nsecond"));
        assert!(rendered.contains("done"));
        assert!(rendered.contains("model calls 2 | tokens in 0 / out 0"));
    }

    #[test]
    fn failed_tool_rendering_uses_only_the_sanitized_feedback_summary() {
        let outcome = GovernedToolOutcome::Failed(FeedbackReport {
            source: "tool_executor".into(),
            validator_id: None,
            exit_code: None,
            classification: "tool_failed".into(),
            summary: "workspace filesystem operation failed".into(),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            fingerprint: "fingerprint".into(),
            retryable: true,
        });
        let rendered = render_tool(outcome);

        assert!(rendered.contains("tool failed"));
        assert!(rendered.contains("workspace filesystem operation failed"));
        assert!(!rendered.contains("fingerprint"));
    }
}
