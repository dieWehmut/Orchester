//! The pre-execution audit barrier.
//!
//! A tool is not authorized by an in-memory policy result.  The barrier first
//! derives the candidate from SQLite, appends and fsyncs the matching audit
//! entry, records that checkpoint transactionally, and only then returns the
//! private permit consumed by a future tool runtime.

use std::sync::Arc;

use orchester_protokoll::{ActionId, EventId, HarnessEvent, RunId};
use thiserror::Error;

use super::approval::{ApprovalBinding, ApprovalError, CapabilityToken};
use super::audit::{AuditError, AuditReceipt, AuditSink};
use super::run_store::EventAppend;
use super::run_store::{SqliteRunStore, StoreError};

#[derive(Debug, Error)]
pub enum BarrierError {
    #[error("execution candidate is unavailable")]
    Candidate(#[source] StoreError),
    #[error("audit append or fsync failed")]
    AuditUnavailable(#[source] AuditError),
    #[error("audit checkpoint could not be committed")]
    Checkpoint(#[source] StoreError),
    #[error("approval authorization failed")]
    Approval(#[source] ApprovalError),
    #[error("approval is required for this action")]
    ApprovalRequired,
    #[error("approval does not match the execution candidate")]
    ApprovalMismatch,
}

pub enum ExecutionAuthorization<'a> {
    Allow,
    Approval {
        capability: &'a CapabilityToken,
        binding: &'a ApprovalBinding,
    },
}

/// An unforgeable proof that a particular action/event has a durable synced
/// audit checkpoint.  Fields are private and the type is intentionally not
/// `Clone`; a future runtime must consume the value exactly once.
#[derive(Debug)]
pub struct ExecutionPermit {
    action_id: ActionId,
    event_id: EventId,
    receipt: AuditReceipt,
}

impl ExecutionPermit {
    pub fn action_id(&self) -> &ActionId {
        &self.action_id
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn audit_sequence(&self) -> u64 {
        self.receipt.sequence()
    }

    pub fn audit_head_hash(&self) -> &str {
        self.receipt.head_hash()
    }
}

pub struct PreExecutionBarrier<S> {
    store: Arc<SqliteRunStore>,
    audit: Arc<S>,
}

impl<S: AuditSink> PreExecutionBarrier<S> {
    pub fn new(store: Arc<SqliteRunStore>, audit: Arc<S>) -> Self {
        Self { store, audit }
    }

    pub fn prepare(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        authorization: ExecutionAuthorization<'_>,
        occurred_at: impl Into<String>,
    ) -> Result<ExecutionPermit, BarrierError> {
        self.prepare_internal(
            owner_actor_id,
            run_id,
            action_id,
            authorization,
            occurred_at,
            unix_now(),
        )
    }

    fn prepare_internal(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        action_id: &ActionId,
        authorization: ExecutionAuthorization<'_>,
        occurred_at: impl Into<String>,
        now_unix: u64,
    ) -> Result<ExecutionPermit, BarrierError> {
        let candidate = self
            .store
            .execution_candidate(owner_actor_id, run_id, action_id)
            .map_err(BarrierError::Candidate)?;
        match &authorization {
            ExecutionAuthorization::Allow if candidate.approval_id().is_some() => {
                return Err(BarrierError::ApprovalRequired)
            }
            ExecutionAuthorization::Approval {
                capability,
                binding,
            } => {
                capability
                    .validate_for(owner_actor_id, binding, now_unix)
                    .map_err(BarrierError::Approval)?;
                if candidate.approval_id() != Some(capability.approval_id().0.as_str())
                    || capability.binding() != *binding
                {
                    return Err(BarrierError::ApprovalMismatch);
                }
            }
            ExecutionAuthorization::Allow => {}
        }
        let receipt = self
            .audit
            .append_and_sync(candidate.audit_input(occurred_at))
            .map_err(BarrierError::AuditUnavailable)?;
        self.store
            .mark_execution_checkpoint(owner_actor_id, &candidate, &receipt)
            .map_err(BarrierError::Checkpoint)?;
        if let ExecutionAuthorization::Approval {
            capability,
            binding,
        } = authorization
        {
            let consumed = self.store.consume_approval(
                capability.approval_id(),
                owner_actor_id,
                binding,
                capability.nonce_hash(),
                now_unix,
            );
            if matches!(consumed, Err(StoreError::ApprovalInvalidState)) {
                self.store
                    .recover_execution_approval(
                        capability.approval_id(),
                        owner_actor_id,
                        binding,
                        capability.nonce_hash(),
                        now_unix,
                    )
                    .map_err(|error| match error {
                        StoreError::ApprovalAuditRequired => {
                            BarrierError::Approval(ApprovalError::AuditCheckpointRequired)
                        }
                        other => BarrierError::Approval(map_approval_store_error(other)),
                    })?;
            } else {
                consumed.map_err(|error| match error {
                    StoreError::ApprovalAuditRequired => {
                        BarrierError::Approval(ApprovalError::AuditCheckpointRequired)
                    }
                    other => BarrierError::Approval(map_approval_store_error(other)),
                })?;
            }
        }
        Ok(ExecutionPermit {
            action_id: candidate.action_id().clone(),
            event_id: candidate.event_id().clone(),
            receipt,
        })
    }

    /// Consume the permit at the tool-start boundary.  The run store rejects
    /// all public `append_event(ToolStarted)` calls that do not come through
    /// this method.
    pub fn start_tool(
        &self,
        owner_actor_id: &str,
        run_id: &RunId,
        permit: ExecutionPermit,
        input: EventAppend,
    ) -> Result<HarnessEvent, BarrierError> {
        self.store
            .append_tool_started_with_permit(owner_actor_id, run_id, permit, input)
            .map_err(BarrierError::Checkpoint)
    }
}

fn map_approval_store_error(error: StoreError) -> ApprovalError {
    match error {
        StoreError::NotFound | StoreError::ApprovalNotFound => ApprovalError::NotFound,
        StoreError::ApprovalUnauthorized => ApprovalError::Unauthorized,
        StoreError::ApprovalExpired => ApprovalError::Expired,
        StoreError::ApprovalBindingMismatch => ApprovalError::BindingMismatch,
        StoreError::ApprovalInvalidState => ApprovalError::InvalidState,
        StoreError::ApprovalNonceMismatch => ApprovalError::InvalidCapability,
        StoreError::ApprovalAuditRequired => ApprovalError::AuditCheckpointRequired,
        _ => ApprovalError::Storage,
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
