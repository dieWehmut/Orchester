use crate::types::ToolCall;
use orchester_protokoll::{AgentAction, CallId, MemoryKind};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::error::Error;
use std::fmt;

/// Maximum raw JSON accepted for one tool call before parsing.
pub const MAX_ARGUMENTS_JSON_BYTES: usize = 1024 * 1024;
/// Maximum byte length of a workspace-relative path or command working directory.
pub const MAX_PATH_BYTES: usize = 4096;
/// Maximum byte length of a search query, approval reason, or recall query.
pub const MAX_QUERY_BYTES: usize = 16 * 1024;
/// Maximum byte length of file content, patches, memory, and final summaries.
pub const MAX_CONTENT_BYTES: usize = 512 * 1024;
/// Maximum byte length of a command program, argument, or validator identifier.
pub const MAX_COMMAND_PART_BYTES: usize = 16 * 1024;
/// Maximum number of command arguments or validator identifiers in one action.
pub const MAX_LIST_ITEMS: usize = 128;
/// Maximum byte length of a model/provider call identifier.
pub const MAX_CALL_ID_BYTES: usize = 256;
/// Maximum directory depth accepted by a `list_files` action.
pub const MAX_LIST_DEPTH: u16 = 16;
/// Maximum number of memory entries returned by a `recall` action.
pub const MAX_RECALL_LIMIT: u16 = 100;

/// Stable, non-secret names for tools understood by the decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    ListFiles,
    SearchText,
    ReadFile,
    WriteFile,
    ApplyPatch,
    RunCommand,
    RunChecks,
    Remember,
    Recall,
    RequestApproval,
    Finish,
}

impl ToolKind {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "list_files" => Some(Self::ListFiles),
            "search_text" => Some(Self::SearchText),
            "read_file" => Some(Self::ReadFile),
            "write_file" => Some(Self::WriteFile),
            "apply_patch" => Some(Self::ApplyPatch),
            "run_command" => Some(Self::RunCommand),
            "run_checks" => Some(Self::RunChecks),
            "remember" => Some(Self::Remember),
            "recall" => Some(Self::Recall),
            "request_approval" => Some(Self::RequestApproval),
            "finish" => Some(Self::Finish),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ListFiles => "list_files",
            Self::SearchText => "search_text",
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::ApplyPatch => "apply_patch",
            Self::RunCommand => "run_command",
            Self::RunChecks => "run_checks",
            Self::Remember => "remember",
            Self::Recall => "recall",
            Self::RequestApproval => "request_approval",
            Self::Finish => "finish",
        }
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for ToolKind {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Stable field labels used by safe decoder diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentField {
    Path,
    Query,
    Content,
    Patch,
    Program,
    CommandArguments,
    CommandArgument,
    WorkingDirectory,
    CheckIds,
    CheckId,
    Reason,
    Summary,
}

impl ArgumentField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Content => "content",
            Self::Patch => "patch",
            Self::Program => "program",
            Self::CommandArguments => "args",
            Self::CommandArgument => "arg",
            Self::WorkingDirectory => "cwd",
            Self::CheckIds => "ids",
            Self::CheckId => "id",
            Self::Reason => "reason",
            Self::Summary => "summary",
        }
    }
}

