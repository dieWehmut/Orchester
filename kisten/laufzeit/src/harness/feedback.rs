//! Bounded, redacted feedback and deterministic loop guards.

use std::sync::OnceLock;

use orchester_protokoll::{FeedbackReport, StopReason};
use regex::{Captures, Regex};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

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
        }
    }

    /// Add a configured secret to the exact-value redaction set.  The value is
    /// kept in a secrecy wrapper and is never formatted or serialized.
    pub fn with_secret(mut self, secret: SecretString) -> Self {
        self.secrets.push(secret);
        self
    }

    pub fn limits(&self) -> FeedbackLimits {
        self.limits
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
        let mut sanitized = ansi_pattern().replace_all(input, "").into_owned();
        // Remove controls before exact-value redaction so an attacker cannot
        // split a configured secret with NUL/escape/newline bytes to evade the
        // matcher while remaining visually reconstructable in a terminal.
        sanitized.retain(|ch| !ch.is_control());
        for secret in &self.secrets {
            let secret = secret.expose_secret();
            if !secret.is_empty() {
                sanitized = sanitized.replace(secret, "[REDACTED]");
            }
        }
        sanitized = private_key_pattern()
            .replace_all(&sanitized, "[REDACTED_PRIVATE_KEY]")
            .into_owned();
        sanitized = authorization_pattern()
            .replace_all(&sanitized, |captures: &Captures<'_>| {
                format!("{}[REDACTED]", &captures[1])
            })
            .into_owned();
        token_pattern()
            .replace_all(&sanitized, "[REDACTED_TOKEN]")
            .into_owned()
    }
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

fn authorization_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"(?i)(authorization\s*:\s*(?:bearer|basic)\s+)[^\s]+").unwrap())
}

fn token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:sk[-_]|ghp_|github_pat_|xox[baprs]-)[A-Za-z0-9._-]{5,}\b|\bAKIA[A-Z0-9]{12,}\b")
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
