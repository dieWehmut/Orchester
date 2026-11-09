use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};

use orchester_protokoll::{Capability, ChangeKind, Event, TaskKind, TodoItem, ToolStatus, Usage};

use crate::adapter::{AdapterAvailability, AgentAdapter, EventStream};
use crate::error::AdapterError;
use crate::extract;
use crate::manifest::{AdapterManifest, EventMapping};
use crate::process::{command_invocation, resolve_command, ProcessTree};

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
            .map(|arg| expand_argument(arg, prompt, model.unwrap_or(""), resume.unwrap_or("")))
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

/// Expand only placeholders in the manifest, never text introduced by a value.
/// Prompts commonly contain code such as `{model}` which must reach the agent intact.
fn expand_argument(template: &str, prompt: &str, model: &str, session_id: &str) -> String {
    let mut result = String::new();
    let mut remaining = template;
    while let Some(index) = remaining.find('{') {
        result.push_str(&remaining[..index]);
        remaining = &remaining[index..];
        if let Some((placeholder, value)) = [
            ("{prompt}", prompt),
            ("{model}", model),
            ("{session_id}", session_id),
        ]
        .into_iter()
        .find(|(placeholder, _)| remaining.starts_with(placeholder))
        {
            result.push_str(value);
            remaining = &remaining[placeholder.len()..];
        } else {
            result.push('{');
            remaining = &remaining[1..];
        }
    }
    result.push_str(remaining);
    result
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
        match resolve_command(&self.manifest.command) {
            Some(path) => {
                AdapterAvailability::available(self.name(), format!("found {}", path.display()))
            }
            None => AdapterAvailability::missing(
                self.name(),
                format!("command '{}' not found on PATH", self.manifest.command),
            ),
        }
    }

    fn native_command(&self) -> Option<&str> {
        Some(&self.manifest.command)
    }

    async fn run(&self, task: orchester_protokoll::Task) -> Result<EventStream, AdapterError> {
        let args = self.build_args(&task.prompt, task.model.as_deref(), task.resume.as_deref());
        let program = resolve_command(&self.manifest.command)
            .unwrap_or_else(|| PathBuf::from(&self.manifest.command));
        let invocation =
            command_invocation(&program, args.into_iter().map(OsString::from).collect());
        let tree = ProcessTree::new()?;
        let mut cmd = Command::new(invocation.program);
        cmd.args(invocation.args)
            .envs(invocation.envs)
            .current_dir(&task.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            // Kill the child if the returned stream (and thus the reaper task) is
            // dropped before EOF — this is how cancellation propagates to the agent.
            .kill_on_drop(true);

        tree.configure_command(&mut cmd);

        let mut child = cmd.spawn().map_err(|source| AdapterError::Spawn {
            command: self.manifest.command.clone(),
            source,
        })?;
        if let Err(error) = tree.attach(&child) {
            tree.terminate(&mut child);
            return Err(error.into());
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::Manifest("child stdout unavailable".into()))?;

        let manifest = self.manifest.clone();
        let lines = futures::stream::try_unfold(BufReader::new(stdout), |mut reader| async move {
            let line = read_native_line(&mut reader).await?;
            Ok::<_, AdapterError>(line.map(|line| (line, reader)))
        });

        // Per-run mutable state: emit SessionStarted only once.
        let mut session_seen = false;

        let stream = lines.flat_map(move |line_res| {
            let events = match line_res {
                Ok(line) => map_line(&manifest, &line, &mut session_seen),
                Err(e) => vec![Err(e)],
            };
            futures::stream::iter(events)
        });

        // Drain diagnostics without retaining or exposing potentially sensitive stderr.
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AdapterError::Manifest("child stderr unavailable".into()))?;
        let drain = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
        });
        let owner = ProcessOwner { tree, child, drain };
        // The returned stream owns the process. Dropping it cancels execution,
        // including when stdout reached EOF but the process is still running.
        let mapped: EventStream = Box::pin(stream);
        let stream = futures::stream::unfold(Some((mapped, owner)), |state| async move {
            let (mut mapped, mut owner) = state?;
            if let Some(event) = mapped.next().await {
                return Some((event, Some((mapped, owner))));
            }
            let status = owner.child.wait().await;
            match status {
                Ok(status) if status.success() => None,
                Ok(status) => Some((
                    Err(AdapterError::Io(std::io::Error::other(format!(
                        "agent exited with {status}"
                    )))),
                    None,
                )),
                Err(error) => Some((Err(AdapterError::Io(error)), None)),
            }
        });
        Ok(Box::pin(stream))
    }
}

// Bound buffering before JSON parsing, including a process that never emits a
// newline. Normalized per-event and total-output limits still apply downstream.
const MAX_NATIVE_LINE_BYTES: usize = 16 * 1024 * 1024;

async fn read_native_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<Option<String>, AdapterError> {
    let mut bytes = Vec::new();
    let count = reader
        .take((MAX_NATIVE_LINE_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .await?;
    if count > MAX_NATIVE_LINE_BYTES {
        return Err(AdapterError::OutputLimit);
    }
    if count == 0 {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error).into())
}

/// Child and stderr task share exactly the event stream's lifetime.
struct ProcessOwner {
    child: Child,
    drain: tokio::task::JoinHandle<()>,
    tree: ProcessTree,
}

impl Drop for ProcessOwner {
    fn drop(&mut self) {
        self.tree.terminate(&mut self.child);
        self.drain.abort();
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

    if mapping.event == "file_change" && mapping.each.is_some() {
        if let Some(changes) = mapping
            .each
            .as_ref()
            .and_then(|path| extract::get(&value, path))
            .and_then(Value::as_array)
        {
            out.extend(
                changes
                    .iter()
                    .filter_map(|change| build_file_change(mapping, change))
                    .map(Ok),
            );
        }
    } else if let Some(event) = build_event(mapping, &value) {
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

/// Build a single change. `map_line` applies this to every `each` array element.
fn build_file_change(m: &EventMapping, v: &Value) -> Option<Event> {
    let path = m.path.as_ref().and_then(|p| extract::resolve_field(v, p))?;
    let kind = parse_change_kind(
        m.kind
            .as_ref()
            .and_then(|p| extract::resolve_field(v, p))
            .as_deref(),
    );
    Some(Event::FileChange { path, kind })
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

    #[test]
    fn codex_reports_every_file_in_a_completed_patch() {
        let a = ManifestAdapter::from_toml(include_str!("../../../manifeste/codex.toml"))
            .expect("valid built-in manifest");
        let mut seen = true;
        let events = map_line(
            &a.manifest,
            r#"{"type":"item.completed","item":{"type":"file_change","changes":[{"path":"new.txt","kind":"add"},{"path":"edited.txt","kind":"update"},{"path":"old.txt","kind":"delete"}]}}"#,
            &mut seen,
        );
        let changes = events
            .into_iter()
            .map(|event| match event.unwrap() {
                Event::FileChange { path, kind } => (path, kind),
                event => panic!("unexpected event: {event:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            changes,
            vec![
                ("new.txt".into(), ChangeKind::Add),
                ("edited.txt".into(), ChangeKind::Update),
                ("old.txt".into(), ChangeKind::Delete),
            ]
        );
    }
}
