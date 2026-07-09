//! Terminal rendering of the Orchester event stream.
//!
//! Two output modes share this module:
//! * **rendered** (default) — each [`Event`] becomes a human-friendly line, with
//!   light ANSI styling (no extra crates; just raw escape codes).
//! * **json** — each event is printed as one line of Orchester's own Event JSONL,
//!   bypassing the renderer entirely so Orchester can itself be orchestrated.

use std::io::Write;

use orchester_laufzeit::SessionRecord;
use orchester_protokoll::{Capability, ChangeKind, Event, TaskKind, ToolStatus};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};

// Minimal ANSI palette. Kept here rather than pulling a styling crate for v0.1.
const DIM: &str = "\x1b[2m";
const ITALIC: &str = "\x1b[3m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

/// Render a single event to `out` in human-readable form.
pub fn render_event(out: &mut impl Write, event: &Event) -> std::io::Result<()> {
    match event {
        Event::SessionStarted { session_id } => {
            writeln!(out, "{DIM}● session {session_id}{RESET}")
        }
        Event::TurnStarted => Ok(()),
        Event::TurnCompleted => Ok(()),
        Event::Reasoning { text } => writeln!(out, "{DIM}{ITALIC}{text}{RESET}"),
        Event::Message { text } => writeln!(out, "{text}"),
        Event::Result { text } => writeln!(out, "{text}"),
        Event::ToolCall {
            name,
            status,
            detail,
        } => {
            let marker = match status {
                ToolStatus::InProgress => "▶",
                ToolStatus::Completed => "✔",
                ToolStatus::Failed => "✖",
            };
            match detail {
                Some(d) if !d.is_empty() => {
                    writeln!(
                        out,
                        "{DIM}{marker} {name} ({}) — {d}{RESET}",
                        status_word(*status)
                    )
                }
                _ => writeln!(
                    out,
                    "{DIM}{marker} {name} ({}){RESET}",
                    status_word(*status)
                ),
            }
        }
        Event::FileChange { path, kind } => {
            let (sign, word) = match kind {
                ChangeKind::Add => ("+", "add"),
                ChangeKind::Update => ("±", "update"),
                ChangeKind::Delete => ("-", "delete"),
            };
            writeln!(out, "{DIM}{sign} {path} ({word}){RESET}")
        }
        Event::TodoList { items } => {
            for item in items {
                let box_ = if item.completed { "[x]" } else { "[ ]" };
                writeln!(out, "{DIM}{box_} {}{RESET}", item.text)?;
            }
            Ok(())
        }
        Event::Usage(u) => writeln!(
            out,
            "{DIM}tokens: in {} · out {} · cached {}{RESET}",
            u.input_tokens, u.output_tokens, u.cached_input_tokens
        ),
        Event::Error { message } => writeln!(out, "{RED}error: {message}{RESET}"),
    }
}

/// Print each event as one JSON line (Orchester's own protocol on the wire).
pub fn render_event_json(out: &mut impl Write, event: &Event) -> std::io::Result<()> {
    // Event serializes infallibly (all fields are plain data), so unwrap is safe.
    let line = serde_json::to_string(event).expect("Event serializes");
    writeln!(out, "{line}")
}

/// Render the capability table for `orchester list`.
pub fn render_list(out: &mut impl Write, caps: &[Capability]) -> std::io::Result<()> {
    if caps.is_empty() {
        return writeln!(out, "{DIM}no adapters registered{RESET}");
    }
    for cap in caps {
        let kinds = cap
            .kinds
            .iter()
            .map(kind_word)
            .collect::<Vec<_>>()
            .join(", ");
        let resume = if cap.supports_resume {
            format!("{GREEN}resume{RESET}")
        } else {
            format!("{DIM}no-resume{RESET}")
        };
        writeln!(out, "{GREEN}{}{RESET}\t[{kinds}]\t{resume}", cap.name)?;
    }
    Ok(())
}

/// Render capabilities as JSONL.
pub fn render_list_json(out: &mut impl Write, caps: &[Capability]) -> std::io::Result<()> {
    for cap in caps {
        let line = serde_json::to_string(cap).expect("Capability serializes");
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// Render adapter availability diagnostics for `orchester doctor`.
pub fn render_doctor(out: &mut impl Write, checks: &[AdapterAvailability]) -> std::io::Result<()> {
    if checks.is_empty() {
        return writeln!(out, "{DIM}no adapters registered{RESET}");
    }

    for check in checks {
        let (color, label) = match check.status {
            AvailabilityStatus::Available => (GREEN, "ok"),
            AvailabilityStatus::Missing => (RED, "missing"),
            AvailabilityStatus::Unknown => (DIM, "unknown"),
        };
        writeln!(
            out,
            "{color}{label}{RESET}\t{}\t{}",
            check.name, check.detail
        )?;
    }
    Ok(())
}

/// Render recorded sessions newest-first for humans.
pub fn render_sessions(out: &mut impl Write, records: &[SessionRecord]) -> std::io::Result<()> {
    if records.is_empty() {
        return writeln!(out, "{DIM}no sessions recorded{RESET}");
    }

    for record in records.iter().rev() {
        let session_id = record.session_id.as_deref().unwrap_or("-");
        writeln!(
            out,
            "{}\t{}\t{}\t{}",
            outcome_word(record.outcome),
            record.agent,
            session_id,
            compact_prompt(&record.prompt)
        )?;
    }
    Ok(())
}

/// Render recorded sessions as JSONL.
pub fn render_sessions_json(
    out: &mut impl Write,
    records: &[SessionRecord],
) -> std::io::Result<()> {
    for record in records {
        let line = serde_json::to_string(record).expect("SessionRecord serializes");
        writeln!(out, "{line}")?;
    }
    Ok(())
}

fn status_word(s: ToolStatus) -> &'static str {
    match s {
        ToolStatus::InProgress => "in_progress",
        ToolStatus::Completed => "completed",
        ToolStatus::Failed => "failed",
    }
}

fn kind_word(k: &TaskKind) -> String {
    match k {
        TaskKind::Code => "code".into(),
        TaskKind::Review => "review".into(),
        TaskKind::Chat => "chat".into(),
        TaskKind::Browser => "browser".into(),
        TaskKind::Custom(s) => s.clone(),
    }
}

fn outcome_word(outcome: orchester_protokoll::Outcome) -> &'static str {
    match outcome {
        orchester_protokoll::Outcome::Success => "success",
        orchester_protokoll::Outcome::Failed => "failed",
        orchester_protokoll::Outcome::Cancelled => "cancelled",
    }
}

