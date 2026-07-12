use crate::types::ToolCall;
use orchester_protokoll::{AgentAction, MemoryKind};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use thiserror::Error;

/// Maximum directory depth accepted by a `list_files` action.
pub const MAX_LIST_DEPTH: u16 = 16;
/// Maximum number of memory entries returned by a `recall` action.
pub const MAX_RECALL_LIMIT: u16 = 100;

/// Safe, structured failures from model tool-call decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    #[error("unknown tool `{tool}` in call `{call_id}`")]
    UnknownTool { call_id: String, tool: String },
    #[error("invalid arguments for tool `{tool}` in call `{call_id}`")]
    InvalidArguments { call_id: String, tool: String },
    #[error("invalid line range for tool `{tool}` in call `{call_id}`")]
    InvalidRange { call_id: String, tool: String },
    #[error("empty program for tool `{tool}` in call `{call_id}`")]
    EmptyProgram { call_id: String, tool: String },
    #[error("depth {depth} is outside 1..={max} for tool `{tool}` in call `{call_id}`")]
    InvalidDepth {
        call_id: String,
        tool: String,
        depth: u16,
        max: u16,
    },
    #[error("limit {limit} is outside 1..={max} for tool `{tool}` in call `{call_id}`")]
    InvalidLimit {
        call_id: String,
        tool: String,
        limit: u16,
        max: u16,
    },
}

/// Strict decoder from provider tool calls to protocol actions.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActionDecoder;

impl ActionDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode(&self, call: &ToolCall) -> Result<AgentAction, DecodeError> {
        match call.name.as_str() {
            "list_files" => {
                let args: ListFilesArgs = decode_args(call)?;
                if args.depth == 0 || args.depth > MAX_LIST_DEPTH {
                    return Err(DecodeError::InvalidDepth {
                        call_id: call.call_id.clone(),
                        tool: call.name.clone(),
                        depth: args.depth,
                        max: MAX_LIST_DEPTH,
                    });
                }
                Ok(AgentAction::ListFiles {
                    path: args.path,
                    depth: args.depth,
                })
            }
            "search_text" => {
                let args: SearchTextArgs = decode_args(call)?;
                Ok(AgentAction::SearchText {
                    path: args.path,
                    query: args.query,
                })
            }
            "read_file" => {
                let args: ReadFileArgs = decode_args(call)?;
                if args.start_line == Some(0)
                    || args.end_line == Some(0)
                    || matches!((args.start_line, args.end_line), (Some(start), Some(end)) if start > end)
                {
                    return Err(DecodeError::InvalidRange {
                        call_id: call.call_id.clone(),
                        tool: call.name.clone(),
                    });
                }
                Ok(AgentAction::ReadFile {
                    path: args.path,
                    start_line: args.start_line,
                    end_line: args.end_line,
                })
            }
            "write_file" => {
                let args: WriteFileArgs = decode_args(call)?;
                Ok(AgentAction::WriteFile {
                    path: args.path,
                    content: args.content,
                })
            }
            "apply_patch" => {
                let args: ApplyPatchArgs = decode_args(call)?;
                Ok(AgentAction::ApplyPatch { patch: args.patch })
            }
            "run_command" => {
                let args: RunCommandArgs = decode_args(call)?;
                if args.program.trim().is_empty() {
                    return Err(DecodeError::EmptyProgram {
                        call_id: call.call_id.clone(),
                        tool: call.name.clone(),
                    });
                }
                Ok(AgentAction::RunCommand {
                    program: args.program,
                    args: args.args,
                    cwd: args.cwd,
                })
            }
            "run_checks" => {
                let args: RunChecksArgs = decode_args(call)?;
                Ok(AgentAction::RunChecks { ids: args.ids })
            }
            "remember" => {
                let args: RememberArgs = decode_args(call)?;
                Ok(AgentAction::Remember {
                    kind: args.kind,
                    content: args.content,
                })
            }
            "recall" => {
                let args: RecallArgs = decode_args(call)?;
                if args.limit == 0 || args.limit > MAX_RECALL_LIMIT {
                    return Err(DecodeError::InvalidLimit {
                        call_id: call.call_id.clone(),
                        tool: call.name.clone(),
                        limit: args.limit,
                        max: MAX_RECALL_LIMIT,
                    });
                }
                Ok(AgentAction::Recall {
                    query: args.query,
                    limit: args.limit,
                })
            }
            "request_approval" => {
                let args: RequestApprovalArgs = decode_args(call)?;
                Ok(AgentAction::RequestApproval {
                    reason: args.reason,
                })
            }
            "finish" => {
                let args: FinishArgs = decode_args(call)?;
                Ok(AgentAction::Finish {
                    summary: args.summary,
                })
            }
            _ => Err(DecodeError::UnknownTool {
                call_id: call.call_id.clone(),
                tool: call.name.clone(),
            }),
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

fn decode_args<T: DeserializeOwned>(call: &ToolCall) -> Result<T, DecodeError> {
    serde_json::from_str(&call.arguments_json).map_err(|_| DecodeError::InvalidArguments {
        call_id: call.call_id.clone(),
        tool: call.name.clone(),
    })
}
