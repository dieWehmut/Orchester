//! Bounded provider-neutral transcript records for durable resume state.

use std::fmt;

use orchester_modell::{ModelItem, ModelMessage, ModelRole, ToolCall};
use orchester_protokoll::CallId;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::feedback::FeedbackEngine;

const MAX_IDENTIFIER_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptLimits {
    pub max_record_bytes: usize,
    pub max_text_bytes: usize,
    pub max_opaque_bytes: usize,
}

impl Default for TranscriptLimits {
    fn default() -> Self {
        Self {
            max_record_bytes: 64 * 1024,
            max_text_bytes: 32 * 1024,
            max_opaque_bytes: 16 * 1024,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum TranscriptRecord {
    System(String),
    User(String),
    Assistant(String),
    ToolCall {
        call_id: CallId,
        name: String,
        arguments_json: String,
    },
    ToolResult {
        call_id: CallId,
        output: String,
    },
    Opaque {
        digest: String,
        byte_len: usize,
    },
}

impl TranscriptRecord {
    pub fn system(text: impl Into<String>) -> Self {
        Self::System(text.into())
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self::User(text.into())
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::Assistant(text.into())
    }

    pub fn tool_call(
        call_id: impl Into<CallId>,
        name: impl Into<String>,
        arguments_json: impl Into<String>,
    ) -> Self {
        Self::ToolCall {
            call_id: call_id.into(),
            name: name.into(),
            arguments_json: arguments_json.into(),
        }
    }

    pub fn tool_result(call_id: impl Into<CallId>, output: impl Into<String>) -> Self {
        Self::ToolResult {
            call_id: call_id.into(),
            output: output.into(),
        }
    }

    pub fn opaque_json(value: &Value, codec: &TranscriptCodec) -> Result<Self, TranscriptError> {
        codec.opaque_reference(value)
    }

    pub(crate) fn byte_len(&self) -> usize {
        match self {
            Self::System(text) | Self::User(text) | Self::Assistant(text) => text.len(),
            Self::ToolCall {
                call_id,
                name,
                arguments_json,
            } => call_id.0.len() + name.len() + arguments_json.len(),
            Self::ToolResult { call_id, output } => call_id.0.len() + output.len(),
            Self::Opaque { digest, .. } => digest.len(),
        }
    }

    pub(crate) fn strings(&self) -> Vec<&str> {
        match self {
            Self::System(text) | Self::User(text) | Self::Assistant(text) => vec![text],
            Self::ToolCall {
                call_id,
                name,
                arguments_json,
            } => vec![&call_id.0, name, arguments_json],
            Self::ToolResult { call_id, output } => vec![&call_id.0, output],
            Self::Opaque { digest, .. } => vec![digest],
        }
    }

    pub(crate) fn to_message(&self) -> ModelMessage {
        match self {
            Self::System(text) => text_message(ModelRole::System, text.clone()),
            Self::User(text) => text_message(ModelRole::User, text.clone()),
            Self::Assistant(text) => text_message(ModelRole::Assistant, text.clone()),
            Self::ToolCall {
                call_id,
                name,
                arguments_json,
            } => ModelMessage {
                role: ModelRole::Assistant,
                items: vec![ModelItem::ToolCall(ToolCall::new(
                    call_id.clone(),
                    name.clone(),
                    arguments_json.clone(),
                ))],
            },
            Self::ToolResult { call_id, output } => ModelMessage {
                role: ModelRole::Tool,
                items: vec![ModelItem::ToolResult {
                    call_id: call_id.clone(),
                    output: output.clone(),
                }],
            },
            Self::Opaque { digest, byte_len } => ModelMessage {
                role: ModelRole::Assistant,
                items: vec![ModelItem::Opaque(json!({
                    "digest": digest,
                    "byte_len": byte_len,
                }))],
            },
        }
    }
}

impl fmt::Debug for TranscriptRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::System(_) => "System",
            Self::User(_) => "User",
            Self::Assistant(_) => "Assistant",
            Self::ToolCall { .. } => "ToolCall",
            Self::ToolResult { .. } => "ToolResult",
            Self::Opaque { .. } => "Opaque",
        };
        formatter
            .debug_struct("TranscriptRecord")
            .field("kind", &kind)
            .field("bytes", &self.byte_len())
            .finish()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptError {
    #[error("transcript limits are invalid")]
    InvalidLimits,
    #[error("transcript text exceeds its durable limit")]
    TextTooLarge,
    #[error("transcript record exceeds its durable limit")]
    RecordTooLarge,
    #[error("opaque provider item exceeds its durable limit")]
    OpaqueTooLarge,
    #[error("transcript identifier is not canonical")]
    InvalidIdentifier,
    #[error("transcript wire record is invalid")]
    InvalidWire,
    #[error("transcript wire record is not canonical")]
    NonCanonical,
    #[error("tool result has no matching call")]
    UnpairedToolResult,
    #[error("tool call is not followed by its result")]
    UnpairedToolCall,
}

pub struct TranscriptCodec {
    limits: TranscriptLimits,
    sanitizer: FeedbackEngine,
}

impl TranscriptCodec {
    pub fn new(limits: TranscriptLimits, secrets: Vec<SecretString>) -> Self {
        let sanitizer = secrets
            .into_iter()
            .fold(FeedbackEngine::default(), FeedbackEngine::with_secret);
        Self { limits, sanitizer }
    }

    pub fn encode(&self, record: &TranscriptRecord) -> Result<String, TranscriptError> {
        self.validate_limits()?;
        let canonical = self.canonicalize(record)?;
        let encoded = serde_json::to_string(&WireRecord::from(&canonical))
            .map_err(|_| TranscriptError::InvalidWire)?;
        if encoded.len() > self.limits.max_record_bytes {
            return Err(TranscriptError::RecordTooLarge);
        }
        Ok(encoded)
    }

    pub fn decode(&self, encoded: &str) -> Result<TranscriptRecord, TranscriptError> {
        self.validate_limits()?;
        if encoded.len() > self.limits.max_record_bytes {
            return Err(TranscriptError::RecordTooLarge);
        }
        let record = TranscriptRecord::from(
            serde_json::from_str::<WireRecord>(encoded)
                .map_err(|_| TranscriptError::InvalidWire)?,
        );
        let canonical = self.canonicalize(&record)?;
        if canonical != record {
            return Err(TranscriptError::NonCanonical);
        }
        let canonical_wire = serde_json::to_string(&WireRecord::from(&canonical))
            .map_err(|_| TranscriptError::InvalidWire)?;
        if canonical_wire != encoded {
            return Err(TranscriptError::NonCanonical);
        }
        Ok(record)
    }

    pub fn encode_all(&self, records: &[TranscriptRecord]) -> Result<Vec<String>, TranscriptError> {
        self.validate_sequence(records)?;
        records.iter().map(|record| self.encode(record)).collect()
    }

    pub fn decode_all(&self, encoded: &[String]) -> Result<Vec<TranscriptRecord>, TranscriptError> {
        let records = encoded
            .iter()
            .map(|record| self.decode(record))
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_sequence(&records)?;
        Ok(records)
    }

    pub fn validate_sequence(&self, records: &[TranscriptRecord]) -> Result<(), TranscriptError> {
        let mut pending_call: Option<&CallId> = None;
        for record in records {
            match record {
                TranscriptRecord::ToolCall { call_id, .. } => {
                    if pending_call.is_some() {
                        return Err(TranscriptError::UnpairedToolCall);
                    }
                    pending_call = Some(call_id);
                }
                TranscriptRecord::ToolResult { call_id, .. } => match pending_call.take() {
                    Some(pending) if pending == call_id => {}
                    _ => return Err(TranscriptError::UnpairedToolResult),
                },
                _ if pending_call.is_some() => return Err(TranscriptError::UnpairedToolCall),
                _ => {}
            }
        }
        Ok(())
    }

    fn opaque_reference(&self, value: &Value) -> Result<TranscriptRecord, TranscriptError> {
        self.validate_limits()?;
        let encoded = serde_json::to_vec(value).map_err(|_| TranscriptError::InvalidWire)?;
        if encoded.len() > self.limits.max_opaque_bytes {
            return Err(TranscriptError::OpaqueTooLarge);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"orchester-opaque-item-v1");
        hasher.update((encoded.len() as u64).to_be_bytes());
        hasher.update(&encoded);
        Ok(TranscriptRecord::Opaque {
            digest: hex(&hasher.finalize()),
            byte_len: encoded.len(),
        })
    }

    fn canonicalize(&self, record: &TranscriptRecord) -> Result<TranscriptRecord, TranscriptError> {
        match record {
            TranscriptRecord::System(text) => Ok(TranscriptRecord::System(self.text(text)?)),
            TranscriptRecord::User(text) => Ok(TranscriptRecord::User(self.text(text)?)),
            TranscriptRecord::Assistant(text) => Ok(TranscriptRecord::Assistant(self.text(text)?)),
            TranscriptRecord::ToolCall {
                call_id,
                name,
                arguments_json,
            } => Ok(TranscriptRecord::ToolCall {
                call_id: CallId::from(self.identifier(&call_id.0)?),
                name: self.identifier(name)?,
                arguments_json: self.arguments(arguments_json)?,
            }),
            TranscriptRecord::ToolResult { call_id, output } => Ok(TranscriptRecord::ToolResult {
                call_id: CallId::from(self.identifier(&call_id.0)?),
                output: self.text(output)?,
            }),
            TranscriptRecord::Opaque { digest, byte_len } => {
                if digest.len() != 64
                    || !digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    || *byte_len > self.limits.max_opaque_bytes
                {
                    return Err(TranscriptError::InvalidWire);
                }
                Ok(record.clone())
            }
        }
    }

    fn text(&self, value: &str) -> Result<String, TranscriptError> {
        let sanitized = self.sanitizer.sanitize_text(value);
        if sanitized.len() > self.limits.max_text_bytes {
            Err(TranscriptError::TextTooLarge)
        } else {
            Ok(sanitized)
        }
    }

    fn identifier(&self, value: &str) -> Result<String, TranscriptError> {
        if value.is_empty()
            || value.len() > MAX_IDENTIFIER_BYTES
            || self.sanitizer.sanitize_text(value) != value
        {
            Err(TranscriptError::InvalidIdentifier)
        } else {
            Ok(value.to_owned())
        }
    }

    fn arguments(&self, value: &str) -> Result<String, TranscriptError> {
        let parsed =
            serde_json::from_str::<Value>(value).map_err(|_| TranscriptError::InvalidWire)?;
        let sanitized = sanitize_json_value(parsed.clone(), &self.sanitizer);
        if sanitized != parsed {
            return Err(TranscriptError::InvalidWire);
        }
        serde_json::to_string(&parsed).map_err(|_| TranscriptError::InvalidWire)
    }

    fn validate_limits(&self) -> Result<(), TranscriptError> {
        if self.limits.max_record_bytes == 0
            || self.limits.max_text_bytes == 0
            || self.limits.max_opaque_bytes == 0
            || self.limits.max_text_bytes > self.limits.max_record_bytes
        {
            Err(TranscriptError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

fn sanitize_json_value(value: Value, sanitizer: &FeedbackEngine) -> Value {
    match value {
        Value::String(text) => Value::String(sanitizer.sanitize_text(&text)),
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| sanitize_json_value(value, sanitizer))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| {
                    (
                        sanitizer.sanitize_text(&key),
                        sanitize_json_value(value, sanitizer),
                    )
                })
                .collect(),
        ),
        other => other,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum WireRecord {
    System {
        text: String,
    },
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        arguments_json: String,
    },
    ToolResult {
        call_id: String,
        output: String,
    },
    Opaque {
        digest: String,
        byte_len: usize,
    },
}

impl From<&TranscriptRecord> for WireRecord {
    fn from(record: &TranscriptRecord) -> Self {
        match record {
            TranscriptRecord::System(text) => Self::System { text: text.clone() },
            TranscriptRecord::User(text) => Self::User { text: text.clone() },
            TranscriptRecord::Assistant(text) => Self::Assistant { text: text.clone() },
            TranscriptRecord::ToolCall {
                call_id,
                name,
                arguments_json,
            } => Self::ToolCall {
                call_id: call_id.0.clone(),
                name: name.clone(),
                arguments_json: arguments_json.clone(),
            },
            TranscriptRecord::ToolResult { call_id, output } => Self::ToolResult {
                call_id: call_id.0.clone(),
                output: output.clone(),
            },
            TranscriptRecord::Opaque { digest, byte_len } => Self::Opaque {
                digest: digest.clone(),
                byte_len: *byte_len,
            },
        }
    }
}

impl From<WireRecord> for TranscriptRecord {
    fn from(record: WireRecord) -> Self {
        match record {
            WireRecord::System { text } => Self::System(text),
            WireRecord::User { text } => Self::User(text),
            WireRecord::Assistant { text } => Self::Assistant(text),
            WireRecord::ToolCall {
                call_id,
                name,
                arguments_json,
            } => Self::ToolCall {
                call_id: CallId::from(call_id),
                name,
                arguments_json,
            },
            WireRecord::ToolResult { call_id, output } => Self::ToolResult {
                call_id: CallId::from(call_id),
                output,
            },
            WireRecord::Opaque { digest, byte_len } => Self::Opaque { digest, byte_len },
        }
    }
}

fn text_message(role: ModelRole, text: String) -> ModelMessage {
    ModelMessage {
        role,
        items: vec![ModelItem::Text(text)],
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
