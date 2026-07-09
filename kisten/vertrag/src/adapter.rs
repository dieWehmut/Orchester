use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use orchester_protokoll::{Capability, Event, Task};

use crate::error::AdapterError;

/// A stream of normalized [`Event`]s produced by a running agent.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<Event, AdapterError>> + Send>>;

/// Local availability of an adapter before attempting a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterAvailability {
    pub name: String,
    pub status: AvailabilityStatus,
    pub detail: String,
}

impl AdapterAvailability {
    pub fn available(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: AvailabilityStatus::Available,
            detail: detail.into(),
        }
    }

    pub fn missing(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: AvailabilityStatus::Missing,
            detail: detail.into(),
        }
    }

    pub fn unknown(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: AvailabilityStatus::Unknown,
            detail: detail.into(),
        }
    }

    pub fn is_missing(&self) -> bool {
        self.status == AvailabilityStatus::Missing
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityStatus {
    Available,
    Missing,
    Unknown,
}

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

    /// Quick local check for whether this adapter can likely be launched.
    fn availability(&self) -> AdapterAvailability {
        AdapterAvailability::unknown(self.name(), "availability check not implemented")
    }

    /// Native interactive command, if this adapter wraps a subprocess CLI.
    ///
    /// Orchester's non-interactive mode uses [`Self::run`] so it can normalize a
    /// JSON event stream. The interactive launcher uses this command to hand the
    /// terminal to the underlying agent exactly as if the user had typed it.
    fn native_command(&self) -> Option<&str> {
        None
    }

    /// Spawn the agent for `task` and return a stream of normalized events.
    async fn run(&self, task: Task) -> Result<EventStream, AdapterError>;
}
