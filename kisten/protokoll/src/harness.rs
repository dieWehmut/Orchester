//! Versioned wire types for the self-owned harness.
//!
//! These types deliberately contain no execution or provider logic. They are
//! the durable boundary shared by the harness loop, CLI, audit stream, and
//! later WebUI adapters.

use std::fmt;

use serde::de::Error as DeError;
use serde::ser::{Error as SerError, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The only harness envelope schema currently accepted on the wire.
pub const HARNESS_SCHEMA_VERSION: u16 = 1;
const MAX_ACTION_SUMMARY_CHARS: usize = 512;

macro_rules! string_id {
    ($name:ident) => {
        /// A strongly typed identifier used by the harness protocol.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

string_id!(EventId);
string_id!(RunId);
string_id!(TurnId);
string_id!(StepId);
string_id!(CallId);
string_id!(ActionId);
string_id!(ApprovalId);
string_id!(ObservationId);

/// A protocol invariant violation. The error intentionally never contains a
/// complete action or summary, so returning it to a CLI/UI cannot leak a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolValidationError {
    UnsupportedSchemaVersion { found: u16, expected: u16 },
    InvalidSequence { found: u64 },
    EmptyActionSummary,
    ActionSummaryNotNormalized,
    ActionSummaryTooLong,
    ApprovalRunIdMismatch { outer: RunId, request: RunId },
}

impl fmt::Display for ProtocolValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found, expected } => {
                write!(
                    f,
                    "unsupported harness schema version {found}; expected {expected}"
                )
            }
            Self::InvalidSequence { found } => {
                write!(
                    f,
                    "harness sequence must be greater than zero (got {found})"
                )
            }
            Self::EmptyActionSummary => f.write_str("approval action summary must not be empty"),
            Self::ActionSummaryNotNormalized => {
                f.write_str("approval action summary is not normalized or redacted")
            }
            Self::ActionSummaryTooLong => f.write_str("approval action summary is too long"),
            Self::ApprovalRunIdMismatch { outer, request } => write!(
                f,
                "approval request run binding does not match event run ({}/{})",
                outer.0, request.0
            ),
        }
    }
}

impl std::error::Error for ProtocolValidationError {}

/// An action candidate produced by the model and subsequently checked by the
/// governance layer before it can reach a tool runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentAction {
    ListFiles {
        path: String,
        depth: u16,
    },
    SearchText {
        path: String,
        query: String,
    },
    ReadFile {
        path: String,
        start_line: Option<u32>,
        end_line: Option<u32>,
    },
    WriteFile {
        path: String,
        content: String,
    },
    ApplyPatch {
        patch: String,
    },
    RunCommand {
        program: String,
        args: Vec<String>,
        cwd: Option<String>,
    },
    RunChecks {
        ids: Vec<String>,
    },
    Remember {
        kind: MemoryKind,
        content: String,
    },
    Recall {
        query: String,
        limit: u16,
    },
    RequestApproval {
        reason: String,
    },
    Finish {
        summary: String,
    },
}

impl AgentAction {
    /// Build a short, side-effect-free summary suitable for an approval UI.
    ///
    /// Every model-controlled string is represented by its byte length rather
    /// than copied into the durable approval/audit view.  This is stronger
    /// than relying on a credential-prefix regex: a newly introduced token
    /// format or an ANSI/control payload cannot bypass the summary boundary.
    pub fn action_summary(&self) -> String {
        let raw = match self {
            Self::ListFiles { path, depth } => {
                format!("list_files path_bytes={} depth={depth}", path.len())
            }
            Self::SearchText { path, query } => format!(
                "search_text path_bytes={} query_bytes={}",
                path.len(),
                query.len()
            ),
            Self::ReadFile {
                path,
                start_line,
                end_line,
            } => format!(
                "read_file path_bytes={} start_line={:?} end_line={:?}",
                path.len(),
                start_line,
                end_line
            ),
            Self::WriteFile { path, content } => {
                format!(
                    "write_file path_bytes={} content_bytes={}",
                    path.len(),
                    content.len()
                )
            }
            Self::ApplyPatch { patch } => format!("apply_patch patch_bytes={}", patch.len()),
            Self::RunCommand { program, args, cwd } => format!(
                "run_command program_bytes={} args_count={} args_bytes={} cwd_bytes={}",
                program.len(),
                args.len(),
                args.iter().map(String::len).sum::<usize>(),
                cwd.as_deref().map_or(0, str::len)
            ),
            Self::RunChecks { ids } => format!(
                "run_checks ids_count={} ids_bytes={}",
                ids.len(),
                ids.iter().map(String::len).sum::<usize>()
            ),
            Self::Remember { kind, content } => format!(
                "remember kind={} content_bytes={}",
                memory_kind_name(kind),
                content.len()
            ),
            Self::Recall { query, limit } => {
                format!("recall query_bytes={} limit={limit}", query.len())
            }
            Self::RequestApproval { reason } => {
                format!("request_approval reason_bytes={}", reason.len())
            }
            Self::Finish { summary } => format!("finish summary_bytes={}", summary.len()),
        };
        normalize_action_summary(&raw)
    }

