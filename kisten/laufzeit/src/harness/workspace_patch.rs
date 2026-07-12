//! Strict, capability-backed `apply_patch` execution.

use std::collections::BTreeSet;
use std::fmt;
use std::io::Read;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use orchester_modell::{MAX_CONTENT_BYTES, MAX_PATH_BYTES};
use orchester_protokoll::AgentAction;
use thiserror::Error;

use super::barrier::StartedTool;
use super::governance::{
    ContentFingerprint, GuardError, GuardErrorKind, ResolvedPath, WorkspaceGuard, WorkspaceLocks,
};

const MAX_PATCH_FILES: usize = 64;
const MAX_PATCH_HUNKS: usize = 256;
const MAX_PATCH_LINES: usize = 100_000;
const MAX_PATCH_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchLimits {
    pub max_patch_bytes: usize,
    pub max_files: usize,
    pub max_hunks: usize,
    pub max_lines: usize,
}

impl Default for PatchLimits {
    fn default() -> Self {
        Self {
            max_patch_bytes: MAX_CONTENT_BYTES,
            max_files: 32,
            max_hunks: 128,
            max_lines: 50_000,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PatchError {
    #[error("started tool is not an apply-patch action")]
    WrongAction,
    #[error("patch input is malformed")]
    Parse,
    #[error("patch exceeds a configured safety limit")]
    LimitExceeded,
    #[error("workspace path operation was rejected: {0:?}")]
    Guard(GuardErrorKind),
    #[error("patch does not match the current file contents")]
    CasMismatch,
    #[error("patch context matches more than one location")]
    AmbiguousMatch,
    #[error("patch operation is not supported")]
    UnsupportedOperation,
    #[error("workspace filesystem operation failed")]
    Io,
    #[error("patch result may have partially committed and must not be replayed")]
    UnknownOutcome,
    #[error("patch configuration is invalid")]
    InvalidInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchResult {
    pub files_changed: usize,
    pub bytes_written: u64,
}

pub struct GovernedWorkspacePatcher {
    workspace: WorkspaceGuard,
    limits: PatchLimits,
    locks: WorkspaceLocks,
}

impl fmt::Debug for GovernedWorkspacePatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GovernedWorkspacePatcher")
            .field("workspace", &self.workspace)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl GovernedWorkspacePatcher {
    pub fn new(
        root: impl AsRef<Path>,
        limits: PatchLimits,
        locks: WorkspaceLocks,
    ) -> Result<Self, PatchError> {
        if limits.max_patch_bytes == 0
            || limits.max_patch_bytes > MAX_CONTENT_BYTES
            || limits.max_files == 0
            || limits.max_files > MAX_PATCH_FILES
            || limits.max_hunks == 0
            || limits.max_hunks > MAX_PATCH_HUNKS
            || limits.max_lines == 0
            || limits.max_lines > MAX_PATCH_LINES
        {
            return Err(PatchError::InvalidInput);
        }
        let workspace = WorkspaceGuard::new(root.as_ref()).map_err(map_guard)?;
        Ok(Self {
            workspace,
            limits,
            locks,
        })
    }

    /// Validates every operation and stages every temporary file before the
    /// first rename. A later rename can still fail independently; callers
    /// must treat `UnknownOutcome` as non-replayable.
    pub async fn execute(&self, started: StartedTool) -> Result<PatchResult, PatchError> {
        let AgentAction::ApplyPatch { patch } = started.into_action() else {
            return Err(PatchError::WrongAction);
        };
        let operations = parse_patch(&patch, self.limits)?;
        let _lock = self.locks.mutate(&self.workspace).await;
        let mut plans = Vec::with_capacity(operations.len());

        for operation in operations {
            match operation.operation {
                FileOperation::Delete => return Err(PatchError::UnsupportedOperation),
                FileOperation::Add(lines) => {
                    let resolved = self
                        .workspace
                        .resolve_write(&operation.path)
                        .map_err(map_guard)?;
                    if resolved.target_exists() {
                        return Err(PatchError::CasMismatch);
                    }
                    if plans
                        .iter()
                        .any(|plan: &PlannedChange| plan.resolved.same_existing_target(&resolved))
                    {
                        return Err(PatchError::Parse);
                    }
                    plans.push(PlannedChange {
                        resolved,
                        expected_content: None,
                        content: lines_to_file(&lines),
                    });
                }
                FileOperation::Update(hunks) => {
                    let resolved =
                        self.workspace
                            .resolve_write(&operation.path)
                            .map_err(|error| {
                                if error.kind() == GuardErrorKind::NotFound {
                                    PatchError::CasMismatch
                                } else {
                                    map_guard(error)
                                }
                            })?;
                    if !resolved.target_exists() {
                        return Err(PatchError::CasMismatch);
                    }
                    let mut source = self.workspace.open_existing(&resolved).map_err(map_guard)?;
                    let mut bytes = Vec::with_capacity(4096);
                    Read::by_ref(&mut source)
                        .take((MAX_CONTENT_BYTES as u64).saturating_add(1))
                        .read_to_end(&mut bytes)
                        .map_err(|_| PatchError::Io)?;
                    if bytes.len() > MAX_CONTENT_BYTES {
                        return Err(PatchError::LimitExceeded);
                    }
                    let expected_content = ContentFingerprint::from_bytes(&bytes);
                    let text = String::from_utf8(bytes).map_err(|_| PatchError::InvalidInput)?;
                    let content = apply_hunks(&text, &hunks)?;
                    if content.len() > MAX_CONTENT_BYTES {
                        return Err(PatchError::LimitExceeded);
                    }
                    if plans
                        .iter()
                        .any(|plan: &PlannedChange| plan.resolved.same_existing_target(&resolved))
                    {
                        return Err(PatchError::Parse);
                    }
                    plans.push(PlannedChange {
                        resolved,
                        expected_content: Some(expected_content),
                        content,
                    });
                }
            }
        }

        let mut staged = Vec::with_capacity(plans.len());
        for plan in plans {
            let mut target = self
                .workspace
                .atomic_write_target(&plan.resolved)
                .map_err(map_guard)?;
            target
                .file_mut()
                .write_all(&plan.content)
                .map_err(|_| PatchError::Io)?;
            staged.push((target, plan.expected_content, plan.content.len() as u64));
        }

        let files_changed = staged.len();
        let mut bytes_written = 0_u64;
        for (target, expected_content, bytes) in staged {
            let result = match expected_content {
                Some(expected) => target.commit_if_unchanged(expected),
                None => target.commit(),
            };
            result.map_err(|error| {
                if matches!(error, GuardError::ContentChanged { .. }) {
                    PatchError::CasMismatch
                } else {
                    PatchError::UnknownOutcome
                }
            })?;
            bytes_written = bytes_written.saturating_add(bytes);
        }
        Ok(PatchResult {
            files_changed,
            bytes_written,
        })
    }
}

struct PlannedChange {
    resolved: ResolvedPath,
    expected_content: Option<ContentFingerprint>,
    content: Vec<u8>,
}

#[derive(Debug)]
struct FilePatch {
    path: PathBuf,
    operation: FileOperation,
}

#[derive(Debug)]
enum FileOperation {
    Update(Vec<Hunk>),
    Add(Vec<String>),
    Delete,
}

#[derive(Debug)]
struct Hunk {
    old_start: Option<usize>,
    old_count: Option<usize>,
    new_count: Option<usize>,
    lines: Vec<PatchLine>,
}

type HunkHeader = (Option<usize>, Option<usize>, Option<usize>);

#[derive(Debug)]
enum PatchLine {
    Context(String),
    Remove(String),
    Add(String),
}

fn parse_patch(patch: &str, limits: PatchLimits) -> Result<Vec<FilePatch>, PatchError> {
    if patch.len() > limits.max_patch_bytes || patch.contains('\0') || patch.contains('\r') {
        return Err(PatchError::LimitExceeded);
    }
    let mut lines = patch.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.first() != Some(&"*** Begin Patch") {
        return Err(PatchError::Parse);
    }
    if lines.len() < 2 || lines.last() != Some(&"*** End Patch") {
        return Err(PatchError::Parse);
    }

    let mut index = 1;
    let mut file_count = 0;
    let mut hunk_count = 0;
    let mut line_count = 0;
    let mut seen = BTreeSet::new();
    let mut operations = Vec::new();
    while index < lines.len() - 1 {
        let line = lines[index];
        let (path, operation) = if let Some(raw) = line.strip_prefix("*** Update File: ") {
            let path = parse_path(raw)?;
            index += 1;
            let mut hunks = Vec::new();
            while index < lines.len() - 1 && !is_file_header(lines[index]) {
                if !lines[index].starts_with("@@") {
                    return Err(PatchError::Parse);
                }
                let (old_start, old_count, new_count) = parse_hunk_header(lines[index])?;
                index += 1;
                let mut hunk_lines = Vec::new();
                while index < lines.len() - 1
                    && !lines[index].starts_with("@@")
                    && !is_file_header(lines[index])
                {
                    let source = lines[index];
                    if source.len() > MAX_PATCH_LINE_BYTES {
                        return Err(PatchError::LimitExceeded);
                    }
                    if source.is_empty() {
                        return Err(PatchError::Parse);
                    }
                    let operation = match source.as_bytes()[0] {
                        b' ' => PatchLine::Context(source[1..].to_owned()),
                        b'-' => PatchLine::Remove(source[1..].to_owned()),
                        b'+' => PatchLine::Add(source[1..].to_owned()),
                        _ => return Err(PatchError::Parse),
                    };
                    hunk_lines.push(operation);
                    line_count += 1;
                    if line_count > limits.max_lines {
                        return Err(PatchError::LimitExceeded);
                    }
                    index += 1;
                }
                if hunk_lines.is_empty() {
                    return Err(PatchError::Parse);
                }
                hunks.push(Hunk {
                    old_start,
                    old_count,
                    new_count,
                    lines: hunk_lines,
                });
                hunk_count += 1;
                if hunk_count > limits.max_hunks {
                    return Err(PatchError::LimitExceeded);
                }
            }
            if hunks.is_empty() {
                return Err(PatchError::Parse);
            }
            (path, FileOperation::Update(hunks))
        } else if let Some(raw) = line.strip_prefix("*** Add File: ") {
            let path = parse_path(raw)?;
            index += 1;
            let mut additions = Vec::new();
            while index < lines.len() - 1 && !is_file_header(lines[index]) {
                let source = lines[index];
                if source.len() > MAX_PATCH_LINE_BYTES || !source.starts_with('+') {
                    return Err(PatchError::Parse);
                }
                additions.push(source[1..].to_owned());
                line_count += 1;
                if line_count > limits.max_lines {
                    return Err(PatchError::LimitExceeded);
                }
                index += 1;
            }
            (path, FileOperation::Add(additions))
        } else if let Some(raw) = line.strip_prefix("*** Delete File: ") {
            let path = parse_path(raw)?;
            index += 1;
            (path, FileOperation::Delete)
        } else {
            return Err(PatchError::Parse);
        };
        if !seen.insert(path_key(&path)) {
            return Err(PatchError::Parse);
        }
        operations.push(FilePatch { path, operation });
        file_count += 1;
        if file_count > limits.max_files {
            return Err(PatchError::LimitExceeded);
        }
    }
    if operations.is_empty() {
        return Err(PatchError::Parse);
    }
    Ok(operations)
}

fn is_file_header(line: &str) -> bool {
    line.starts_with("*** Update File: ")
        || line.starts_with("*** Add File: ")
        || line.starts_with("*** Delete File: ")
        || line == "*** End Patch"
}

fn parse_path(raw: &str) -> Result<PathBuf, PatchError> {
    if raw.is_empty()
        || raw.len() > MAX_PATH_BYTES
        || raw.trim() != raw
        || raw.chars().any(char::is_control)
    {
        return Err(PatchError::Parse);
    }
    let path = PathBuf::from(raw);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else {
                    normalized.push("..");
                }
            }
            Component::RootDir | Component::Prefix(_) => return Err(PatchError::Parse),
        }
    }
    if normalized.as_os_str().is_empty() {
        normalized.push(".");
    }
    Ok(normalized)
}

fn path_key(path: &Path) -> String {
    let mut key = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    key.make_ascii_lowercase();
    key
}

fn parse_hunk_header(line: &str) -> Result<HunkHeader, PatchError> {
    if line == "@@" {
        return Ok((None, None, None));
    }
    let body = line.strip_prefix("@@").ok_or(PatchError::Parse)?;
    let close = body.find("@@").ok_or(PatchError::Parse)?;
    let mut ranges = body[..close].split_whitespace();
    let old = ranges.next().ok_or(PatchError::Parse)?;
    let new = ranges.next().ok_or(PatchError::Parse)?;
    if ranges.next().is_some() || !old.starts_with('-') || !new.starts_with('+') {
        return Err(PatchError::Parse);
    }
    let (old_start, old_count) = parse_range(&old[1..])?;
    let (new_start, new_count) = parse_range(&new[1..])?;
    let _ = new_start;
    Ok((Some(old_start), Some(old_count), Some(new_count)))
}

fn parse_range(value: &str) -> Result<(usize, usize), PatchError> {
    let (start, count) = value.split_once(',').map_or((value, "1"), |(s, c)| (s, c));
    let start = start.parse::<usize>().map_err(|_| PatchError::Parse)?;
    let count = count.parse::<usize>().map_err(|_| PatchError::Parse)?;
    if start == 0 && count != 0 {
        return Err(PatchError::Parse);
    }
    Ok((start, count))
}

fn apply_hunks(source: &str, hunks: &[Hunk]) -> Result<Vec<u8>, PatchError> {
    let (current, trailing_newline) = split_lines(source);
    let mut cursor = 0;
    let mut output = Vec::new();
    for hunk in hunks {
        let start = if let Some(old_start) = hunk.old_start {
            let start = if old_start == 0 { 0 } else { old_start - 1 };
            if start < cursor || start > current.len() {
                return Err(PatchError::CasMismatch);
            }
            start
        } else {
            find_hunk_start(&current, cursor, &hunk.lines)?
        };
        output.extend(current[cursor..start].iter().cloned());
        cursor = start;
        let mut old_seen = 0;
        let mut new_seen = 0;
        for line in &hunk.lines {
            match line {
                PatchLine::Context(value) => {
                    expect_line(&current, &mut cursor, value)?;
                    output.push(value.clone());
                    old_seen += 1;
                    new_seen += 1;
                }
                PatchLine::Remove(value) => {
                    expect_line(&current, &mut cursor, value)?;
                    old_seen += 1;
                }
                PatchLine::Add(value) => {
                    output.push(value.clone());
                    new_seen += 1;
                }
            }
        }
        if hunk.old_count.is_some_and(|expected| expected != old_seen)
            || hunk.new_count.is_some_and(|expected| expected != new_seen)
        {
            return Err(PatchError::Parse);
        }
    }
    output.extend(current[cursor..].iter().cloned());
    let has_addition = hunks.iter().any(|hunk| {
        hunk.lines
            .iter()
            .any(|line| matches!(line, PatchLine::Add(_)))
    });
    let mut result = output.join("\n");
    if !output.is_empty() && (trailing_newline || has_addition) {
        result.push('\n');
    }
    Ok(result.into_bytes())
}

fn find_hunk_start(
    current: &[String],
    cursor: usize,
    lines: &[PatchLine],
) -> Result<usize, PatchError> {
    let needle = lines
        .iter()
        .filter_map(|line| match line {
            PatchLine::Context(value) | PatchLine::Remove(value) => Some(value.as_str()),
            PatchLine::Add(_) => None,
        })
        .collect::<Vec<_>>();
    if needle.is_empty() {
        return Ok(cursor);
    }
    if needle.len() > current.len().saturating_sub(cursor) {
        return Err(PatchError::CasMismatch);
    }
    let mut prefix = vec![0; needle.len()];
    for index in 1..needle.len() {
        let mut matched = prefix[index - 1];
        while matched > 0 && needle[index] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[index] == needle[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }

    let mut found = None;
    let mut matched = 0;
    for (offset, line) in current[cursor..].iter().enumerate() {
        while matched > 0 && line != needle[matched] {
            matched = prefix[matched - 1];
        }
        if line == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            let start = cursor + offset + 1 - needle.len();
            if found.replace(start).is_some() {
                return Err(PatchError::AmbiguousMatch);
            }
            matched = prefix[matched - 1];
        }
    }
    found.ok_or(PatchError::CasMismatch)
}

fn expect_line(current: &[String], cursor: &mut usize, expected: &str) -> Result<(), PatchError> {
    if current.get(*cursor).map(String::as_str) != Some(expected) {
        return Err(PatchError::CasMismatch);
    }
    *cursor += 1;
    Ok(())
}

fn split_lines(source: &str) -> (Vec<String>, bool) {
    let trailing_newline = source.ends_with('\n');
    let body = source.strip_suffix('\n').unwrap_or(source);
    if body.is_empty() {
        if trailing_newline {
            (vec![String::new()], true)
        } else {
            (Vec::new(), false)
        }
    } else {
        (
            body.split('\n').map(str::to_owned).collect(),
            trailing_newline,
        )
    }
}

fn lines_to_file(lines: &[String]) -> Vec<u8> {
    if lines.is_empty() {
        return Vec::new();
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text.into_bytes()
}

fn map_guard(error: GuardError) -> PatchError {
    match error {
        GuardError::LimitExceeded { .. } => PatchError::LimitExceeded,
        GuardError::Io { .. } => PatchError::Io,
        other => PatchError::Guard(other.kind()),
    }
}
