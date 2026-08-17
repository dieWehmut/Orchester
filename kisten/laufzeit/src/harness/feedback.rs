//! Bounded, redacted feedback and deterministic loop guards.

use std::sync::OnceLock;

use orchester_protokoll::{FeedbackReport, StopReason};
use regex::{Captures, Regex};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::harness::secret_scan::is_format_character;

const STREAM_GUARD_BYTES: usize = 64;
const STREAM_SCAN_INTERVAL_BYTES: usize = 256;
const MAX_STREAM_BYTES: usize = orchester_modell::MAX_CONTENT_BYTES;
const STREAM_BOUNDARY_GUARD_BYTES: usize = MAX_STREAM_BYTES;
const PRIVATE_KEY_SENTINEL: &str = "\u{0}\u{1}";
const AUTHORIZATION_SENTINEL: &str = "\u{0}\u{2}";
const TOKEN_SENTINEL: &str = "\u{0}\u{3}";

/// Order-independent identity of the exact secret redaction set shared by
/// context assembly and durable persistence. The digest is never serialized
/// or displayed; it only prevents differently configured boundaries from
/// being wired together.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SecretSetId([u8; 32]);

impl SecretSetId {
    pub fn empty() -> Self {
        Self::from_secrets(&[])
    }

    pub(crate) fn from_secrets(secrets: &[SecretString]) -> Self {
        let mut members = secrets
            .iter()
            .map(|secret| {
                let value = secret.expose_secret();
                let mut hasher = Sha256::new();
                hasher.update(b"orchester-secret-member-v1\0");
                hasher.update((value.len() as u64).to_be_bytes());
                hasher.update(value.as_bytes());
                let digest: [u8; 32] = hasher.finalize().into();
                digest
            })
            .collect::<Vec<_>>();
        members.sort_unstable();

        let mut hasher = Sha256::new();
        hasher.update(b"orchester-secret-set-v1\0");
        hasher.update((members.len() as u64).to_be_bytes());
        for member in members {
            hasher.update(member);
        }
        Self(hasher.finalize().into())
    }
}

