//! Durable-API approval state machine and one-shot capabilities.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use orchester_protokoll::{ActionId, ApprovalId, ApprovalRequest, EventId, RunId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::harness::run_store::SqliteRunStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalState {
    Requested,
    Awaiting,
    Approved,
    Denied,
    Expired,
    Invalidated,
    Executing,
    Consumed,
}

impl ApprovalState {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Awaiting => "awaiting",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::Invalidated => "invalidated",
            Self::Executing => "executing",
            Self::Consumed => "consumed",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, ApprovalError> {
        match value {
            "requested" => Ok(Self::Requested),
            "awaiting" => Ok(Self::Awaiting),
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            "expired" => Ok(Self::Expired),
            "invalidated" => Ok(Self::Invalidated),
            "executing" => Ok(Self::Executing),
            "consumed" => Ok(Self::Consumed),
            _ => Err(ApprovalError::Storage),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalBinding {
    pub run_id: RunId,
    pub action_id: ActionId,
    pub action_hash: String,
    pub workspace_identity: String,
    pub policy_snapshot_hash: String,
    pub config_snapshot_hash: String,
}

impl ApprovalBinding {
    pub fn test(action: &str, workspace: &str, policy: &str, config: &str) -> Self {
        Self {
            run_id: RunId::from("run-1"),
            action_id: ActionId::from("action-1"),
            action_hash: action.into(),
            workspace_identity: workspace.into(),
            policy_snapshot_hash: policy.into(),
            config_snapshot_hash: config.into(),
        }
    }

    pub(crate) fn matches_request(&self, request: &ApprovalRequest) -> bool {
        self.run_id == request.run_id
            && self.action_id == request.action_id
            && self.action_hash == request.action_hash
            && self.workspace_identity == request.workspace_identity
            && self.policy_snapshot_hash == request.policy_snapshot_hash
            && self.config_snapshot_hash == request.config_snapshot_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequestInput {
    pub approval_id: ApprovalId,
    pub owner_actor_id: String,
    pub binding: ApprovalBinding,
    pub action_summary: String,
    pub risk: String,
    pub rule_id: String,
    pub created_at: String,
    pub expires_at: String,
    pub created_at_unix: u64,
    pub expires_at_unix: u64,
}

impl ApprovalRequestInput {
    pub(crate) fn protocol_request(&self) -> ApprovalRequest {
        ApprovalRequest {
            approval_id: self.approval_id.clone(),
            run_id: self.binding.run_id.clone(),
            action_id: self.binding.action_id.clone(),
            action_summary: self.action_summary.clone(),
            action_hash: self.binding.action_hash.clone(),
            workspace_identity: self.binding.workspace_identity.clone(),
            policy_snapshot_hash: self.binding.policy_snapshot_hash.clone(),
            config_snapshot_hash: self.binding.config_snapshot_hash.clone(),
            risk: self.risk.clone(),
            rule_id: self.rule_id.clone(),
            // The integer deadlines are the authority. Keep the wire strings
            // derived from them so display/audit timestamps cannot disagree
            // with the expiry checks performed by the store.
            created_at: format!("unix:{}", self.created_at_unix),
            expires_at: format!("unix:{}", self.expires_at_unix),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalSnapshot {
    pub approval_id: ApprovalId,
    pub run_id: RunId,
    pub action_id: ActionId,
    pub owner_actor_id: String,
    pub state: ApprovalState,
    pub binding: ApprovalBinding,
    pub action_summary: String,
    pub risk: String,
    pub rule_id: String,
    pub created_at_unix: u64,
    pub expires_at_unix: u64,
    pub approval_event_id: Option<EventId>,
    pub row_version: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewApproval {
    pub request: ApprovalRequest,
    pub owner_actor_id: String,
    pub expires_at_unix: u64,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("approval was not found")]
    NotFound,
    #[error("approval request is invalid")]
    InvalidRequest,
    #[error("approval already exists")]
    Duplicate,
    #[error("approval operation is not authorized")]
    Unauthorized,
    #[error("approval is in an invalid state")]
    InvalidState,
    #[error("approval has expired")]
    Expired,
    #[error("approval binding no longer matches")]
    BindingMismatch,
    #[error("approval capability is invalid")]
    InvalidCapability,
    #[error("secure capability nonce generation is unavailable")]
    EntropyUnavailable,
    #[error("approval store lock is poisoned")]
    LockPoisoned,
    #[error("approval persistence failed")]
    Storage,
    #[error("approval requires a durable audit checkpoint")]
    AuditCheckpointRequired,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityToken {
    approval_id: ApprovalId,
    owner_actor_id: String,
    binding: ApprovalBinding,
    expires_at_unix: u64,
    nonce: [u8; 32],
}

impl fmt::Debug for CapabilityToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CapabilityToken([REDACTED])")
    }
}

#[derive(Debug, Clone)]
struct ApprovalRecord {
    request: ApprovalRequest,
    owner_actor_id: String,
    expires_at_unix: u64,
    state: ApprovalState,
    capability_hash: Option<[u8; 32]>,
}

pub struct ApprovalStore {
    records: RwLock<HashMap<ApprovalId, ApprovalRecord>>,
    next_nonce: AtomicU64,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

/// Deterministic clock handle for offline state-machine tests and demos.
#[derive(Clone)]
pub struct TestClock {
    value: Arc<AtomicU64>,
}

impl TestClock {
    pub fn set(&self, now_unix: u64) {
        self.value.store(now_unix, Ordering::Relaxed);
    }
}

impl Default for ApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalStore {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(unix_now))
    }

    pub fn with_fixed_time(now_unix: u64) -> Self {
        Self::with_clock(Arc::new(move || now_unix))
    }

    pub fn with_test_clock(now_unix: u64) -> (Self, TestClock) {
        let value = Arc::new(AtomicU64::new(now_unix));
        let clock_value = Arc::clone(&value);
        (
            Self::with_clock(Arc::new(move || clock_value.load(Ordering::Relaxed))),
            TestClock { value },
        )
    }

    fn with_clock(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            next_nonce: AtomicU64::new(1),
            clock,
        }
    }

    pub fn request(&self, input: NewApproval) -> Result<ApprovalId, ApprovalError> {
        input
            .request
            .validate()
            .map_err(|_| ApprovalError::InvalidRequest)?;
        if input.owner_actor_id.trim().is_empty() || input.expires_at_unix == 0 {
            return Err(ApprovalError::InvalidRequest);
        }
        if input.expires_at_unix <= (self.clock)() {
            return Err(ApprovalError::Expired);
        }
        let id = input.request.approval_id.clone();
        let mut records = self.write()?;
        if records.contains_key(&id) {
            return Err(ApprovalError::Duplicate);
        }
        records.insert(
            id.clone(),
            ApprovalRecord {
                request: input.request,
                owner_actor_id: input.owner_actor_id,
                expires_at_unix: input.expires_at_unix,
                state: ApprovalState::Awaiting,
                capability_hash: None,
            },
        );
        Ok(id)
    }

    pub fn state(&self, approval_id: &ApprovalId) -> Result<ApprovalState, ApprovalError> {
        self.read()?
            .get(approval_id)
            .map(|record| record.state)
            .ok_or(ApprovalError::NotFound)
    }

    pub fn approve(
        &self,
        approval_id: &ApprovalId,
        actor_id: &str,
        binding: &ApprovalBinding,
    ) -> Result<CapabilityToken, ApprovalError> {
        let now_unix = (self.clock)();
        let mut records = self.write()?;
        let record = records
            .get_mut(approval_id)
            .ok_or(ApprovalError::NotFound)?;
        authorize(record, actor_id)?;
        expire_if_needed(record, now_unix)?;
        if record.state != ApprovalState::Awaiting {
            return Err(ApprovalError::InvalidState);
        }
        if !binding.matches_request(&record.request) {
            record.state = ApprovalState::Invalidated;
            return Err(ApprovalError::BindingMismatch);
        }

        let counter = self.next_nonce.fetch_add(1, Ordering::Relaxed);
        let nonce = create_nonce(approval_id, actor_id, now_unix, counter)?;
        record.capability_hash = Some(hash_bytes(&nonce));
        record.state = ApprovalState::Approved;
        Ok(CapabilityToken {
            approval_id: approval_id.clone(),
            owner_actor_id: actor_id.into(),
            binding: binding.clone(),
            expires_at_unix: record.expires_at_unix,
            nonce,
        })
    }

    pub fn deny(&self, approval_id: &ApprovalId, actor_id: &str) -> Result<(), ApprovalError> {
        let now_unix = (self.clock)();
        let mut records = self.write()?;
        let record = records
            .get_mut(approval_id)
            .ok_or(ApprovalError::NotFound)?;
        authorize(record, actor_id)?;
        expire_if_needed(record, now_unix)?;
        if record.state != ApprovalState::Awaiting {
            return Err(ApprovalError::InvalidState);
        }
        record.state = ApprovalState::Denied;
        Ok(())
    }

    pub fn consume(
        &self,
        capability: &CapabilityToken,
        actor_id: &str,
        binding: &ApprovalBinding,
    ) -> Result<(), ApprovalError> {
        let now_unix = (self.clock)();
        let mut records = self.write()?;
        let record = records
            .get_mut(&capability.approval_id)
            .ok_or(ApprovalError::NotFound)?;
        authorize(record, actor_id)?;
        expire_if_needed(record, now_unix)?;
        if record.state != ApprovalState::Approved {
            return Err(ApprovalError::InvalidState);
        }
        if capability.owner_actor_id != actor_id
            || capability.expires_at_unix != record.expires_at_unix
            || capability.binding != *binding
            || !binding.matches_request(&record.request)
        {
            record.state = ApprovalState::Invalidated;
            record.capability_hash = None;
            return Err(ApprovalError::BindingMismatch);
        }
        let expected = record
            .capability_hash
            .ok_or(ApprovalError::InvalidCapability)?;
        if expected != hash_bytes(&capability.nonce) {
            return Err(ApprovalError::InvalidCapability);
        }
        record.state = ApprovalState::Consumed;
        record.capability_hash = None;
        Ok(())
    }

    pub fn invalidate(
        &self,
        approval_id: &ApprovalId,
        actor_id: &str,
    ) -> Result<(), ApprovalError> {
        let mut records = self.write()?;
        let record = records
            .get_mut(approval_id)
            .ok_or(ApprovalError::NotFound)?;
        authorize(record, actor_id)?;
        if matches!(
            record.state,
            ApprovalState::Consumed | ApprovalState::Denied
        ) {
            return Err(ApprovalError::InvalidState);
        }
        record.state = ApprovalState::Invalidated;
        record.capability_hash = None;
        Ok(())
    }

    pub fn test_request(id: &str, owner: &str, expires_at_unix: u64) -> NewApproval {
        NewApproval {
            request: ApprovalRequest {
                approval_id: ApprovalId::from(id),
                run_id: RunId::from("run-1"),
                action_id: ActionId::from("action-1"),
                action_summary: "run_command program_bytes=5 args_count=1 args_bytes=4 cwd_bytes=0"
                    .into(),
                action_hash: "action-hash".into(),
                workspace_identity: "workspace".into(),
                policy_snapshot_hash: "policy".into(),
                config_snapshot_hash: "config".into(),
                risk: "high".into(),
                rule_id: "command.external_effect".into(),
                created_at: "2026-07-12T00:00:00Z".into(),
                expires_at: "2026-07-12T00:01:40Z".into(),
            },
            owner_actor_id: owner.into(),
            expires_at_unix,
        }
    }

    fn read(
        &self,
    ) -> Result<RwLockReadGuard<'_, HashMap<ApprovalId, ApprovalRecord>>, ApprovalError> {
        self.records.read().map_err(|_| ApprovalError::LockPoisoned)
    }

    fn write(
        &self,
    ) -> Result<RwLockWriteGuard<'_, HashMap<ApprovalId, ApprovalRecord>>, ApprovalError> {
        self.records
            .write()
            .map_err(|_| ApprovalError::LockPoisoned)
    }
}

/// SQLite-backed approval service.  Unlike [`ApprovalStore`], this type never
/// keeps approval state in a process-local map; every transition is a CAS in
/// `SqliteRunStore` and remains visible after reopen.
pub struct DurableApprovalStore {
    store: Arc<SqliteRunStore>,
    next_nonce: AtomicU64,
}

impl DurableApprovalStore {
    pub fn new(store: Arc<SqliteRunStore>) -> Self {
        Self {
            store,
            next_nonce: AtomicU64::new(1),
        }
    }

    pub fn store(&self) -> &Arc<SqliteRunStore> {
        &self.store
    }

    pub fn request(&self, input: ApprovalRequestInput) -> Result<ApprovalId, ApprovalError> {
        input
            .protocol_request()
            .validate()
            .map_err(|_| ApprovalError::InvalidRequest)?;
        let now_unix = unix_now();
        if input.owner_actor_id.trim().is_empty()
            || input.created_at_unix >= input.expires_at_unix
            || input.created_at_unix > now_unix
            || input.expires_at_unix <= now_unix
        {
            return Err(ApprovalError::InvalidRequest);
        }
        self.store
            .persist_approval_request(&input)
            .map_err(map_store_error)?;
        Ok(input.approval_id)
    }

    pub fn state(
        &self,
        approval_id: &ApprovalId,
        owner_actor_id: &str,
    ) -> Result<ApprovalState, ApprovalError> {
        self.store
            .load_approval(approval_id, owner_actor_id)
            .map(|snapshot| snapshot.state)
            .map_err(map_store_error)
    }

    pub fn snapshot(
        &self,
        approval_id: &ApprovalId,
        owner_actor_id: &str,
    ) -> Result<ApprovalSnapshot, ApprovalError> {
        self.store
            .load_approval(approval_id, owner_actor_id)
            .map_err(map_store_error)
    }

    pub fn approve(
        &self,
        approval_id: &ApprovalId,
        actor_id: &str,
        binding: &ApprovalBinding,
    ) -> Result<CapabilityToken, ApprovalError> {
        let now_unix = unix_now();
        let counter = self.next_nonce.fetch_add(1, Ordering::Relaxed);
        let nonce = create_nonce(approval_id, actor_id, now_unix, counter)?;
        let capability_hash = hex_hash(&hash_bytes(&nonce));
        let snapshot = self
            .store
            .approve_approval(approval_id, actor_id, binding, capability_hash, now_unix)
            .map_err(map_store_error)?;
        Ok(CapabilityToken {
            approval_id: approval_id.clone(),
            owner_actor_id: actor_id.to_owned(),
            binding: binding.clone(),
            expires_at_unix: snapshot.expires_at_unix,
            nonce,
        })
    }

    /// Replace a capability lost after the approval transaction committed.
    /// Reissue atomically revokes the old nonce hash and preserves the human
    /// decision; only the approval owner with the exact binding can do it.
    pub fn reissue(
        &self,
        approval_id: &ApprovalId,
        actor_id: &str,
        binding: &ApprovalBinding,
    ) -> Result<CapabilityToken, ApprovalError> {
        let now_unix = unix_now();
        let counter = self.next_nonce.fetch_add(1, Ordering::Relaxed);
        let nonce = create_nonce(approval_id, actor_id, now_unix, counter)?;
        let capability_hash = hex_hash(&hash_bytes(&nonce));
        let snapshot = self
            .store
            .reissue_approval(approval_id, actor_id, binding, capability_hash, now_unix)
            .map_err(map_store_error)?;
        Ok(CapabilityToken {
            approval_id: approval_id.clone(),
            owner_actor_id: actor_id.to_owned(),
            binding: binding.clone(),
            expires_at_unix: snapshot.expires_at_unix,
            nonce,
        })
    }

    pub fn deny(&self, approval_id: &ApprovalId, actor_id: &str) -> Result<(), ApprovalError> {
        self.store
            .deny_approval(approval_id, actor_id, unix_now())
            .map(|_| ())
            .map_err(map_store_error)
    }

    pub fn consume(
        &self,
        capability: &CapabilityToken,
        actor_id: &str,
        binding: &ApprovalBinding,
    ) -> Result<(), ApprovalError> {
        let now_unix = unix_now();
        capability.validate_for(actor_id, binding, now_unix)?;
        self.store
            .consume_approval(
                &capability.approval_id,
                actor_id,
                binding,
                hex_hash(&hash_bytes(&capability.nonce)),
                now_unix,
            )
            .map(|_| ())
            .map_err(map_store_error)
    }
}

impl CapabilityToken {
    pub fn approval_id(&self) -> &ApprovalId {
        &self.approval_id
    }

    pub fn binding(&self) -> &ApprovalBinding {
        &self.binding
    }

    pub(crate) fn nonce_hash(&self) -> String {
        hex_hash(&hash_bytes(&self.nonce))
    }

    pub(crate) fn validate_for(
        &self,
        actor_id: &str,
        binding: &ApprovalBinding,
        now_unix: u64,
    ) -> Result<(), ApprovalError> {
        if self.owner_actor_id != actor_id {
            return Err(ApprovalError::Unauthorized);
        }
        if self.binding != *binding {
            return Err(ApprovalError::BindingMismatch);
        }
        if self.expires_at_unix <= now_unix {
            return Err(ApprovalError::Expired);
        }
        Ok(())
    }
}

fn map_store_error(error: crate::harness::run_store::StoreError) -> ApprovalError {
    use crate::harness::run_store::StoreError;
    match error {
        StoreError::NotFound | StoreError::ApprovalNotFound => ApprovalError::NotFound,
        StoreError::ApprovalUnauthorized => ApprovalError::Unauthorized,
        StoreError::ApprovalExpired => ApprovalError::Expired,
        StoreError::ApprovalBindingMismatch => ApprovalError::BindingMismatch,
        StoreError::ApprovalInvalidState => ApprovalError::InvalidState,
        StoreError::ApprovalAuditRequired => ApprovalError::AuditCheckpointRequired,
        StoreError::ApprovalNonceMismatch => ApprovalError::InvalidCapability,
        _ => ApprovalError::Storage,
    }
}

fn authorize(record: &ApprovalRecord, actor_id: &str) -> Result<(), ApprovalError> {
    if record.owner_actor_id == actor_id {
        Ok(())
    } else {
        Err(ApprovalError::Unauthorized)
    }
}

fn expire_if_needed(record: &mut ApprovalRecord, now_unix: u64) -> Result<(), ApprovalError> {
    if now_unix >= record.expires_at_unix {
        record.state = ApprovalState::Expired;
        record.capability_hash = None;
        Err(ApprovalError::Expired)
    } else {
        Ok(())
    }
}

fn create_nonce(
    id: &ApprovalId,
    actor: &str,
    now: u64,
    counter: u64,
) -> Result<[u8; 32], ApprovalError> {
    let mut nonce = [0u8; 32];
    getrandom::fill(&mut nonce).map_err(|_| ApprovalError::EntropyUnavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-approval-capability-v1");
    hasher.update(nonce);
    hasher.update(id.0.as_bytes());
    hasher.update(actor.as_bytes());
    hasher.update(now.to_le_bytes());
    hasher.update(counter.to_le_bytes());
    Ok(hasher.finalize().into())
}

fn hash_bytes(value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(value);
    hasher.finalize().into()
}

fn hex_hash(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