    /// Alias emphasizing that the returned text is safe for durable display.
    pub fn redacted_summary(&self) -> String {
        self.action_summary()
    }
}

fn memory_kind_name(kind: &MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Convention => "convention",
        MemoryKind::ArchitectureDecision => "architecture_decision",
        MemoryKind::Lesson => "lesson",
    }
}

/// Normalize whitespace and redact common credential forms in an approval
/// summary. This is deliberately conservative: false positives are safer than
/// writing an API key to an approval row, audit event, or UI stream.
pub fn normalize_action_summary(input: &str) -> String {
    let collapsed = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut output = Vec::with_capacity(collapsed.len());
    let mut redact_next = false;

    for token in collapsed.split(' ') {
        if redact_next {
            output.push("[REDACTED]".to_owned());
            redact_next = false;
            continue;
        }

        let (redacted, consumes_next) = redact_summary_token(token);
        if consumes_next {
            redact_next = true;
        }
        output.push(redacted);
    }

    let mut normalized = output.join(" ");
    let char_count = normalized.chars().count();
    if char_count > MAX_ACTION_SUMMARY_CHARS {
        normalized = normalized
            .chars()
            .take(MAX_ACTION_SUMMARY_CHARS - 3)
            .collect::<String>();
        normalized.push_str("...");
    }
    normalized
}

fn redact_summary_token(token: &str) -> (String, bool) {
    if token.is_empty() {
        return (String::new(), false);
    }

    if token.eq_ignore_ascii_case("bearer")
        || token.eq_ignore_ascii_case("authorization")
        || token.eq_ignore_ascii_case("token")
    {
        return (token.to_owned(), true);
    }

    if let Some(separator) = token.find(['=', ':']) {
        let key = &token[..separator];
        let value = &token[separator + 1..];
        if is_sensitive_key(key) {
            if value.is_empty() {
                return (token.to_owned(), true);
            }
            return (format!("{key}=[REDACTED]"), false);
        }
    }

    let key = token.trim_start_matches('-');
    if is_sensitive_key(key) {
        return (token.to_owned(), true);
    }

    if looks_like_secret(token) {
        return ("[REDACTED]".to_owned(), false);
    }

    (token.to_owned(), false)
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("api-key")
        || lower == "key"
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("passwd")
        || lower.contains("authorization")
        || lower == "auth"
}

fn looks_like_secret(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.starts_with("sk-")
        || lower.starts_with("sk_")
        || lower.starts_with("ghp_")
        || lower.starts_with("github_pat_")
        || lower.starts_with("xoxb-")
        || lower.starts_with("xoxp-")
        || (token.starts_with("AKIA") && token.len() >= 16)
        || token.contains("-----BEGIN")
}

/// The bounded categories of durable memory a harness action may propose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Convention,
    ArchitectureDecision,
    Lesson,
}

/// Governance outcome for a decoded action, ordered from least to most
/// restrictive so a stricter policy can safely win a merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Ask,
    Deny,
}

/// A normalized result from a tool or validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackReport {
    pub source: String,
    pub validator_id: Option<String>,
    pub exit_code: Option<i32>,
    pub classification: String,
    pub summary: String,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub fingerprint: String,
    pub retryable: bool,
}

/// Typed terminal or pause reasons for a harness run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Succeeded,
    Failed,
    Cancelled,
    AwaitingApproval,
    BudgetExceeded,
    RepeatedFailure,
    InterruptedUnknownOutcome,
}

/// Structured output from a tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: ObservationId,
    pub call_id: CallId,
    pub kind: String,
    pub summary: String,
    pub data: serde_json::Value,
}