impl fmt::Display for ArgumentField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Safe, structured failures from model tool-call decoding.
///
/// Every variant that correlates to a call carries a typed [`CallId`], but
/// neither `Display` nor `Debug` renders that provider-controlled identifier.
#[derive(Clone, PartialEq, Eq)]
pub enum DecodeError {
    UnknownTool {
        call_id: CallId,
    },
    InvalidCallId {
        call_id: CallId,
        actual_bytes: usize,
        max_bytes: usize,
    },
    ArgumentsTooLarge {
        call_id: CallId,
        tool: ToolKind,
        actual_bytes: usize,
        max_bytes: usize,
    },
    InvalidArguments {
        call_id: CallId,
        tool: ToolKind,
    },
    InvalidRange {
        call_id: CallId,
        tool: ToolKind,
    },
    EmptyProgram {
        call_id: CallId,
        tool: ToolKind,
    },
    EmptyField {
        call_id: CallId,
        tool: ToolKind,
        field: ArgumentField,
    },
    InvalidField {
        call_id: CallId,
        tool: ToolKind,
        field: ArgumentField,
    },
    FieldTooLarge {
        call_id: CallId,
        tool: ToolKind,
        field: ArgumentField,
        actual_bytes: usize,
        max_bytes: usize,
    },
    TooManyItems {
        call_id: CallId,
        tool: ToolKind,
        field: ArgumentField,
        actual: usize,
        max: usize,
    },
    InvalidDepth {
        call_id: CallId,
        tool: ToolKind,
        depth: u16,
        max: u16,
    },
    InvalidLimit {
        call_id: CallId,
        tool: ToolKind,
        limit: u16,
        max: u16,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTool { .. } => formatter.write_str("unknown model tool"),
            Self::InvalidCallId {
                actual_bytes,
                max_bytes,
                ..
            } => write!(
                formatter,
                "invalid model call identifier ({actual_bytes} bytes; maximum {max_bytes})"
            ),
            Self::ArgumentsTooLarge {
                tool,
                actual_bytes,
                max_bytes,
                ..
            } => write!(
                formatter,
                "arguments for {tool} are too large ({actual_bytes} bytes; maximum {max_bytes})"
            ),
            Self::InvalidArguments { tool, .. } => {
                write!(formatter, "invalid arguments for {tool}")
            }
            Self::InvalidRange { tool, .. } => write!(formatter, "invalid line range for {tool}"),
            Self::EmptyProgram { tool, .. } => {
                write!(formatter, "empty program field for {tool}")
            }
            Self::EmptyField { tool, field, .. } => {
                write!(formatter, "empty {field} field for {tool}")
            }
            Self::InvalidField { tool, field, .. } => {
                write!(formatter, "invalid {field} field for {tool}")
            }
            Self::FieldTooLarge {
                tool,
                field,
                actual_bytes,
                max_bytes,
                ..
            } => write!(
                formatter,
                "{field} field for {tool} is too large ({actual_bytes} bytes; maximum {max_bytes})"
            ),
            Self::TooManyItems {
                tool,
                field,
                actual,
                max,
                ..
            } => write!(
                formatter,
                "too many {field} items for {tool} ({actual}; maximum {max})"
            ),
            Self::InvalidDepth {
                tool, depth, max, ..
            } => write!(formatter, "depth {depth} is outside 1..={max} for {tool}"),
            Self::InvalidLimit {
                tool, limit, max, ..
            } => write!(formatter, "limit {limit} is outside 1..={max} for {tool}"),
        }
    }
}

impl fmt::Debug for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "DecodeError({self})")
    }
}

impl Error for DecodeError {}

/// Strict decoder from provider tool calls to protocol actions.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActionDecoder;

impl ActionDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, call: &ToolCall) -> Result<AgentAction, DecodeError> {
        let Some(tool) = ToolKind::from_name(&call.name) else {
            return Err(DecodeError::UnknownTool {
                call_id: call.call_id.clone(),
            });
        };
        validate_call_envelope(call, tool)?;

