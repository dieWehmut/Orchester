use std::io::{self, Write};

use orchester_laufzeit::harness::execution::GovernedToolOutcome;
use orchester_laufzeit::harness::service::{SelfAgentRunOutcome, SelfAgentTurn};
use orchester_protokoll::{Observation, PolicyDecision};
use serde_json::Value;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// The transcript's vocabulary, as the reference draws it.
///
/// A line of prose opens with a bullet and hangs its continuation under it; a
/// tool step opens with the bullet too, carries its body on a rail, and closes
/// the rail with a corner that says what the step produced and how much of it
/// was left out. The glyphs are part of the text rather than the styling,
/// because the transcript stores plain text and the frame renderer colours it.
const BULLET: &str = "•";
const RAIL: &str = "│";
const CORNER: &str = "└";
const INDENT: &str = "  ";

/// How many body lines a step shows before the corner counts the rest.
///
/// The reference puts the whole body behind a transcript view; this product has
/// no such view, so the count stands alone rather than promising a key that
/// does nothing. The block is bounded because one long read would otherwise
/// bury the answer that follows it.
const BODY_LINES: usize = 8;

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
            let closing = format!(
                "tool failed · retryable: {}",
                if feedback.retryable { "yes" } else { "no" }
            );
            render_block(
                out,
                "Failed",
                &lines_of(&feedback.summary),
                &closing,
            )
        }
    }
}

fn render_model_turn(out: &mut impl Write, turn: &SelfAgentTurn) -> io::Result<()> {
    match turn {
        SelfAgentTurn::Text { text, .. } => render_prose(out, text),
        SelfAgentTurn::Action { action, policy, .. } => {
            let state = match policy.decision {
                PolicyDecision::Allow => "ready for governed execution",
                PolicyDecision::Ask => "human approval required",
                PolicyDecision::Deny => "blocked by policy",
            };
            let header = format!(
                "Action {}",
                safe_terminal_text(&action.action_summary())
            );
            render_block(
                out,
                &header,
                &lines_of(&format!(
                    "policy: {} | rule {} | risk {:?}",
                    policy_name(policy.decision),
                    safe_terminal_text(&policy.rule_id),
                    policy.risk
                )),
                state,
            )
        }
    }
}

/// A line of prose: the bullet on the first line, the rest hung under it.
fn render_prose(out: &mut impl Write, text: &str) -> io::Result<()> {
    let lines = lines_of(text);
    if lines.is_empty() {
        return writeln!(out, "{BULLET}");
    }
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            writeln!(out, "{BULLET} {line}")?;
        } else {
            writeln!(out, "{INDENT}{line}")?;
        }
    }
    Ok(())
}

/// A step: the bullet and its verb, the body on a rail, and the corner.
///
/// The corner carries the step's own summary, so the last line of a block says
/// what the step did, and `… +N lines` when the body was bounded.
fn render_block(
    out: &mut impl Write,
    header: &str,
    body: &[String],
    closing: &str,
) -> io::Result<()> {
    writeln!(out, "{BULLET} {header}")?;
    let shown = body.len().min(BODY_LINES);
    for line in &body[..shown] {
        writeln!(out, "{INDENT}{RAIL} {line}")?;
    }
    let hidden = body.len().saturating_sub(shown);
    let closing = if hidden > 0 {
        format!("… +{hidden} lines · {closing}")
    } else {
        closing.to_owned()
    };
    writeln!(out, "{INDENT}{CORNER} {closing}")?;
    Ok(())
}

/// The text as body lines: control characters escaped, tabs and spaces kept.
fn lines_of(text: &str) -> Vec<String> {
    let escaped = safe_terminal_text(text);
    let lines: Vec<String> = escaped
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect();
    if lines.iter().all(|line| line.is_empty()) {
        Vec::new()
    } else {
        lines
    }
}

/// What ran, in the reference's "Ran" position: the verb of the step.
fn step_verb(kind: &str) -> &str {
    match kind {
        "read_file" => "Read",
        "list_files" => "List",
        "search_text" => "Search",
        other => other,
    }
}

fn render_observation(out: &mut impl Write, observation: &Observation) -> io::Result<()> {
    let body = observation_body(observation);
    render_block(
        out,
        step_verb(&observation.kind),
        &body,
        &safe_terminal_text(&observation.summary),
    )
}

/// The lines a step produced, as plain text rather than as a JSON dump.
fn observation_body(observation: &Observation) -> Vec<String> {
    match observation.kind.as_str() {
        "read_file" => content_lines(&observation.data),
        "list_files" => file_entries(&observation.data),
        "search_text" => search_matches(&observation.data),
        _ => json_lines(&observation.data),
    }
}

fn content_lines(data: &Value) -> Vec<String> {
    let Some(lines) = data.get("content_lines").and_then(Value::as_array) else {
        return json_lines(data);
    };
    lines
        .iter()
        .filter_map(Value::as_str)
        .map(|line| safe_terminal_text(line).trim_end().to_owned())
        .collect()
}