/// A durable, hash-bindable request for human approval.
#[derive(Debug, Clone, PartialEq)]
pub struct ApprovalRequest {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    pub action_id: ActionId,
    /// A short normalized summary. It must never contain a raw credential.
    pub action_summary: String,
    pub action_hash: String,
    pub workspace_identity: String,
    pub policy_snapshot_hash: String,
    pub config_snapshot_hash: String,
    pub risk: String,
    pub rule_id: String,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalRequestWire {
    approval_id: ApprovalId,
    run_id: RunId,
    action_id: ActionId,
    action_summary: String,
    action_hash: String,
    workspace_identity: String,
    policy_snapshot_hash: String,
    config_snapshot_hash: String,
    risk: String,
    rule_id: String,
    created_at: String,
    expires_at: String,
}

impl From<&ApprovalRequest> for ApprovalRequestWire {
    fn from(value: &ApprovalRequest) -> Self {
        Self {
            approval_id: value.approval_id.clone(),
            run_id: value.run_id.clone(),
            action_id: value.action_id.clone(),
            action_summary: value.action_summary.clone(),
            action_hash: value.action_hash.clone(),
            workspace_identity: value.workspace_identity.clone(),
            policy_snapshot_hash: value.policy_snapshot_hash.clone(),
            config_snapshot_hash: value.config_snapshot_hash.clone(),
            risk: value.risk.clone(),
            rule_id: value.rule_id.clone(),
            created_at: value.created_at.clone(),
            expires_at: value.expires_at.clone(),
        }
    }
}

impl From<ApprovalRequestWire> for ApprovalRequest {
    fn from(value: ApprovalRequestWire) -> Self {
        Self {
            approval_id: value.approval_id,
            run_id: value.run_id,
            action_id: value.action_id,
            action_summary: normalize_action_summary(&value.action_summary),
            action_hash: value.action_hash,
            workspace_identity: value.workspace_identity,
            policy_snapshot_hash: value.policy_snapshot_hash,
            config_snapshot_hash: value.config_snapshot_hash,
            risk: value.risk,
            rule_id: value.rule_id,
            created_at: value.created_at,
            expires_at: value.expires_at,
        }
    }
}

impl ApprovalRequest {
    /// Return a copy whose durable summary is normalized and redacted.
    pub fn sanitized(&self) -> Self {
        Self {
            action_summary: normalize_action_summary(&self.action_summary),
            ..self.clone()
        }
    }

    /// Build a request from a decoded action without copying its content into
    /// the approval record.
    pub fn action_summary_for(action: &AgentAction) -> String {
        action.action_summary()
    }

    pub fn validate(&self) -> Result<(), ProtocolValidationError> {
        if self.action_summary.trim().is_empty() {
            return Err(ProtocolValidationError::EmptyActionSummary);
        }
        if self.action_summary.chars().count() > MAX_ACTION_SUMMARY_CHARS {
            return Err(ProtocolValidationError::ActionSummaryTooLong);
        }
        let normalized = normalize_action_summary(&self.action_summary);
        if normalized != self.action_summary {
            return Err(ProtocolValidationError::ActionSummaryNotNormalized);
        }
        Ok(())
    }
}

impl Serialize for ApprovalRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let sanitized = self.sanitized();
        sanitized.validate().map_err(S::Error::custom)?;
        ApprovalRequestWire::from(&sanitized).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ApprovalRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let request = ApprovalRequest::from(ApprovalRequestWire::deserialize(deserializer)?);
        request.validate().map_err(D::Error::custom)?;
        Ok(request)
    }
}