impl std::fmt::Debug for SecretSetId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretSetId(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackClass {
    DecodeError,
    PolicyDenied,
    ToolFailed,
    ProcessExit,
    ProcessCancelled,
    ProcessTimedOut,
    ProcessSpawnFailed,
    ValidatorFailed,
    ValidatorPassed,
    ValidatorMutatedSources,
    ValidatorOutputTruncated,
    SnapshotLimitExceeded,
    StorageFailed,
}

impl FeedbackClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DecodeError => "decode_error",
            Self::PolicyDenied => "policy_denied",
            Self::ToolFailed => "tool_failed",
            Self::ProcessExit => "process_exit",
            Self::ProcessCancelled => "process_cancelled",
            Self::ProcessTimedOut => "process_timed_out",
            Self::ProcessSpawnFailed => "process_spawn_failed",
            Self::ValidatorFailed => "validator_failed",
            Self::ValidatorPassed => "validator_passed",
            Self::ValidatorMutatedSources => "validator_mutated_sources",
            Self::ValidatorOutputTruncated => "validator_output_truncated",
            Self::SnapshotLimitExceeded => "snapshot_limit_exceeded",
            Self::StorageFailed => "storage_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackLimits {
    pub summary_bytes: usize,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

impl Default for FeedbackLimits {
    fn default() -> Self {
        Self {
            summary_bytes: 8 * 1024,
            stdout_bytes: 24 * 1024,
            stderr_bytes: 24 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackInput {
    pub source: String,
    pub validator_id: Option<String>,
    pub exit_code: Option<i32>,
    pub class: FeedbackClass,
    pub summary: String,
    pub stdout: String,
    pub stderr: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FeedbackTruncation {
    pub summary: bool,
    pub stdout: bool,
    pub stderr: bool,
}

impl FeedbackTruncation {
    pub fn any(self) -> bool {
        self.summary || self.stdout || self.stderr
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltFeedback {
    pub report: FeedbackReport,
    pub truncated: FeedbackTruncation,
}

#[derive(Debug, Clone)]
pub struct FeedbackEngine {
    limits: FeedbackLimits,
    secrets: Vec<SecretString>,
    normalized_secrets: Vec<SecretString>,
}

impl Default for FeedbackEngine {
    fn default() -> Self {
        Self::new(FeedbackLimits::default())
    }
}

impl FeedbackEngine {
    pub fn new(limits: FeedbackLimits) -> Self {
        Self {
            limits,
            secrets: Vec::new(),
            normalized_secrets: Vec::new(),
        }
    }

    /// Add a configured secret to the exact-value redaction set.  The value is
    /// kept in a secrecy wrapper and is never formatted or serialized.
    pub fn with_secret(mut self, secret: SecretString) -> Self {
        for normalized in [
            normalize_text(secret.expose_secret()),
            normalize_model_text(secret.expose_secret()),
        ] {
            if !normalized.is_empty()
                && !self
                    .normalized_secrets
                    .iter()
                    .any(|candidate| candidate.expose_secret() == normalized)
            {
                self.normalized_secrets
                    .push(SecretString::new(normalized.into_boxed_str()));
            }
        }
        self.normalized_secrets
            .sort_by_key(|secret| std::cmp::Reverse(secret.expose_secret().len()));
        self.secrets.push(secret);
        self
    }

    pub fn limits(&self) -> FeedbackLimits {
        self.limits
    }

    pub(crate) fn secret_set_id(&self) -> SecretSetId {
        SecretSetId::from_secrets(&self.secrets)
    }

    /// Detect sensitive material after applying the same terminal/control
    /// normalization used by durable redaction. Callers that send data to a
    /// provider can therefore reject a secret split by an escape or NUL byte
    /// before the raw value crosses that boundary.
    pub(crate) fn contains_sensitive_material(&self, input: &str) -> bool {
        let normalized = normalize_text(input);
        self.normalized_secrets.iter().any(|secret| {
            let value = secret.expose_secret();
            !value.is_empty() && normalized.contains(value)
        }) || looks_like_secret(&normalized)
            || private_key_pattern().is_match(&normalized)
            || authorization_pattern().is_match(&normalized)
            || token_pattern().is_match(&normalized)
    }

    pub fn build(&self, input: FeedbackInput) -> BuiltFeedback {
        // Sanitize and redact the complete diagnostic before either hashing or
        // truncating. This prevents truncation boundaries from leaking a token.
        let source = self.sanitize_text(&input.source);
        let validator_id = input.validator_id.as_ref().map(|id| self.sanitize_text(id));
        let summary = self.sanitize_text(&input.summary);
        let stdout = self.sanitize_text(&input.stdout);
        let stderr = self.sanitize_text(&input.stderr);
        let fingerprint = fingerprint(
            &source,
            validator_id.as_deref(),
            input.class,
            input.exit_code,
            &summary,
            &stdout,
            &stderr,
        );

        let (summary, summary_truncated) = truncate_head(&summary, self.limits.summary_bytes);
        let (stdout_tail, stdout_truncated) = truncate_tail(&stdout, self.limits.stdout_bytes);
        let (stderr_tail, stderr_truncated) = truncate_tail(&stderr, self.limits.stderr_bytes);
        BuiltFeedback {
            report: FeedbackReport {
                source: bounded_identifier(&source),
                validator_id: validator_id.map(|id| bounded_identifier(&id)),
                exit_code: input.exit_code,
                classification: input.class.as_str().to_owned(),
                summary,
                stdout_tail,
                stderr_tail,
                fingerprint,
                retryable: input.retryable,
            },
            truncated: FeedbackTruncation {
                summary: summary_truncated,
                stdout: stdout_truncated,
                stderr: stderr_truncated,
            },
        }
    }

    pub(crate) fn sanitize_text(&self, input: &str) -> String {
        // Remove controls before exact-value redaction so an attacker cannot
        // split a configured secret with NUL/escape/newline bytes to evade the
        // matcher while remaining visually reconstructable in a terminal.
        self.redact_text(normalize_text(input))
    }

    pub(crate) fn sanitize_model_text(&self, input: &str) -> String {
        self.sanitize_normalized_model_text(normalize_model_text(input))
    }

    fn sanitize_normalized_model_text(&self, normalized: String) -> String {
        let sanitized = self.redact_text(normalized);
        if self.contains_sensitive_material(&sanitized) {
            "[REDACTED]".into()
        } else {
            sanitized
        }
    }

    fn redact_text(&self, mut sanitized: String) -> String {
        // Protect detector matches with control sentinels that normalized input
        // and normalized configured secrets cannot contain. Exact-value
        // replacement therefore cannot break a detector match or rewrite its
        // eventual marker.
        sanitized = private_key_pattern()
            .replace_all(&sanitized, PRIVATE_KEY_SENTINEL)
            .into_owned();
        sanitized = authorization_pattern()
            .replace_all(&sanitized, |captures: &Captures<'_>| {
                format!("{}{AUTHORIZATION_SENTINEL}", &captures[1])
            })
            .into_owned();
        sanitized = token_pattern()
            .replace_all(&sanitized, TOKEN_SENTINEL)
            .into_owned();
        for secret in &self.normalized_secrets {
            let secret = secret.expose_secret();
            if !secret.is_empty() {
                sanitized = sanitized.replace(secret, "[REDACTED]");
            }
        }
        sanitized = sanitized.replace(PRIVATE_KEY_SENTINEL, "[REDACTED_PRIVATE_KEY]");
        sanitized = sanitized.replace(AUTHORIZATION_SENTINEL, "[REDACTED]");
        sanitized.replace(TOKEN_SENTINEL, "[REDACTED_TOKEN]")
    }

    fn sensitive_material_crosses_boundary(&self, previous: &str, current: &str) -> bool {
        if previous.is_empty() || current.is_empty() {
            return false;
        }
        let boundary = previous.len();
        let mut combined = String::with_capacity(boundary.saturating_add(current.len()));
        combined.push_str(previous);
        combined.push_str(current);

        self.normalized_secrets
            .iter()
            .any(|secret| literal_crosses_boundary(&combined, boundary, secret.expose_secret()))
            || regex_crosses_boundary(token_pattern(), &combined, boundary)
            || regex_crosses_boundary(authorization_pattern(), &combined, boundary)
            || regex_crosses_boundary(private_key_pattern(), &combined, boundary)
            || provider_token_prefix_crosses_boundary(&combined, boundary)
            || regex_crosses_boundary(private_key_begin_pattern(), &combined, boundary)
            || regex_crosses_boundary(authorization_prefix_pattern(), &combined, boundary)
    }
}

/// Incrementally redacts one provider text stream without exposing chunk
/// boundaries. The visible value is a complete snapshot, so callers can
/// replace previously rendered text when a later chunk completes a token.
pub struct StreamingRedactor {
    sanitizer: FeedbackEngine,
    raw: String,
    visible: String,
    previous_response_tail: String,
    guard_bytes: usize,
    boundary_guard_bytes: usize,
    next_scan_bytes: usize,
}

impl StreamingRedactor {
    pub fn new(secrets: Vec<SecretString>) -> Self {
        let sanitizer = secrets
            .into_iter()
            .fold(FeedbackEngine::default(), FeedbackEngine::with_secret);
        Self::from_sanitizer(sanitizer)
    }

    pub(crate) fn from_sanitizer(sanitizer: FeedbackEngine) -> Self {
        let guard_bytes = sanitizer
            .normalized_secrets
            .iter()
            .map(|secret| secret.expose_secret().len())
            .max()
            .unwrap_or_default()
            .max(STREAM_GUARD_BYTES);
        Self {
            sanitizer,
            raw: String::new(),
            visible: String::new(),
            previous_response_tail: String::new(),
            guard_bytes,
            boundary_guard_bytes: guard_bytes.max(STREAM_BOUNDARY_GUARD_BYTES),
            next_scan_bytes: guard_bytes.saturating_add(1).min(MAX_STREAM_BYTES),
        }
    }

    /// Start a fresh provider response while retaining the exact sanitizer.
    pub fn begin_response(&mut self) {
        self.remember_response_tail();
        self.raw.clear();
        self.visible.clear();
        self.next_scan_bytes = self.guard_bytes.saturating_add(1).min(MAX_STREAM_BYTES);
    }

    /// Append raw provider text and return the full safe snapshot to render.
    pub fn push(&mut self, delta: &str) -> &str {
        if !append_bounded(&mut self.raw, delta, MAX_STREAM_BYTES) {
            return &self.visible;
        }
        let final_bounded_scan = self.raw.len() == MAX_STREAM_BYTES;
        if self.raw.len() < self.next_scan_bytes && !final_bounded_scan {
            return &self.visible;
        }
        // Each complete snapshot costs O(raw bytes). Grow the next threshold
        // geometrically so the sum of all rescanned prefixes is O(total bytes)
        // while still producing early incremental output.
        self.next_scan_bytes = next_stream_scan_bytes(self.raw.len());
        let sanitized = self.sanitize_current_response();
        if self.sanitizer.contains_sensitive_material(&sanitized) {
            return &self.visible;
        }
        let end = guarded_prefix_end(&sanitized, self.guard_bytes);
        self.visible.clear();
        self.visible.push_str(&sanitized[..end]);
        &self.visible
    }

    /// Flush the final safe snapshot after the provider stream completes.
    pub fn finish(&mut self) -> &str {
        let sanitized = self.sanitize_current_response();
        self.visible = if self.sanitizer.contains_sensitive_material(&sanitized) {
            "[REDACTED]".into()
        } else {
            sanitized
        };
        self.remember_response_tail();
        self.raw.clear();
        self.next_scan_bytes = self.guard_bytes.saturating_add(1).min(MAX_STREAM_BYTES);
        &self.visible
    }

    fn sanitize_current_response(&self) -> String {
        let current = normalize_model_text(&self.raw);
        let boundary_prefix = utf8_prefix(&current, self.boundary_guard_bytes);
        if self
            .sanitizer
            .sensitive_material_crosses_boundary(&self.previous_response_tail, boundary_prefix)
        {
            "[REDACTED]".into()
        } else {
            self.sanitizer.sanitize_normalized_model_text(current)
        }
    }

    fn remember_response_tail(&mut self) {
        if self.raw.is_empty() {
            return;
        }
        let current = normalize_model_text(&self.raw);
        let mut combined = String::with_capacity(
            self.previous_response_tail
                .len()
                .saturating_add(current.len()),
        );
        combined.push_str(&self.previous_response_tail);
        combined.push_str(&current);
        self.previous_response_tail = utf8_suffix(&combined, self.boundary_guard_bytes).to_owned();
    }
}

impl std::fmt::Debug for StreamingRedactor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamingRedactor")
            .field("raw_bytes", &self.raw.len())
            .field("visible_bytes", &self.visible.len())
            .field(
                "previous_response_tail_bytes",
                &self.previous_response_tail.len(),
            )
            .field("boundary_guard_bytes", &self.boundary_guard_bytes)
            .field("next_scan_bytes", &self.next_scan_bytes)
            .finish_non_exhaustive()
    }
}

fn append_bounded(target: &mut String, value: &str, limit: usize) -> bool {
    let remaining = limit.saturating_sub(target.len());
    let mut end = value.len().min(remaining);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    if end == 0 {
        return false;
    }
    target.push_str(&value[..end]);
    true
}

fn guarded_prefix_end(value: &str, guard_bytes: usize) -> usize {
    let mut end = value.len().saturating_sub(guard_bytes);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn next_stream_scan_bytes(scanned_bytes: usize) -> usize {
    scanned_bytes
        .saturating_add(scanned_bytes.max(STREAM_SCAN_INTERVAL_BYTES))
        .min(MAX_STREAM_BYTES)
}

fn literal_crosses_boundary(value: &str, boundary: usize, needle: &str) -> bool {
    if needle.is_empty() || needle.len() <= 1 {
        return false;
    }
    let mut search_start = boundary.saturating_sub(needle.len() - 1);
    while search_start > 0 && !value.is_char_boundary(search_start) {
        search_start -= 1;
    }
    for (start, _) in value[search_start..].match_indices(needle) {
        let start = search_start + start;
        if start >= boundary {
            return false;
        }
        if start.saturating_add(needle.len()) > boundary {
            return true;
        }
    }
    false
}

fn regex_crosses_boundary(pattern: &Regex, value: &str, boundary: usize) -> bool {
    for matched in pattern.find_iter(value) {
        if matched.start() >= boundary {
            return false;
        }
        if matched.end() > boundary {
            return true;
        }
    }
    false
}

fn provider_token_prefix_crosses_boundary(value: &str, boundary: usize) -> bool {
    for matched in provider_token_boundary_pattern().find_iter(value) {
        if matched.start() >= boundary {
            return false;
        }
        let has_token_boundary = matched.start() == 0
            || value[..matched.start()]
                .chars()
                .next_back()
                .is_some_and(|ch| !ch.is_ascii_alphanumeric());
        if has_token_boundary && matched.end() > boundary {
            return true;
        }
    }
    false
}

fn normalize_text(input: &str) -> String {
    let mut normalized = ansi_pattern().replace_all(input, "").into_owned();
    normalized.retain(|ch| !ch.is_control() && !is_format_character(ch));
    normalized
}

fn normalize_model_text(input: &str) -> String {
    let mut normalized = ansi_pattern().replace_all(input, "").into_owned();
    normalized
        .retain(|ch| !is_format_character(ch) && (!ch.is_control() || matches!(ch, '\n' | '\t')));
    normalized
}

fn looks_like_secret(value: &str) -> bool {
    private_key_begin_pattern().is_match(value)
        || provider_token_prefix_pattern().is_match(value)
        || authorization_prefix_pattern().is_match(value)
}

fn fingerprint(
    source: &str,
    validator_id: Option<&str>,
    class: FeedbackClass,
    exit_code: Option<i32>,
    summary: &str,
    stdout: &str,
    stderr: &str,
) -> String {
    let diagnostics = normalize_diagnostics(&format!("{summary}\n{stdout}\n{stderr}"));
    let mut hasher = Sha256::new();
    for field in [
        source,
        validator_id.unwrap_or(""),
        class.as_str(),
        &exit_code.map(|code| code.to_string()).unwrap_or_default(),
        &diagnostics,
    ] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field.as_bytes());
    }
    hex(&hasher.finalize())
}

fn normalize_diagnostics(input: &str) -> String {
    let mut normalized = timestamp_pattern()
        .replace_all(input, "<timestamp>")
        .into_owned();
    normalized = duration_pattern()
        .replace_all(&normalized, "<duration>")
        .into_owned();
    normalized = temp_path_pattern()
        .replace_all(&normalized, "<temp>")
        .into_owned();
    normalized = port_pattern()
        .replace_all(&normalized, "${prefix}<port>")
        .into_owned();
    normalized = line_column_pattern()
        .replace_all(&normalized, "${prefix}:<line>:<column>")
        .into_owned();
    normalized = hex_address_pattern()
        .replace_all(&normalized, "<address>")
        .into_owned();
    normalized
}

fn truncate_head(input: &str, max_bytes: usize) -> (String, bool) {
    if input.len() <= max_bytes {
        return (input.to_owned(), false);
    }
    const MARKER: &str = "[truncated] ";
    if max_bytes <= MARKER.len() {
        return (utf8_prefix(MARKER, max_bytes).to_owned(), true);
    }
    let keep = max_bytes - MARKER.len();
    (format!("{MARKER}{}", utf8_prefix(input, keep)), true)
}

fn truncate_tail(input: &str, max_bytes: usize) -> (String, bool) {
    if input.len() <= max_bytes {
        return (input.to_owned(), false);
    }
    const MARKER: &str = "[truncated] ";
    if max_bytes <= MARKER.len() {
        return (utf8_prefix(MARKER, max_bytes).to_owned(), true);
    }
    let keep = max_bytes - MARKER.len();
    (format!("{MARKER}{}", utf8_suffix(input, keep)), true)
}

fn utf8_prefix(input: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(input.len());
    while !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

fn utf8_suffix(input: &str, max_bytes: usize) -> &str {
    let mut start = input.len().saturating_sub(max_bytes);
    while !input.is_char_boundary(start) {
        start += 1;
    }
    &input[start..]
}

fn bounded_identifier(input: &str) -> String {
    utf8_prefix(input, 128).to_owned()
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

fn ansi_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))").unwrap()
    })
}

