use super::{ActivityDetail, ActivityTurn, clip, empty_detail, finish, opaque_id, push_entry};
use serde_json::Value;
use std::collections::HashSet;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub(super) fn text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .filter(|part| {
                matches!(
                    part["type"].as_str(),
                    Some("text" | "input_text" | "output_text")
                )
            })
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
    }
    String::new()
}

pub(super) fn arguments(value: &Value) -> String {
    if value.is_null() {
        String::new()
    } else if let Some(text) = value.as_str() {
        text.into()
    } else {
        serde_json::to_string_pretty(value).unwrap_or_default()
    }
}

pub(super) fn project(value: &str) -> String {
    clip(
        value
            .trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default(),
        120,
    )
}

pub(super) fn parse(
    provider: &str,
    source_id: &str,
    updated_at: i64,
    records: &[(u64, Value)],
) -> ActivityDetail {
    let mut row = empty_detail(provider, source_id, updated_at);
    let mut current_turn: Option<String> = None;
    for (offset, record) in records {
        parse_record(&mut row, &mut current_turn, provider, *offset, record);
    }
    finish(&mut row);
    row
}

fn parse_record(
    row: &mut ActivityDetail,
    current_turn: &mut Option<String>,
    provider: &str,
    offset: u64,
    record: &Value,
) {
    let entry_ids = record_entry_ids(provider, &row.summary.id, offset, record);
    let timestamp = record["timestamp"]
        .as_str()
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        .map(|value| (value.unix_timestamp_nanos() / 1_000_000) as i64);
    if provider == "codex" {
        codex_turn(row, current_turn, offset, record, timestamp);
        codex(row, offset, record);
    } else {
        claude(row, offset, record);
    }
    let phase = if provider == "codex" {
        match record["payload"]["phase"].as_str() {
            Some("commentary") => Some("commentary"),
            Some("final" | "final_answer") => Some("final"),
            _ => None,
        }
    } else {
        match record["message"]["stop_reason"].as_str() {
            Some("end_turn" | "stop_sequence") => Some("final"),
            Some("tool_use") => Some("commentary"),
            _ => None,
        }
    };
    for entry in &mut row.entries {
        if entry_ids.contains(&entry.id) {
            entry.timestamp = timestamp;
            entry.phase = (entry.kind == "assistant").then_some(phase).flatten();
            entry.turn_id = current_turn.clone();
            let call = if provider == "codex" {
                record["payload"]["call_id"].as_str()
            } else {
                None
            };
            if let Some(call) = call {
                entry.call_id = Some(opaque_id(&row.summary.id, call));
            }
        }
    }
}

/// Stream the source once while retaining only a page and its turn context.
/// Offsets refer to decoded JSONL, so compression never changes entry identities.
pub(super) struct CodexHistory {
    row: ActivityDetail,
    current_turn: Option<String>,
    positions: std::collections::HashMap<String, u64>,
    before: Option<u64>,
    spilled: bool,
}
impl CodexHistory {
    pub fn new(source: &str, updated: i64, before: Option<u64>) -> Self {
        Self {
            row: empty_detail("codex", source, updated),
            current_turn: None,
            positions: Default::default(),
            before,
            spilled: false,
        }
    }
    pub fn record(&mut self, offset: u64, record: &Value) {
        if self.before.is_some_and(|before| offset >= before) {
            // Finish source turn timings even when their ending is on a newer page.
            let timestamp = record["timestamp"]
                .as_str()
                .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
                .map(|value| (value.unix_timestamp_nanos() / 1_000_000) as i64);
            codex_turn(
                &mut self.row,
                &mut self.current_turn,
                offset,
                record,
                timestamp,
            );
            if record["type"] != "response_item" {
                codex(&mut self.row, offset, record);
            }
        } else {
            let first = self.row.entries.first().map(|entry| entry.id.clone());
            parse_record(
                &mut self.row,
                &mut self.current_turn,
                "codex",
                offset,
                record,
            );
            if first.is_some_and(|id| !self.row.entries.iter().any(|entry| entry.id == id)) {
                self.spilled = true;
            }
            for id in record_entry_ids("codex", &self.row.summary.id, offset, record) {
                if self.row.entries.iter().any(|entry| entry.id == id) {
                    self.positions.insert(id, offset);
                }
            }
            self.positions
                .retain(|id, _| self.row.entries.iter().any(|entry| &entry.id == id));
        }
        self.row.turns.retain(|turn| {
            self.current_turn.as_ref() == Some(&turn.id)
                || self
                    .row
                    .entries
                    .iter()
                    .any(|entry| entry.turn_id.as_ref() == Some(&turn.id))
        });
    }
    pub fn finish(mut self) -> ActivityDetail {
        if self.spilled {
            self.row.next_before = self
                .row
                .entries
                .first()
                .and_then(|entry| self.positions.get(&entry.id).copied());
        }
        self.row.summary.truncated = self.row.content_limited;
        finish(&mut self.row);
        self.row
    }
}