/// The event payload variants emitted by the self-harness.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessEventKind {
    RunCreated,
    StepStarted,
    ModelStarted,
    ModelCompleted {
        assistant_text: String,
    },
    ActionRecorded {
        action_id: ActionId,
        action: AgentAction,
        origin_model_call_id: Option<CallId>,
    },
    PolicyDecided {
        action_id: ActionId,
        decision: PolicyDecision,
        rule_id: String,
    },
    ApprovalRequested {
        request: ApprovalRequest,
    },
    ApprovalResolved {
        approval_id: ApprovalId,
        decision: String,
    },
    ToolStarted {
        action_id: ActionId,
    },
    ToolCompleted {
        observation: Observation,
    },
    ToolFailed {
        feedback: FeedbackReport,
    },
    ValidatorCompleted {
        feedback: FeedbackReport,
    },
    RunPaused {
        reason: StopReason,
    },
    RunCompleted {
        reason: StopReason,
        summary: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KindWire {
    kind: String,
    payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCompletedPayload {
    assistant_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionRecordedPayload {
    action_id: ActionId,
    action: AgentAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin_model_call_id: Option<CallId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDecidedPayload {
    action_id: ActionId,
    decision: PolicyDecision,
    rule_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalRequestedPayload {
    request: ApprovalRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalResolvedPayload {
    approval_id: ApprovalId,
    decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolStartedPayload {
    action_id: ActionId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolCompletedPayload {
    observation: Observation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackPayload {
    feedback: FeedbackReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunPausedPayload {
    reason: StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunCompletedPayload {
    reason: StopReason,
    summary: String,
}

fn empty_payload(payload: &serde_json::Value) -> Result<(), String> {
    match payload.as_object() {
        Some(fields) if fields.is_empty() => Ok(()),
        _ => Err("unit harness event payload must be an empty object".into()),
    }
}

fn decode_payload<T: for<'de> Deserialize<'de>>(
    kind: &str,
    payload: serde_json::Value,
) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|_| format!("invalid payload for harness event {kind}"))
}

impl HarnessEventKind {
    fn from_kind_payload(kind: &str, payload: serde_json::Value) -> Result<Self, String> {
        match kind {
            "run.created" => {
                empty_payload(&payload)?;
                Ok(Self::RunCreated)
            }
            "step.started" => {
                empty_payload(&payload)?;
                Ok(Self::StepStarted)
            }
            "model.started" => {
                empty_payload(&payload)?;
                Ok(Self::ModelStarted)
            }
            "model.completed" => {
                let value: ModelCompletedPayload = decode_payload(kind, payload)?;
                Ok(Self::ModelCompleted {
                    assistant_text: value.assistant_text,
                })
            }
            "action.recorded" => {
                let value: ActionRecordedPayload = decode_payload(kind, payload)?;
                Ok(Self::ActionRecorded {
                    action_id: value.action_id,
                    action: value.action,
                    origin_model_call_id: value.origin_model_call_id,
                })
            }
            "policy.decided" => {
                let value: PolicyDecidedPayload = decode_payload(kind, payload)?;
                Ok(Self::PolicyDecided {
                    action_id: value.action_id,
                    decision: value.decision,
                    rule_id: value.rule_id,
                })
            }
            "approval.requested" => {
                let value: ApprovalRequestedPayload = decode_payload(kind, payload)?;
                Ok(Self::ApprovalRequested {
                    request: value.request,
                })
            }
            "approval.resolved" => {
                let value: ApprovalResolvedPayload = decode_payload(kind, payload)?;
                Ok(Self::ApprovalResolved {
                    approval_id: value.approval_id,
                    decision: value.decision,
                })
            }
            "tool.started" => {
                let value: ToolStartedPayload = decode_payload(kind, payload)?;
                Ok(Self::ToolStarted {
                    action_id: value.action_id,
                })
            }
            "tool.completed" => {
                let value: ToolCompletedPayload = decode_payload(kind, payload)?;
                Ok(Self::ToolCompleted {
                    observation: value.observation,
                })
            }
            "tool.failed" => {
                let value: FeedbackPayload = decode_payload(kind, payload)?;
                Ok(Self::ToolFailed {
                    feedback: value.feedback,
                })
            }
            "validator.completed" => {
                let value: FeedbackPayload = decode_payload(kind, payload)?;
                Ok(Self::ValidatorCompleted {
                    feedback: value.feedback,
                })
            }
            "run.paused" => {
                let value: RunPausedPayload = decode_payload(kind, payload)?;
                Ok(Self::RunPaused {
                    reason: value.reason,
                })
            }
            "run.completed" => {
                let value: RunCompletedPayload = decode_payload(kind, payload)?;
                Ok(Self::RunCompleted {
                    reason: value.reason,
                    summary: normalize_action_summary(&value.summary),
                })
            }
            _ => Err(format!("unknown harness event kind {kind}")),
        }
    }

    fn sanitized(&self) -> Self {
        match self {
            Self::ApprovalRequested { request } => Self::ApprovalRequested {
                request: request.sanitized(),
            },
            Self::RunCompleted { reason, summary } => Self::RunCompleted {
                reason: reason.clone(),
                summary: normalize_action_summary(summary),
            },
            other => other.clone(),
        }
    }

    /// Stable dotted name used by JSONL, SSE, and UI event dispatch.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::RunCreated => "run.created",
            Self::StepStarted => "step.started",
            Self::ModelStarted => "model.started",
            Self::ModelCompleted { .. } => "model.completed",
            Self::ActionRecorded { .. } => "action.recorded",
            Self::PolicyDecided { .. } => "policy.decided",
            Self::ApprovalRequested { .. } => "approval.requested",
            Self::ApprovalResolved { .. } => "approval.resolved",
            Self::ToolStarted { .. } => "tool.started",
            Self::ToolCompleted { .. } => "tool.completed",
            Self::ToolFailed { .. } => "tool.failed",
            Self::ValidatorCompleted { .. } => "validator.completed",
            Self::RunPaused { .. } => "run.paused",
            Self::RunCompleted { .. } => "run.completed",
        }
    }
}

/// Serialize the kind discriminator and its typed payload directly rather
/// than going through `serde_json::Value` (whose default map sorts keys). This
/// keeps fixtures byte-stable while still using the public typed DTOs.
fn serialize_kind_fields<T>(kind: &HarnessEventKind, state: &mut T) -> Result<(), T::Error>
where
    T: serde::ser::SerializeStruct,
{
    match kind {
        HarnessEventKind::RunCreated => {
            state.serialize_field("kind", "run.created")?;
            state.serialize_field("payload", &EmptyPayload {})?;
        }
        HarnessEventKind::StepStarted => {
            state.serialize_field("kind", "step.started")?;
            state.serialize_field("payload", &EmptyPayload {})?;
        }
        HarnessEventKind::ModelStarted => {
            state.serialize_field("kind", "model.started")?;
            state.serialize_field("payload", &EmptyPayload {})?;
        }
        HarnessEventKind::ModelCompleted { assistant_text } => {
            state.serialize_field("kind", "model.completed")?;
            state.serialize_field(
                "payload",
                &ModelCompletedPayload {
                    assistant_text: assistant_text.clone(),
                },
            )?;
        }
        HarnessEventKind::ActionRecorded {
            action_id,
            action,
            origin_model_call_id,
        } => {
            state.serialize_field("kind", "action.recorded")?;
            state.serialize_field(
                "payload",
                &ActionRecordedPayload {
                    action_id: action_id.clone(),
                    action: action.clone(),
                    origin_model_call_id: origin_model_call_id.clone(),
                },
            )?;
        }
        HarnessEventKind::PolicyDecided {
            action_id,
            decision,
            rule_id,
        } => {
            state.serialize_field("kind", "policy.decided")?;
            state.serialize_field(
                "payload",
                &PolicyDecidedPayload {
                    action_id: action_id.clone(),
                    decision: *decision,
                    rule_id: rule_id.clone(),
                },
            )?;
        }
        HarnessEventKind::ApprovalRequested { request } => {
            state.serialize_field("kind", "approval.requested")?;
            state.serialize_field(
                "payload",
                &ApprovalRequestedPayload {
                    request: request.sanitized(),
                },
            )?;
        }
        HarnessEventKind::ApprovalResolved {
            approval_id,
            decision,
        } => {
            state.serialize_field("kind", "approval.resolved")?;
            state.serialize_field(
                "payload",
                &ApprovalResolvedPayload {
                    approval_id: approval_id.clone(),
                    decision: decision.clone(),
                },
            )?;
        }
        HarnessEventKind::ToolStarted { action_id } => {
            state.serialize_field("kind", "tool.started")?;
            state.serialize_field(
                "payload",
                &ToolStartedPayload {
                    action_id: action_id.clone(),
                },
            )?;
        }
        HarnessEventKind::ToolCompleted { observation } => {
            state.serialize_field("kind", "tool.completed")?;
            state.serialize_field(
                "payload",
                &ToolCompletedPayload {
                    observation: observation.clone(),
                },
            )?;
        }
        HarnessEventKind::ToolFailed { feedback } => {
            state.serialize_field("kind", "tool.failed")?;
            state.serialize_field(
                "payload",
                &FeedbackPayload {
                    feedback: feedback.clone(),
                },
            )?;
        }
        HarnessEventKind::ValidatorCompleted { feedback } => {
            state.serialize_field("kind", "validator.completed")?;
            state.serialize_field(
                "payload",
                &FeedbackPayload {
                    feedback: feedback.clone(),
                },
            )?;
        }
        HarnessEventKind::RunPaused { reason } => {
            state.serialize_field("kind", "run.paused")?;
            state.serialize_field(
                "payload",
                &RunPausedPayload {
                    reason: reason.clone(),
                },
            )?;
        }
        HarnessEventKind::RunCompleted { reason, summary } => {
            state.serialize_field("kind", "run.completed")?;
            state.serialize_field(
                "payload",
                &RunCompletedPayload {
                    reason: reason.clone(),
                    summary: normalize_action_summary(summary),
                },
            )?;
        }
    }
    Ok(())
}

impl Serialize for HarnessEventKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("HarnessEventKind", 2)?;
        serialize_kind_fields(self, &mut state)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for HarnessEventKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = KindWire::deserialize(deserializer)?;
        Self::from_kind_payload(&wire.kind, wire.payload).map_err(D::Error::custom)
    }
}

/// Versioned envelope shared by persisted and streamed harness events.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessEvent {
    pub schema_version: u16,
    pub event_id: EventId,
    pub run_id: RunId,
    pub turn_id: Option<TurnId>,
    pub step_id: Option<StepId>,
    pub call_id: Option<CallId>,
    pub sequence: u64,
    pub occurred_at: String,
    pub kind: HarnessEventKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessEventWire {
    schema_version: u16,
    event_id: EventId,
    run_id: RunId,
    turn_id: Option<TurnId>,
    step_id: Option<StepId>,
    call_id: Option<CallId>,
    sequence: u64,
    occurred_at: String,
    kind: String,
    payload: serde_json::Value,
}

impl HarnessEvent {
    /// Build a deterministic envelope for protocol tests and fixtures.
    ///
    /// Runtime code will supply real timestamps and identifiers when it
    /// creates events; this helper intentionally has no clock dependency.
    pub fn new_for_test(
        event_id: EventId,
        run_id: RunId,
        step_id: StepId,
        sequence: u64,
        kind: HarnessEventKind,
    ) -> Self {
        Self {
            schema_version: HARNESS_SCHEMA_VERSION,
            event_id,
            run_id,
            turn_id: None,
            step_id: Some(step_id),
            call_id: None,
            sequence,
            occurred_at: "2026-07-10T00:00:00Z".into(),
            kind,
        }
    }

    /// Validate invariants which cannot be expressed by the JSON schema alone.
    pub fn validate(&self) -> Result<(), ProtocolValidationError> {
        if self.schema_version != HARNESS_SCHEMA_VERSION {
            return Err(ProtocolValidationError::UnsupportedSchemaVersion {
                found: self.schema_version,
                expected: HARNESS_SCHEMA_VERSION,
            });
        }
        if self.sequence == 0 {
            return Err(ProtocolValidationError::InvalidSequence {
                found: self.sequence,
            });
        }
        if let HarnessEventKind::ApprovalRequested { request } = &self.kind {
            request.validate()?;
            if request.run_id != self.run_id {
                return Err(ProtocolValidationError::ApprovalRunIdMismatch {
                    outer: self.run_id.clone(),
                    request: request.run_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Stable dotted name used by renderers without parsing serialized JSON.
    pub fn kind_name(&self) -> &'static str {
        self.kind.kind_name()
    }
}

impl Serialize for HarnessEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Sanitize display-only fields before validation and serialization so
        // a caller cannot accidentally persist a raw credential by constructing
        // a struct literal instead of using a builder.
        let kind = self.kind.sanitized();
        let sanitized = Self {
            kind,
            ..self.clone()
        };
        sanitized.validate().map_err(S::Error::custom)?;
        let mut state = serializer.serialize_struct("HarnessEvent", 10)?;
        state.serialize_field("schema_version", &sanitized.schema_version)?;
        state.serialize_field("event_id", &sanitized.event_id)?;
        state.serialize_field("run_id", &sanitized.run_id)?;
        state.serialize_field("turn_id", &sanitized.turn_id)?;
        state.serialize_field("step_id", &sanitized.step_id)?;
        state.serialize_field("call_id", &sanitized.call_id)?;
        state.serialize_field("sequence", &sanitized.sequence)?;
        state.serialize_field("occurred_at", &sanitized.occurred_at)?;
        serialize_kind_fields(&sanitized.kind, &mut state)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for HarnessEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = HarnessEventWire::deserialize(deserializer)?;
        let event = Self {
            schema_version: wire.schema_version,
            event_id: wire.event_id,
            run_id: wire.run_id,
            turn_id: wire.turn_id,
            step_id: wire.step_id,
            call_id: wire.call_id,
            sequence: wire.sequence,
            occurred_at: wire.occurred_at,
            kind: HarnessEventKind::from_kind_payload(&wire.kind, wire.payload)
                .map_err(D::Error::custom)?,
        };
        event.validate().map_err(D::Error::custom)?;
        Ok(event)
    }
}