fn private_key_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?s)-----BEGIN [^-\r\n]*PRIVATE KEY-----.*?-----END [^-\r\n]*PRIVATE KEY-----")
            .unwrap()
    })
}

fn private_key_begin_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)-----BEGIN [^-\r\n]*PRIVATE KEY-----")
            .expect("static private-key prefix pattern")
    })
}

fn authorization_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"(?i)(authorization\s*:\s*(?:bearer|basic)\s+)[^\s]+").unwrap())
}

fn authorization_prefix_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)authorization\s*:\s*(?:bearer|basic)\s+")
            .expect("static authorization prefix pattern")
    })
}

fn provider_token_prefix_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[^A-Za-z0-9])(?:sk[-_]|ghp_|github_pat_|xox[baprs]-|AKIA)")
            .expect("static provider-token prefix pattern")
    })
}

fn provider_token_boundary_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:sk[-_]|ghp_|github_pat_|xox[baprs]-|AKIA)")
            .expect("static provider-token boundary pattern")
    })
}

fn token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:sk[-_]|ghp_|github_pat_|xox[baprs]-)[A-Za-z0-9._-]{8,}\b|\bAKIA[A-Z0-9]{12,}\b")
            .unwrap()
    })
}

fn timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b20\d{2}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?\b").unwrap()
    })
}

