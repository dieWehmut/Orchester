use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use orchester_protokoll::{Capability, Event, Task};

use crate::error::AdapterError;

/// A stream of normalized [`Event`]s produced by a running agent.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, AdapterError>> + Send>>;

/// The core interface Orchester uses to drive any agent.
///
/// Implementors spawn the underlying agent (usually a subprocess), read its native
/// output, and yield a stream of vendor-neutral [`Event`]s. Orchester never assumes
/// anything about *how* the agent works — only that it honours this contract.
#[async_trait]
pub trait AgentAdapter: Send + Sync {
    /// Stable adapter name, matched by the CLI `--agent <name>`.
    fn name(&self) -> &str;

    /// What this adapter advertises (feeds `orchester list` and the future planner).
    fn capabilities(&self) -> Capability;

    /// Spawn the agent for `task` and return a stream of normalized events.
    async fn run(&self, task: Task) -> Result<EventStream, AdapterError>;
}