fn compact_prompt(prompt: &str) -> String {
    let first_line = prompt.lines().next().unwrap_or("").trim();
    let mut chars = first_line.chars();
    let mut out: String = chars.by_ref().take(77).collect();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_protokoll::Usage;

    fn rendered(event: &Event) -> String {
        let mut buf = Vec::new();
        render_event(&mut buf, event).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn message_is_plain_text() {
        assert_eq!(rendered(&Event::Message { text: "hi".into() }), "hi\n");
    }

    #[test]
    fn turn_events_render_nothing() {
        assert_eq!(rendered(&Event::TurnStarted), "");
        assert_eq!(rendered(&Event::TurnCompleted), "");
    }

    #[test]
    fn error_is_marked() {
        let out = rendered(&Event::Error {
            message: "boom".into(),
        });
        assert!(out.contains("error: boom"));
    }

    #[test]
    fn json_mode_is_valid_jsonl() {
        let mut buf = Vec::new();
        render_event_json(&mut buf, &Event::Message { text: "hi".into() }).unwrap();
        let line = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["type"], "message");
        assert_eq!(value["text"], "hi");
    }

    #[test]
    fn json_roundtrips_usage() {
        let mut buf = Vec::new();
        let ev = Event::Usage(Usage {
            input_tokens: 7,
            ..Usage::default()
        });
        render_event_json(&mut buf, &ev).unwrap();
        let line = String::from_utf8(buf).unwrap();
        let back: Event = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn list_shows_names_and_kinds() {
        let caps = vec![Capability {
            name: "mock".into(),
            kinds: vec![TaskKind::Chat],
            supports_resume: false,
            streaming: true,
        }];
        let mut buf = Vec::new();
        render_list(&mut buf, &caps).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("mock"));
        assert!(out.contains("chat"));
        assert!(out.contains("no-resume"));
    }

    #[test]
    fn list_json_is_valid_jsonl() {
        let caps = vec![Capability {
            name: "mock".into(),
            kinds: vec![TaskKind::Chat],
            supports_resume: false,
            streaming: true,
        }];
        let mut buf = Vec::new();
        render_list_json(&mut buf, &caps).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(out.trim()).unwrap();

        assert_eq!(value["name"], "mock");
        assert_eq!(value["kinds"][0], "chat");
        assert_eq!(value["streaming"], true);
    }

    #[test]
    fn doctor_shows_adapter_status() {
        let checks = vec![
            AdapterAvailability::available("mock", "built-in mock adapter"),
            AdapterAvailability::missing("ghost", "command 'ghost' not found on PATH"),
        ];
        let mut buf = Vec::new();
        render_doctor(&mut buf, &checks).unwrap();
        let out = String::from_utf8(buf).unwrap();

        assert!(out.contains("ok"));
        assert!(out.contains("mock"));
        assert!(out.contains("missing"));
        assert!(out.contains("ghost"));
    }

    #[test]
    fn sessions_show_newest_first() {
        let records = vec![
            session_record("first", "sid-1"),
            session_record("second", "sid-2"),
        ];
        let mut buf = Vec::new();
        render_sessions(&mut buf, &records).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let first = out.find("second").unwrap();
        let second = out.find("first").unwrap();

        assert!(first < second);
        assert!(out.contains("success"));
        assert!(out.contains("sid-2"));
    }

    #[test]
    fn sessions_json_is_valid_jsonl() {
        let records = vec![session_record("json prompt", "sid-json")];
        let mut buf = Vec::new();
        render_sessions_json(&mut buf, &records).unwrap();
        let out = String::from_utf8(buf).unwrap();
        let value: serde_json::Value = serde_json::from_str(out.trim()).unwrap();

        assert_eq!(value["agent"], "mock");
        assert_eq!(value["session_id"], "sid-json");
        assert_eq!(value["prompt"], "json prompt");
    }

    fn session_record(prompt: &str, session_id: &str) -> SessionRecord {
        SessionRecord {
            recorded_at_unix: 1,
            agent: "mock".into(),
            session_id: Some(session_id.into()),
            prompt: prompt.into(),
            cwd: ".".into(),
            model: None,
            outcome: orchester_protokoll::Outcome::Success,
            final_text: "done".into(),
            usage: Default::default(),
        }
    }
}
