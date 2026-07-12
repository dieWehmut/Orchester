//! # Orchester Protocol
//!
//! The **unified protocol** is Orchester's moat. Every heterogeneous coding agent
//! (Claude Code, Codex CLI, OpenCode, …) is normalized into the vendor-neutral types
//! defined here: a [`Task`] goes in, a stream of [`Event`]s comes out, and a
//! [`RunResult`] summarizes the run.
//!
//! Design rules:
//! - Every public type derives `Serialize, Deserialize, Debug, Clone`.
//! - [`Event`] is `#[serde(tag = "type")]` so the event stream is itself clean JSONL —
//!   Orchester's output is valid input to another Orchester (it can be orchestrated too).
//! - This crate depends on nothing else in the workspace. Everything depends on it.

mod capability;
mod event;
mod harness;
mod result;
mod session;
mod task;

pub use capability::{Capability, TaskKind};
pub use event::{ChangeKind, Event, TodoItem, ToolStatus};
pub use harness::*;
pub use result::{Outcome, RunResult, Usage};
pub use session::SessionState;
pub use task::Task;