fn file_entries(data: &Value) -> Vec<String> {
    let Some(entries) = data.get("entries").and_then(Value::as_array) else {
        return json_lines(data);
    };
    entries
        .iter()
        .map(|entry| {
            let kind = entry.get("kind").and_then(Value::as_str).unwrap_or("entry");
            let path = entry.get("path").and_then(Value::as_str).unwrap_or("?");
            format!(
                "{:<9} {}",
                safe_terminal_text(kind),
                safe_terminal_text(path)
            )
        })
        .collect()
}

fn search_matches(data: &Value) -> Vec<String> {
    let Some(matches) = data.get("matches").and_then(Value::as_array) else {
        return json_lines(data);
    };
    matches
        .iter()
        .map(|found| {
            let path = found.get("path").and_then(Value::as_str).unwrap_or("?");
            let line = found.get("line").and_then(Value::as_u64).unwrap_or(0);
            let text = found.get("text").and_then(Value::as_str).unwrap_or("");
            format!(
                "{}:{} {}",
                safe_terminal_text(path),
                line,
                safe_terminal_text(text)
            )
        })
        .collect()
}

fn json_lines(data: &Value) -> Vec<String> {
    match serde_json::to_string_pretty(data) {
        Ok(encoded) => lines_of(&encoded),
        Err(_) => Vec::new(),
    }
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

        // The bullet opens the prose and the continuation hangs under it, which
        // is the shape the reference draws a paragraph in.
        assert!(rendered.contains("• first\n  \\u{1b}[31msecond"), "{rendered}");
        assert!(!rendered.contains("\x1b[31msecond"));
        assert!(rendered.contains("model calls 1 | tokens in 0 / out 0"));
    }

    #[test]
    fn a_step_draws_a_verb_a_rail_and_a_corner_with_its_summary() {
        let outcome = GovernedToolOutcome::Completed(Observation {
            observation_id: ObservationId::from("observation-1"),
            call_id: CallId::from("call-1"),
            kind: "read_file".into(),
            summary: "read bytes=12 lines=2".into(),
            data: serde_json::json!({"content_lines": ["first", "second"]}),
        });
        let rendered = render_tool(outcome);

        assert!(rendered.contains("• Read\n"), "{rendered}");
        assert!(rendered.contains("  │ first\n"), "{rendered}");
        assert!(rendered.contains("  │ second\n"), "{rendered}");
        // The corner closes the rail and says what the step produced, rather
        // than repeating "tool: read_file" and the summary above the body.
        assert!(rendered.contains("  └ read bytes=12 lines=2"), "{rendered}");
        assert!(!rendered.contains("tool: read_file"), "{rendered}");
        assert!(!rendered.contains("└ … +"), "{rendered}");
    }

    #[test]
    fn a_long_body_is_bounded_and_its_corner_counts_the_rest() {
        let lines = (0..20).map(|i| format!("line {i}")).collect::<Vec<_>>();
        let outcome = GovernedToolOutcome::Completed(Observation {
            observation_id: ObservationId::from("observation-1"),
            call_id: CallId::from("call-1"),
            kind: "read_file".into(),
            summary: "read bytes=99 lines=20".into(),
            data: serde_json::json!({"content_lines": lines}),
        });
        let rendered = render_tool(outcome);

        assert!(rendered.contains("  │ line 0\n"), "{rendered}");
        assert!(rendered.contains("  │ line 7\n"), "{rendered}");
        assert!(!rendered.contains("line 8"), "{rendered}");
        assert!(
            rendered.contains("  └ … +12 lines · read bytes=99 lines=20"),
            "{rendered}"
        );
        // No key is promised: this product has no transcript view to open, and
        // a hint that opens nothing is worse than the count alone.
        assert!(!rendered.contains("ctrl+"), "{rendered}");
    }

    #[test]
    fn a_list_body_keeps_its_columns_and_its_verb_names_the_step() {
        let outcome = GovernedToolOutcome::Completed(Observation {
            observation_id: ObservationId::from("observation-1"),
            call_id: CallId::from("call-1"),
            kind: "list_files".into(),
            summary: "listed entries=2".into(),
            data: serde_json::json!({"entries": [
                {"kind": "directory", "path": "src"},
                {"kind": "file", "path": "README.md"},
            ]}),
        });
        let rendered = render_tool(outcome);

        assert!(rendered.contains("• List\n"), "{rendered}");
        assert!(rendered.contains("  │ directory src\n"), "{rendered}");
        assert!(rendered.contains("  │ file      README.md\n"), "{rendered}");
        assert!(rendered.contains("  └ listed entries=2"), "{rendered}");
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

        assert!(
            rendered.contains("• Action read_file path_bytes=10 start_line=None end_line=None"),
            "{rendered}"
        );
        assert!(rendered.contains("  │ policy: allow | rule workspace.read | risk Low"), "{rendered}");
        assert!(rendered.contains("  └ ready for governed execution"), "{rendered}");
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

        assert!(rendered.contains("• Read"));
        assert!(rendered.contains("  │ first\n  │ second"), "{rendered}");
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

        assert!(rendered.contains("• Failed"), "{rendered}");
        assert!(rendered.contains("  │ workspace filesystem operation failed"), "{rendered}");
        // The corner is where the block says whether asking again could work.
        assert!(
            rendered.contains("  └ tool failed · retryable: yes"),
            "{rendered}"
        );
        assert!(!rendered.contains("fingerprint"));
    }
}