        match tool {
            ToolKind::ListFiles => {
                let args: ListFilesArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Path,
                    &args.path,
                    MAX_PATH_BYTES,
                )?;
                validate_control_free(call, tool, ArgumentField::Path, &args.path)?;
                if args.depth == 0 || args.depth > MAX_LIST_DEPTH {
                    return Err(DecodeError::InvalidDepth {
                        call_id: call.call_id.clone(),
                        tool,
                        depth: args.depth,
                        max: MAX_LIST_DEPTH,
                    });
                }
                Ok(AgentAction::ListFiles {
                    path: args.path,
                    depth: args.depth,
                })
            }
            ToolKind::SearchText => {
                let args: SearchTextArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Path,
                    &args.path,
                    MAX_PATH_BYTES,
                )?;
                validate_control_free(call, tool, ArgumentField::Path, &args.path)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Query,
                    &args.query,
                    MAX_QUERY_BYTES,
                )?;
                Ok(AgentAction::SearchText {
                    path: args.path,
                    query: args.query,
                })
            }
            ToolKind::ReadFile => {
                let args: ReadFileArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Path,
                    &args.path,
                    MAX_PATH_BYTES,
                )?;
                validate_control_free(call, tool, ArgumentField::Path, &args.path)?;
                if args.start_line == Some(0)
                    || args.end_line == Some(0)
                    || matches!((args.start_line, args.end_line), (Some(start), Some(end)) if start > end)
                {
                    return Err(DecodeError::InvalidRange {
                        call_id: call.call_id.clone(),
                        tool,
                    });
                }
                Ok(AgentAction::ReadFile {
                    path: args.path,
                    start_line: args.start_line,
                    end_line: args.end_line,
                })
            }
            ToolKind::WriteFile => {
                let args: WriteFileArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Path,
                    &args.path,
                    MAX_PATH_BYTES,
                )?;
                validate_control_free(call, tool, ArgumentField::Path, &args.path)?;
                validate_bounded_field(
                    call,
                    tool,
                    ArgumentField::Content,
                    &args.content,
                    MAX_CONTENT_BYTES,
                )?;
                Ok(AgentAction::WriteFile {
                    path: args.path,
                    content: args.content,
                })
            }
            ToolKind::ApplyPatch => {
                let args: ApplyPatchArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Patch,
                    &args.patch,
                    MAX_CONTENT_BYTES,
                )?;
                Ok(AgentAction::ApplyPatch { patch: args.patch })
            }
            ToolKind::RunCommand => {
                let args: RunCommandArgs = decode_args(call, tool)?;
                validate_bounded_field(
                    call,
                    tool,
                    ArgumentField::Program,
                    &args.program,
                    MAX_COMMAND_PART_BYTES,
                )?;
                if args.program.trim().is_empty() {
                    return Err(DecodeError::EmptyProgram {
                        call_id: call.call_id.clone(),
                        tool,
                    });
                }
                validate_control_free(call, tool, ArgumentField::Program, &args.program)?;
                validate_list(
                    call,
                    tool,
                    ArgumentField::CommandArguments,
                    args.args.len(),
                    MAX_LIST_ITEMS,
                    false,
                )?;
                for argument in &args.args {
                    validate_bounded_field(
                        call,
                        tool,
                        ArgumentField::CommandArgument,
                        argument,
                        MAX_COMMAND_PART_BYTES,
                    )?;
                }
                if let Some(cwd) = &args.cwd {
                    validate_required_field(
                        call,
                        tool,
                        ArgumentField::WorkingDirectory,
                        cwd,
                        MAX_PATH_BYTES,
                    )?;
                    validate_control_free(call, tool, ArgumentField::WorkingDirectory, cwd)?;
                }
                Ok(AgentAction::RunCommand {
                    program: args.program,
                    args: args.args,
                    cwd: args.cwd,
                })
            }
            ToolKind::RunChecks => {
                let args: RunChecksArgs = decode_args(call, tool)?;
                validate_list(
                    call,
                    tool,
                    ArgumentField::CheckIds,
                    args.ids.len(),
                    MAX_LIST_ITEMS,
                    true,
                )?;
                for id in &args.ids {
                    validate_required_field(
                        call,
                        tool,
                        ArgumentField::CheckId,
                        id,
                        MAX_COMMAND_PART_BYTES,
                    )?;
                    validate_control_free(call, tool, ArgumentField::CheckId, id)?;
                }
                Ok(AgentAction::RunChecks { ids: args.ids })
            }
            ToolKind::Remember => {
                let args: RememberArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Content,
                    &args.content,
                    MAX_CONTENT_BYTES,
                )?;
                Ok(AgentAction::Remember {
                    kind: args.kind,
                    content: args.content,
                })
            }
            ToolKind::Recall => {
                let args: RecallArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Query,
                    &args.query,
                    MAX_QUERY_BYTES,
                )?;
                if args.limit == 0 || args.limit > MAX_RECALL_LIMIT {
                    return Err(DecodeError::InvalidLimit {
                        call_id: call.call_id.clone(),
                        tool,
                        limit: args.limit,
                        max: MAX_RECALL_LIMIT,
                    });
                }
                Ok(AgentAction::Recall {
                    query: args.query,
                    limit: args.limit,
                })
            }
            ToolKind::RequestApproval => {
                let args: RequestApprovalArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Reason,
                    &args.reason,
                    MAX_QUERY_BYTES,
                )?;
                Ok(AgentAction::RequestApproval {
                    reason: args.reason,
                })
            }
            ToolKind::Finish => {
                let args: FinishArgs = decode_args(call, tool)?;
                validate_required_field(
                    call,
                    tool,
                    ArgumentField::Summary,
                    &args.summary,
                    MAX_CONTENT_BYTES,
                )?;
                Ok(AgentAction::Finish {
                    summary: args.summary,
                })
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListFilesArgs {
    path: String,
    depth: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchTextArgs {
    path: String,
    query: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadFileArgs {
    path: String,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyPatchArgs {
    patch: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunCommandArgs {
    program: String,
    args: Vec<String>,
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunChecksArgs {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RememberArgs {
    kind: MemoryKind,
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecallArgs {
    query: String,
    limit: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestApprovalArgs {
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinishArgs {
    summary: String,
}

fn validate_call_envelope(call: &ToolCall, tool: ToolKind) -> Result<(), DecodeError> {
    let call_id = &call.call_id.0;
    if call_id.is_empty()
        || call_id.len() > MAX_CALL_ID_BYTES
        || call_id.chars().any(char::is_control)
    {
        return Err(DecodeError::InvalidCallId {
            call_id: call.call_id.clone(),
            actual_bytes: call_id.len(),
            max_bytes: MAX_CALL_ID_BYTES,
        });
    }
    if call.arguments_json.len() > MAX_ARGUMENTS_JSON_BYTES {
        return Err(DecodeError::ArgumentsTooLarge {
            call_id: call.call_id.clone(),
            tool,
            actual_bytes: call.arguments_json.len(),
            max_bytes: MAX_ARGUMENTS_JSON_BYTES,
        });
    }
    Ok(())
}

fn decode_args<T: DeserializeOwned>(call: &ToolCall, tool: ToolKind) -> Result<T, DecodeError> {
    serde_json::from_str(&call.arguments_json).map_err(|_| DecodeError::InvalidArguments {
        call_id: call.call_id.clone(),
        tool,
    })
}

fn validate_bounded_field(
    call: &ToolCall,
    tool: ToolKind,
    field: ArgumentField,
    value: &str,
    max_bytes: usize,
) -> Result<(), DecodeError> {
    if value.len() > max_bytes {
        return Err(DecodeError::FieldTooLarge {
            call_id: call.call_id.clone(),
            tool,
            field,
            actual_bytes: value.len(),
            max_bytes,
        });
    }
    Ok(())
}

fn validate_required_field(
    call: &ToolCall,
    tool: ToolKind,
    field: ArgumentField,
    value: &str,
    max_bytes: usize,
) -> Result<(), DecodeError> {
    validate_bounded_field(call, tool, field, value, max_bytes)?;
    if value.trim().is_empty() {
        return Err(DecodeError::EmptyField {
            call_id: call.call_id.clone(),
            tool,
            field,
        });
    }
    Ok(())
}

fn validate_control_free(
    call: &ToolCall,
    tool: ToolKind,
    field: ArgumentField,
    value: &str,
) -> Result<(), DecodeError> {
    if value.chars().any(char::is_control) {
        return Err(DecodeError::InvalidField {
            call_id: call.call_id.clone(),
            tool,
            field,
        });
    }
    Ok(())
}

fn validate_list(
    call: &ToolCall,
    tool: ToolKind,
    field: ArgumentField,
    actual: usize,
    max: usize,
    require_non_empty: bool,
) -> Result<(), DecodeError> {
    if actual > max {
        return Err(DecodeError::TooManyItems {
            call_id: call.call_id.clone(),
            tool,
            field,
            actual,
            max,
        });
    }
    if require_non_empty && actual == 0 {
        return Err(DecodeError::EmptyField {
            call_id: call.call_id.clone(),
            tool,
            field,
        });
    }
    Ok(())
}