fn record_entry_ids(
    provider: &str,
    session_id: &str,
    offset: u64,
    record: &Value,
) -> HashSet<String> {
    if provider == "codex" {
        return if record["type"] == "response_item" {
            HashSet::from([opaque_id(session_id, &format!("line-{offset}"))])
        } else {
            HashSet::new()
        };
    }
    if !matches!(record["type"].as_str(), Some("user" | "assistant"))
        || record["isMeta"].as_bool() == Some(true)
    {
        return HashSet::new();
    }
    let fallback = format!("line-{offset}");
    let message_id = record["uuid"].as_str().unwrap_or(&fallback);
    if let Some(parts) = record["message"]["content"].as_array() {
        (0..parts.len())
            .map(|index| opaque_id(session_id, &format!("{message_id}-{index}")))
            .collect()
    } else {
        HashSet::from([opaque_id(session_id, message_id)])
    }
}

fn codex_turn(
    row: &mut ActivityDetail,
    current: &mut Option<String>,
    offset: u64,
    record: &Value,
    timestamp: Option<i64>,
) {
    let payload = &record["payload"];
    let event = payload["type"].as_str();
    let started =
        record["type"] == "event_msg" && matches!(event, Some("task_started" | "turn_started"));
    let completed = record["type"] == "event_msg"
        && matches!(
            event,
            Some(
                "task_complete"
                    | "task_completed"
                    | "turn_complete"
                    | "turn_aborted"
                    | "task_interrupted"
            )
        );
    if started || record["type"] == "turn_context" {
        let fallback = format!("turn-{offset}");
        let source = payload["turn_id"]
            .as_str()
            .or(if started { Some(&fallback) } else { None });
        if let Some(source) = source {
            let id = opaque_id(&row.summary.id, source);
            if let Some(turn) = row.turns.iter_mut().find(|turn| turn.id == id) {
                if started {
                    turn.started_at = timestamp;
                }
            } else {
                row.turns.push(ActivityTurn {
                    id: id.clone(),
                    started_at: if started { timestamp } else { None },
                    completed_at: None,
                });
            }
            *current = Some(id);
        }
    }
    if completed {
        let target = payload["turn_id"]
            .as_str()
            .map(|id| opaque_id(&row.summary.id, id))
            .or_else(|| current.clone());
        if let Some(turn) = row
            .turns
            .iter_mut()
            .find(|turn| Some(&turn.id) == target.as_ref())
        {
            // Invalid or reversed source times must not produce a fabricated duration.
            turn.completed_at =
                timestamp.filter(|end| turn.started_at.map_or(true, |start| *end >= start));
        }
        *current = None;
    }
}

fn title(row: &mut ActivityDetail, value: &str) {
    let value = value.trim();
    if row.summary.title.is_empty()
        && !value.is_empty()
        && !value.starts_with('<')
        && !value.starts_with("# AGENTS.md")
        && !value.starts_with("# Environment")
    {
        row.summary.title = clip(value.lines().next().unwrap_or(value), 120);
    }
}