fn duration_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"\b\d+(?:\.\d+)?(?:ns|us|ms|s|sec|secs|seconds?)\b").unwrap())
}

fn temp_path_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:[a-z]:[\\/](?:[^\\/\s]+[\\/])*(?:temp|tmp)[\\/][^\s:]+|/(?:tmp|var/tmp)/[^\s:]+)")
            .unwrap()
    })
}

fn line_column_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?m)(?P<prefix>(?:<temp>|[^\s:]*[./\\][^\s:]+)):\d+(?::\d+)?").unwrap()
    })
}

fn port_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(?P<prefix>port\s+|(?:localhost|127\.0\.0\.1):)\d{2,5}\b").unwrap()
    })
}

fn hex_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b0x[0-9a-fA-F]{6,}\b").unwrap())
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LoopGuardConfigError {
    #[error("repeated-failure threshold must be greater than zero")]
    ZeroThreshold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureLoopGuard {
    threshold: u32,
    failure: RepetitionCounter,
    action: RepetitionCounter,
}

impl Default for FailureLoopGuard {
    fn default() -> Self {
        Self::new(3).expect("non-zero default threshold")
    }
}

impl FailureLoopGuard {
    pub fn new(threshold: u32) -> Result<Self, LoopGuardConfigError> {
        if threshold == 0 {
            return Err(LoopGuardConfigError::ZeroThreshold);
        }
        Ok(Self {
            threshold,
            failure: RepetitionCounter::default(),
            action: RepetitionCounter::default(),
        })
    }

    pub fn record_failure(&mut self, fingerprint: &str) -> Option<StopReason> {
        self.failure.record(fingerprint, self.threshold)
    }

    pub fn record_no_progress_action(&mut self, action_hash: &str) -> Option<StopReason> {
        self.action.record(action_hash, self.threshold)
    }

    /// A successful observation only proves progress when source or durable
    /// state actually changed.
    pub fn record_success(&mut self, state_changed: bool) {
        if state_changed {
            self.failure.reset();
            self.action.reset();
        }
    }

    pub fn failure_count(&self) -> u32 {
        self.failure.count
    }

    pub fn action_count(&self) -> u32 {
        self.action.count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RepetitionCounter {
    signature: Option<String>,
    count: u32,
}

impl RepetitionCounter {
    fn record(&mut self, signature: &str, threshold: u32) -> Option<StopReason> {
        if self.signature.as_deref() == Some(signature) {
            self.count = self.count.saturating_add(1);
        } else {
            self.signature = Some(signature.to_owned());
            self.count = 1;
        }
        (self.count >= threshold).then_some(StopReason::RepeatedFailure)
    }

    fn reset(&mut self) {
        self.signature = None;
        self.count = 0;
    }
}
