use std::path::Path;

use orchester_protokoll::{ActionId, CallId, FeedbackReport, Observation, ObservationId, RunId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::super::executor::{ToolExecution, ToolExecutorError};
use super::super::files::{EntryKind, FileToolError};

pub(super) fn from_execution(
    run_id: &RunId,
    action_id: &ActionId,
    call_id: &CallId,
    execution: ToolExecution,
) -> Result<Observation, ()> {
    let observation_id = observation_id(run_id, action_id, call_id);
    match execution {
        ToolExecution::Read(result) => {
            let content_lines = result.content.lines().collect::<Vec<_>>();
            Ok(Observation {
                observation_id,
                call_id: call_id.clone(),
                kind: "read_file".into(),
                summary: format!("read bytes={} lines={}", result.bytes, result.lines),
                data: json!({
                    "bytes": result.bytes,
                    "content_lines": content_lines,
                    "lines": result.lines,
                }),
            })
        }
        ToolExecution::Listed(result) => {
            let entries = result
                .entries
                .into_iter()
                .map(|entry| {
                    Ok(json!({
                        "kind": entry_kind(entry.kind),
                        "path": utf8_path(&entry.path)?,
                    }))
                })
                .collect::<Result<Vec<Value>, ()>>()?;
            Ok(Observation {
                observation_id,
                call_id: call_id.clone(),
                kind: "list_files".into(),
                summary: format!("listed entries={}", entries.len()),
                data: json!({"entries": entries}),
            })
        }
        ToolExecution::Searched(result) => {
            let matches = result
                .matches
                .into_iter()
                .map(|found| {
                    Ok(json!({
                        "line": found.line,
                        "path": utf8_path(&found.path)?,
                        "text": found.text,
                    }))
                })
                .collect::<Result<Vec<Value>, ()>>()?;
            Ok(Observation {
                observation_id,
                call_id: call_id.clone(),
                kind: "search_text".into(),
                summary: format!(
                    "search matches={} skipped_oversized_files={}",
                    matches.len(),
                    result.skipped_oversized_files
                ),
                data: json!({
                    "matches": matches,
                    "skipped_oversized_files": result.skipped_oversized_files,
                }),
            })
        }
    }
}

pub(super) fn tool_failure(error: ToolExecutorError) -> FeedbackReport {
    let retryable = matches!(error, ToolExecutorError::File(FileToolError::Io));
    feedback(error.to_string(), retryable)
}

pub(super) fn output_failure() -> FeedbackReport {
    feedback("tool output could not be represented safely".into(), false)
}

fn feedback(summary: String, retryable: bool) -> FeedbackReport {
    FeedbackReport {
        source: "tool_executor".into(),
        validator_id: None,
        exit_code: None,
        classification: "tool_failed".into(),
        summary,
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        fingerprint: "pending".into(),
        retryable,
    }
}

fn observation_id(run_id: &RunId, action_id: &ActionId, call_id: &CallId) -> ObservationId {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-tool-observation-v1\0");
    for value in [&run_id.0, &action_id.0, &call_id.0] {
        hasher.update((value.len() as u64).to_be_bytes());
        hasher.update(value.as_bytes());
    }
    ObservationId::from(format!("observation-{}", hex(&hasher.finalize())))
}

fn utf8_path(path: &Path) -> Result<&str, ()> {
    path.to_str().ok_or(())
}

fn entry_kind(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => "file",
        EntryKind::Directory => "directory",
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