fn codex(row: &mut ActivityDetail, offset: u64, record: &Value) {
    let payload = &record["payload"];
    let id = format!("line-{offset}");
    match record["type"].as_str() {
        Some("session_meta" | "turn_context") => {
            if let Some(cwd) = payload["cwd"].as_str() {
                row.summary.project = project(cwd);
                row.summary.project_id = Some(opaque_id("codex-project", cwd));
            }
        }
        Some("event_msg") => match payload["type"].as_str() {
            Some("task_started" | "turn_started") => row.summary.state = "active",
            Some("task_complete" | "task_completed" | "turn_complete") => {
                row.summary.state = "completed"
            }
            Some("turn_aborted" | "task_interrupted") => row.summary.state = "interrupted",
            // response_item is the single content authority: event_msg often
            // repeats the same user/assistant text and is lifecycle-only here.
            _ => (),
        },
        Some("response_item") => match payload["type"].as_str() {
            Some("message") => {
                let kind = match payload["role"].as_str() {
                    Some("user") => "user",
                    Some("assistant") => "assistant",
                    _ => return,
                };
                let content = text(&payload["content"]);
                if kind == "user" {
                    if content.trim_start().starts_with('<') || content.starts_with("# AGENTS.md") {
                        return;
                    }
                    title(row, &content);
                    row.summary.state = "active";
                } else if matches!(payload["phase"].as_str(), Some("final" | "final_answer")) {
                    row.summary.state = "completed";
                }
                push_entry(row, &id, kind, &content, None);
            }
            Some("function_call" | "custom_tool_call") => {
                row.summary.state = "active";
                let args = if payload["arguments"].is_null() {
                    &payload["input"]
                } else {
                    &payload["arguments"]
                };
                push_entry(row, &id, "tool", &arguments(args), payload["name"].as_str());
            }
            Some("function_call_output" | "custom_tool_call_output") => {
                let output = &payload["output"];
                // Structured multimodal payloads contribute only text blocks.
                let content = output
                    .as_str()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .filter(Value::is_array)
                    .map(|value| text(&value))
                    .unwrap_or_else(|| text(output));
                push_entry(row, &id, "result", &content, None);
            }
            _ => (),
        },
        _ => (),
    }
}

fn claude(row: &mut ActivityDetail, offset: u64, record: &Value) {
    if let Some(cwd) = record["cwd"].as_str() {
        row.summary.project = project(cwd);
        row.summary.project_id = Some(opaque_id("claude-project", cwd));
    }
    match record["type"].as_str() {
        Some("last-prompt") => {
            if let Some(prompt) = record["lastPrompt"].as_str() {
                title(row, prompt);
            }
        }
        Some("system") => match record["subtype"].as_str() {
            Some("turn_duration") => row.summary.state = "completed",
            Some("api_error") => {
                row.summary.state = "interrupted";
            }
            _ => (),
        },
        Some("user" | "assistant") => {
            if record["isMeta"].as_bool() == Some(true) {
                return;
            }
            let kind = if record["type"] == "user" {
                "user"
            } else {
                "assistant"
            };
            let content = &record["message"]["content"];
            let message_id = record["uuid"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("line-{offset}"));
            if let Some(content) = content.as_str() {
                push_entry(row, &message_id, kind, content, None);
                if kind == "user" {
                    title(row, content);
                    row.summary.state = "active";
                }
            } else if let Some(parts) = content.as_array() {
                for (index, part) in parts.iter().enumerate() {
                    let id = format!("{message_id}-{index}");
                    match part["type"].as_str() {
                        Some("text") => {
                            let content = part["text"].as_str().unwrap_or_default();
                            push_entry(row, &id, kind, content, None);
                            if kind == "user" {
                                title(row, content);
                                row.summary.state = "active";
                            }
                        }
                        Some("tool_use") => {
                            row.summary.state = "active";
                            push_entry(
                                row,
                                &id,
                                "tool",
                                &arguments(&part["input"]),
                                part["name"].as_str(),
                            );
                            let entry_id = opaque_id(&row.summary.id, &id);
                            if let (Some(entry), Some(call)) = (
                                row.entries.iter_mut().find(|entry| entry.id == entry_id),
                                part["id"].as_str(),
                            ) {
                                entry.call_id = Some(opaque_id(&row.summary.id, call));
                            }
                        }
                        Some("tool_result") => {
                            push_entry(row, &id, "result", &text(&part["content"]), None);
                            let entry_id = opaque_id(&row.summary.id, &id);
                            if let (Some(entry), Some(call)) = (
                                row.entries.iter_mut().find(|entry| entry.id == entry_id),
                                part["tool_use_id"].as_str(),
                            ) {
                                entry.call_id = Some(opaque_id(&row.summary.id, call));
                            }
                        }
                        _ => (),
                    }
                }
            }
            if kind == "assistant"
                && matches!(
                    record["message"]["stop_reason"].as_str(),
                    Some("end_turn" | "stop_sequence")
                )
            {
                row.summary.state = "completed";
            }
            if record["isApiErrorMessage"].as_bool() == Some(true) {
                row.summary.state = "interrupted";
            }
        }
        _ => (),
    }
}
