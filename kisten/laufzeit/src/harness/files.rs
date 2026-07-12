//! Capability-backed read-only workspace tools.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::governance::{GuardError, GuardErrorKind, WorkspaceGuard};

const MAX_LIST_DEPTH: u16 = 64;
const MAX_DIRECTORY_BATCH: usize = 4_096;
const MAX_CONFIGURED_READ_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CONFIGURED_LIST_ENTRIES: usize = 100_000;
const MAX_CONFIGURED_SEARCH_MATCHES: usize = 10_000;
const MAX_CONFIGURED_MATCH_BYTES: usize = 64 * 1024;
const MAX_QUERY_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileToolLimits {
    pub max_read_bytes: u64,
    pub max_list_entries: usize,
    pub max_search_matches: usize,
    pub max_match_bytes: usize,
}

impl Default for FileToolLimits {
    fn default() -> Self {
        Self {
            max_read_bytes: 16 * 1024 * 1024,
            max_list_entries: 10_000,
            max_search_matches: 200,
            max_match_bytes: 4 * 1024,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FileToolError {
    #[error("workspace path operation was rejected: {0:?}")]
    Guard(GuardErrorKind),
    #[error("file content exceeds the configured output limit")]
    LimitExceeded,
    #[error("file content is not valid UTF-8")]
    InvalidUtf8,
    #[error("file tool input is invalid")]
    InvalidInput,
    #[error("workspace filesystem operation failed")]
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedEntry {
    pub path: PathBuf,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListResult {
    pub entries: Vec<ListedEntry>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ReadResult {
    pub content: String,
    pub bytes: u64,
    pub lines: usize,
}

impl fmt::Debug for ReadResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadResult")
            .field("content_bytes", &self.content.len())
            .field("bytes", &self.bytes)
            .field("lines", &self.lines)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

impl fmt::Debug for SearchMatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchMatch")
            .field("path", &self.path)
            .field("line", &self.line)
            .field("text_bytes", &self.text.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub skipped_oversized_files: usize,
}

pub struct FileTools {
    workspace: WorkspaceGuard,
    limits: FileToolLimits,
}

impl fmt::Debug for FileTools {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FileTools")
            .field("workspace", &self.workspace)
            .field("limits", &self.limits)
            .finish()
    }
}

impl FileTools {
    pub fn new(root: impl AsRef<Path>, limits: FileToolLimits) -> Result<Self, FileToolError> {
        if limits.max_read_bytes == 0
            || limits.max_list_entries == 0
            || limits.max_search_matches == 0
            || limits.max_match_bytes == 0
            || limits.max_read_bytes > MAX_CONFIGURED_READ_BYTES
            || limits.max_list_entries > MAX_CONFIGURED_LIST_ENTRIES
            || limits.max_search_matches > MAX_CONFIGURED_SEARCH_MATCHES
            || limits.max_match_bytes > MAX_CONFIGURED_MATCH_BYTES
        {
            return Err(FileToolError::InvalidInput);
        }
        let workspace = WorkspaceGuard::new(root.as_ref()).map_err(map_guard)?;
        Ok(Self { workspace, limits })
    }

    pub fn workspace(&self) -> &WorkspaceGuard {
        &self.workspace
    }

    pub fn read_file(
        &self,
        requested: &Path,
        start_line: Option<u32>,
        end_line: Option<u32>,
    ) -> Result<ReadResult, FileToolError> {
        if matches!((start_line, end_line), (Some(0), _) | (_, Some(0)))
            || matches!((start_line, end_line), (Some(start), Some(end)) if start > end)
        {
            return Err(FileToolError::InvalidInput);
        }
        let bytes = self
            .workspace
            .read_file_bounded(requested, self.limits.max_read_bytes)
            .map_err(map_guard)?;
        let source_bytes = bytes.len() as u64;
        let text = String::from_utf8(bytes).map_err(|_| FileToolError::InvalidUtf8)?;
        let all_lines = text.lines().collect::<Vec<_>>();
        let start = start_line.unwrap_or(1) as usize;
        let end = end_line
            .map(|line| line as usize)
            .unwrap_or(all_lines.len());
        let content = if start > all_lines.len() {
            String::new()
        } else {
            all_lines[start - 1..end.min(all_lines.len())]
                .iter()
                .map(|line| redact_text(line))
                .collect::<Vec<_>>()
                .join("\n")
        };
        Ok(ReadResult {
            lines: content.lines().count(),
            content,
            bytes: source_bytes,
        })
    }

    pub fn list_files(&self, requested: &Path, depth: u16) -> Result<ListResult, FileToolError> {
        if depth == 0 || depth > MAX_LIST_DEPTH {
            return Err(FileToolError::InvalidInput);
        }
        let mut entries = Vec::new();
        match self.workspace.directory_entries(requested) {
            Ok(children) => self.visit_directory(requested, children, depth, &mut entries)?,
            Err(error) if error.kind() == GuardErrorKind::NotDirectory => {
                self.push_entry(&mut entries, requested, EntryKind::File)?;
            }
            Err(error) => return Err(map_guard(error)),
        }
        Ok(ListResult { entries })
    }

    pub fn search_text(
        &self,
        requested: &Path,
        query: &str,
    ) -> Result<SearchResult, FileToolError> {
        if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.chars().any(char::is_control)
        {
            return Err(FileToolError::InvalidInput);
        }
        let listed = self.list_files(requested, MAX_LIST_DEPTH)?;
        let mut matches = Vec::new();
        let mut skipped_oversized_files = 0;
        for entry in listed.entries {
            if entry.kind != EntryKind::File {
                continue;
            }
            let bytes = match self
                .workspace
                .read_file_bounded(&entry.path, self.limits.max_read_bytes)
            {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == GuardErrorKind::LimitExceeded => {
                    skipped_oversized_files += 1;
                    continue;
                }
                Err(error) => return Err(map_guard(error)),
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line.contains(query) {
                    if matches.len() >= self.limits.max_search_matches {
                        return Err(FileToolError::LimitExceeded);
                    }
                    matches.push(SearchMatch {
                        path: entry.path.clone(),
                        line: index + 1,
                        text: truncate_text(&redact_text(line), self.limits.max_match_bytes),
                    });
                }
            }
        }
        Ok(SearchResult {
            matches,
            skipped_oversized_files,
        })
    }

    fn visit_directory(
        &self,
        relative: &Path,
        children: impl Iterator<Item = Result<OsString, GuardError>>,
        remaining_depth: u16,
        output: &mut Vec<ListedEntry>,
    ) -> Result<(), FileToolError> {
        let batch_limit = self
            .limits
            .max_list_entries
            .saturating_add(1)
            .min(MAX_DIRECTORY_BATCH + 1);
        let mut names = Vec::new();
        for child in children {
            if names.len() >= batch_limit {
                return Err(FileToolError::LimitExceeded);
            }
            names.push(child.map_err(map_guard)?);
        }
        names.sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
        for name in names {
            let child = relative.join(name);
            match self.workspace.directory_entries(&child) {
                Ok(grandchildren) => {
                    self.push_entry(output, &child, EntryKind::Directory)?;
                    if remaining_depth > 1 {
                        self.visit_directory(&child, grandchildren, remaining_depth - 1, output)?;
                    }
                }
                Err(error) if error.kind() == GuardErrorKind::NotDirectory => {
                    self.push_entry(output, &child, EntryKind::File)?;
                }
                Err(error) => return Err(map_guard(error)),
            }
        }
        Ok(())
    }

    fn push_entry(
        &self,
        output: &mut Vec<ListedEntry>,
        path: &Path,
        kind: EntryKind,
    ) -> Result<(), FileToolError> {
        if output.len() >= self.limits.max_list_entries {
            return Err(FileToolError::LimitExceeded);
        }
        output.push(ListedEntry {
            path: path.to_path_buf(),
            kind,
        });
        Ok(())
    }
}

fn map_guard(error: GuardError) -> FileToolError {
    match error {
        GuardError::LimitExceeded { .. } => FileToolError::LimitExceeded,
        GuardError::Io { source, .. }
            if matches!(
                source.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            FileToolError::Io
        }
        other => FileToolError::Guard(other.kind()),
    }
}

fn redact_text(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("-----begin ") && lower.contains("private key-----") {
        return "[REDACTED PRIVATE KEY]".into();
    }
    for key in [
        "openai_api_key",
        "anthropic_api_key",
        "api_key",
        "authorization",
        "password",
        "secret",
        "token",
    ] {
        if let Some(index) = lower.find(key) {
            let rest = &line[index + key.len()..];
            if rest.starts_with('=') || rest.starts_with(':') || rest.starts_with(" =") {
                return format!("{}=[REDACTED]", &line[..index + key.len()]);
            }
        }
    }
    let secret_tokens = line
        .split_whitespace()
        .filter(|token| {
            ["sk-", "ghp_", "github_pat_", "xoxb-", "xoxp-"]
                .iter()
                .any(|prefix| token.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    if secret_tokens.is_empty() {
        return line.to_owned();
    }
    let mut redacted = line.to_owned();
    for token in secret_tokens {
        redacted = redacted.replace(token, "[REDACTED]");
    }
    redacted
}

fn truncate_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes.saturating_sub(3);
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}...", &value[..end])
}
