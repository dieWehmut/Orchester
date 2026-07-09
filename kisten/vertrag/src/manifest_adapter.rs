use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;

use orchester_protokoll::{Capability, ChangeKind, Event, TaskKind, TodoItem, ToolStatus, Usage};

use crate::adapter::{AdapterAvailability, AgentAdapter, EventStream};
use crate::error::AdapterError;
use crate::extract;
use crate::manifest::{AdapterManifest, EventMapping};

/// The generic engine: interprets any [`AdapterManifest`] as a working adapter.
pub struct ManifestAdapter {
    manifest: AdapterManifest,
}

impl ManifestAdapter {
    /// Wrap a parsed manifest.
    pub fn new(manifest: AdapterManifest) -> Self {
        Self { manifest }
    }

    /// Parse a manifest from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, AdapterError> {
        let manifest: AdapterManifest = toml::from_str(toml_str)?;
        Ok(Self::new(manifest))
    }

    /// Build the argv (excluding the program) for a task, substituting placeholders.
    fn build_args(&self, prompt: &str, model: Option<&str>, resume: Option<&str>) -> Vec<String> {
        let template = match (resume, &self.manifest.resume_args) {
            (Some(_), Some(resume_args)) => resume_args,
            _ => &self.manifest.args,
        };
        template
            .iter()
            .map(|arg| {
                arg.replace("{prompt}", prompt)
                    .replace("{model}", model.unwrap_or(""))
                    .replace("{session_id}", resume.unwrap_or(""))
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn build_args_for_test(
        &self,
        prompt: &str,
        model: Option<&str>,
        resume: Option<&str>,
    ) -> Vec<String> {
        self.build_args(prompt, model, resume)
    }
}

#[async_trait]
impl AgentAdapter for ManifestAdapter {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn capabilities(&self) -> Capability {
        Capability {
            name: self.manifest.name.clone(),
            kinds: self.manifest.kinds.iter().map(|k| parse_kind(k)).collect(),
            supports_resume: self.manifest.supports_resume,
            streaming: self.manifest.streaming,
        }
    }

    fn availability(&self) -> AdapterAvailability {
        match find_executable(&self.manifest.command) {
            Some(path) => {
                AdapterAvailability::available(self.name(), format!("found {}", path.display()))
            }
            None => AdapterAvailability::missing(
                self.name(),
                format!("command '{}' not found on PATH", self.manifest.command),
            ),
        }
    }

    async fn run(&self, task: orchester_protokoll::Task) -> Result<EventStream, AdapterError> {
        let args = self.build_args(&task.prompt, task.model.as_deref(), task.resume.as_deref());
        let mut cmd = Command::new(&self.manifest.command);
        cmd.args(&args)
            .current_dir(&task.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            // Kill the child if the returned stream (and thus the reaper task) is
            // dropped before EOF — this is how cancellation propagates to the agent.
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|source| AdapterError::Spawn {
            command: self.manifest.command.clone(),
            source,
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::Manifest("child stdout unavailable".into()))?;

        let manifest = self.manifest.clone();
        let lines = LinesStream::new(BufReader::new(stdout).lines());

        // Per-run mutable state: emit SessionStarted only once.
        let mut session_seen = false;

        let stream = lines.flat_map(move |line_res| {
            let events = match line_res {
                Ok(line) => map_line(&manifest, &line, &mut session_seen),
                Err(e) => vec![Err(AdapterError::Io(e))],
            };
            futures::stream::iter(events)
        });

        // Keep the child alive for the duration of the stream by moving it into a
        // reaper task; on stream drop the child is killed via its kill-on-drop-less
        // handle, so we explicitly wait here in a detached task.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Box::pin(stream))
    }
}

/// Map a `kinds` string to a [`TaskKind`].
fn parse_kind(k: &str) -> TaskKind {
    match k {
        "code" => TaskKind::Code,
        "review" => TaskKind::Review,
        "chat" => TaskKind::Chat,
        "browser" => TaskKind::Browser,
        other => TaskKind::Custom(other.to_string()),
    }
}

fn find_executable(command: &str) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 || command_path.is_absolute() {
        return command_path.is_file().then(|| command_path.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    let names = executable_names(command);
    for dir in env::split_paths(&path) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_names(command: &str) -> Vec<OsString> {
    if Path::new(command).extension().is_some() {
        return vec![OsString::from(command)];
    }

    let mut names = vec![OsString::from(command)];
    if let Some(pathext) = env::var_os("PATHEXT") {
        for ext in env::split_paths(&pathext) {
            if let Some(ext) = ext.to_str() {
                names.push(OsString::from(format!("{command}{ext}")));
            }
        }
    }
    names
}

#[cfg(not(windows))]
fn executable_names(command: &str) -> Vec<OsString> {
    vec![OsString::from(command)]
}

/// Turn a single stdout line into zero or more normalized events.
///
/// Pure and side-effect-free apart from flipping `session_seen`, so it is unit-tested
/// directly against captured vendor JSON fixtures.
pub(crate) fn map_line(
    manifest: &AdapterManifest,
    line: &str,
    session_seen: &mut bool,
) -> Vec<Result<Event, AdapterError>> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        // Non-JSON lines (banners, logs) are ignored rather than fatal.
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();

    // 1. Session id: emit SessionStarted the first time we ever see it.
    if !*session_seen {
        if let Some(path) = &manifest.parse.session_id {
            if let Some(id) = extract::get_string(&value, path) {
                if !id.is_empty() {
                    *session_seen = true;
                    out.push(Ok(Event::SessionStarted { session_id: id }));
                }
            }
        }
    }

    // 2. Discriminator lookup (with optional nested sub-discriminator).
    let Some(disc) = extract::get_string(&value, &manifest.parse.discriminator) else {
        return out;
    };
    let mapping = manifest
        .parse
        .sub_discriminator
        .as_ref()
        .and_then(|sub| extract::get_string(&value, sub))
        .and_then(|sub_val| manifest.parse.map.get(&format!("{disc}/{sub_val}")))
        .or_else(|| manifest.parse.map.get(&disc));

    let Some(mapping) = mapping else {
        // Unknown type: ignored (debug-level in a real build).
        return out;
    };

    if let Some(event) = build_event(mapping, &value) {
        out.push(Ok(event));
    }
    out
}

/// Construct one [`Event`] from a matched [`EventMapping`] and the line's JSON.
fn build_event(m: &EventMapping, v: &Value) -> Option<Event> {
    let text = |field: &Option<String>| -> String {
        field
            .as_ref()
            .and_then(|p| extract::resolve_field(v, p))
            .unwrap_or_default()
    };

    match m.event.as_str() {
        "ignore" => None,
        "session_started" => None, // handled by the session_id path
        "turn_started" => Some(Event::TurnStarted),
        "turn_completed" => Some(Event::TurnCompleted),
        "message" => Some(Event::Message {
            text: text(&m.text),
        }),
        "reasoning" => Some(Event::Reasoning {
            text: text(&m.text),
        }),
        "result" => Some(Event::Result {
            text: text(&m.text),
        }),
        "error" => Some(Event::Error {
            message: text(&m.message),
        }),
        "tool_call" => Some(Event::ToolCall {
            name: m
                .name
                .as_ref()
                .and_then(|p| extract::resolve_field(v, p))
                .unwrap_or_else(|| "tool".to_string()),
            status: parse_tool_status(
                m.status
                    .as_ref()
                    .and_then(|p| extract::resolve_field(v, p))
                    .as_deref(),
            ),
            detail: m.detail.as_ref().and_then(|p| extract::resolve_field(v, p)),
        }),
        "file_change" => build_file_change(m, v),
        "usage" => Some(Event::Usage(Usage {
            input_tokens: m
                .input_tokens
                .as_ref()
                .map(|p| extract::get_u64(v, p))
                .unwrap_or(0),
            output_tokens: m
                .output_tokens
                .as_ref()
                .map(|p| extract::get_u64(v, p))
                .unwrap_or(0),
            cached_input_tokens: m
                .cached_input_tokens
                .as_ref()
                .map(|p| extract::get_u64(v, p))
                .unwrap_or(0),
            reasoning_output_tokens: m
                .reasoning_output_tokens
                .as_ref()
                .map(|p| extract::get_u64(v, p))
                .unwrap_or(0),
        })),
        "todo_list" => build_todo_list(m, v),
        _ => None,
    }
}

/// FileChange mapping emits the *first* change; multi-file lines are represented by
/// the first element (the runtime already receives a stream, and most agents emit one
/// patch item per line). `each` addresses the array; `path`/`kind` are relative to it.
fn build_file_change(m: &EventMapping, v: &Value) -> Option<Event> {
    if let Some(each_path) = &m.each {
        let arr = extract::get(v, each_path)?.as_array()?;
        let first = arr.first()?;
        let path = m
            .path
            .as_ref()
            .and_then(|p| extract::resolve_field(first, p))?;
        let kind = parse_change_kind(
            m.kind
                .as_ref()
                .and_then(|p| extract::resolve_field(first, p))
                .as_deref(),
        );
        Some(Event::FileChange { path, kind })
    } else {
        let path = m.path.as_ref().and_then(|p| extract::resolve_field(v, p))?;
        let kind = parse_change_kind(
            m.kind
                .as_ref()
                .and_then(|p| extract::resolve_field(v, p))
                .as_deref(),
        );
        Some(Event::FileChange { path, kind })
    }
}

fn build_todo_list(m: &EventMapping, v: &Value) -> Option<Event> {
    let arr = m
        .items
        .as_ref()
        .and_then(|p| extract::get(v, p))?
        .as_array()?;
    let text_field = m.item_text.as_deref().unwrap_or("text");
    let done_field = m.item_completed.as_deref().unwrap_or("completed");
    let items = arr
        .iter()
        .map(|el| TodoItem {
            text: extract::get_string(el, text_field).unwrap_or_default(),
            completed: matches!(extract::get(el, done_field), Some(Value::Bool(true))),
        })
        .collect();
    Some(Event::TodoList { items })
}

fn parse_tool_status(s: Option<&str>) -> ToolStatus {
    match s {
        Some("completed") => ToolStatus::Completed,
        Some("failed") | Some("error") => ToolStatus::Failed,
        _ => ToolStatus::InProgress,
    }
}

fn parse_change_kind(s: Option<&str>) -> ChangeKind {
    match s {
        Some("add") => ChangeKind::Add,
        Some("delete") => ChangeKind::Delete,
        _ => ChangeKind::Update,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal claude-shaped manifest exercising session_id, discriminator,
    /// and a couple of event mappings.
    const CLAUDE_TOML: &str = r#"
name = "claude"
command = "claude"
args = ["-p", "{prompt}", "--output-format", "stream-json", "--verbose"]
resume_args = ["-p", "{prompt}", "--resume", "{session_id}", "--output-format", "stream-json", "--verbose"]
kinds = ["code", "chat"]
supports_resume = true

[parse]
discriminator = "type"
session_id = "session_id"

[parse.map]
assistant = { event = "message", text = "message.content[0].text" }
result    = { event = "result",  text = "result" }
"#;

    fn adapter() -> ManifestAdapter {
        ManifestAdapter::from_toml(CLAUDE_TOML).expect("valid manifest")
    }

    fn adapter_with_command(command: String) -> ManifestAdapter {
        let mut manifest = adapter().manifest.clone();
        manifest.command = command;
        ManifestAdapter::new(manifest)
    }

    #[test]
    fn build_args_substitutes_prompt() {
        let a = adapter();
        let args = a.build_args_for_test("list files", None, None);
        assert_eq!(
            args,
            vec![
                "-p",
                "list files",
                "--output-format",
                "stream-json",
                "--verbose"
            ]
        );
    }

    #[test]
    fn build_args_uses_resume_template_when_resuming() {
        let a = adapter();
        let args = a.build_args_for_test("more", None, Some("sess-123"));
        assert_eq!(
            args,
            vec![
                "-p",
                "more",
                "--resume",
                "sess-123",
                "--output-format",
                "stream-json",
                "--verbose"
            ]
        );
    }

    #[test]
    fn availability_reports_existing_command() {
        let exe = std::env::current_exe().expect("current test executable");
        let a = adapter_with_command(exe.to_string_lossy().into_owned());
        let availability = a.availability();

        assert!(!availability.is_missing());
        assert_eq!(availability.name, "claude");
        assert!(availability.detail.contains("found"));
    }

    #[test]
    fn availability_reports_missing_command() {
        let a = adapter_with_command("orchester-command-that-should-not-exist".into());
        let availability = a.availability();

        assert!(availability.is_missing());
        assert_eq!(availability.name, "claude");
        assert!(availability.detail.contains("not found"));
    }

    #[test]
    fn session_id_emitted_once() {
        let a = adapter();
        let m = &a.manifest;
        let mut seen = false;

        let first = map_line(m, r#"{"type":"system","session_id":"abc"}"#, &mut seen);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            &first[0],
            Ok(Event::SessionStarted { session_id }) if session_id == "abc"
        ));
        assert!(seen);

        // A second line carrying the same id must NOT re-emit SessionStarted.
        let second = map_line(m, r#"{"type":"system","session_id":"abc"}"#, &mut seen);
        assert!(second.is_empty());
    }

    #[test]
    fn assistant_line_maps_to_message() {
        let a = adapter();
        let mut seen = true; // suppress session handling for this line
        let out = map_line(
            &a.manifest,
            r#"{"type":"assistant","message":{"content":[{"text":"hello"}]}}"#,
            &mut seen,
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], Ok(Event::Message { text }) if text == "hello"));
    }

    #[test]
    fn result_line_maps_to_result() {
        let a = adapter();
        let mut seen = true;
        let out = map_line(
            &a.manifest,
            r#"{"type":"result","result":"done"}"#,
            &mut seen,
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], Ok(Event::Result { text }) if text == "done"));
    }

    #[test]
    fn unknown_type_and_non_json_are_ignored() {
        let a = adapter();
        let mut seen = true;
        assert!(map_line(&a.manifest, r#"{"type":"mystery"}"#, &mut seen).is_empty());
        assert!(map_line(&a.manifest, "not json at all", &mut seen).is_empty());
        assert!(map_line(&a.manifest, "", &mut seen).is_empty());
    }
}
