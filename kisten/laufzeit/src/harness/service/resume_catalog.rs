//! Safe projection of durable self-agent runs for the `/resume` surface.
//!
//! The run store keeps internal identifiers, prompts, paths, and provider
//! evidence.  This module deliberately projects only a bounded list of opaque
//! handles and continuation classifications.  Selection and continuation
//! must re-check ownership and workspace scope before touching a run.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use orchester_protokoll::RunId;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::WorkspaceIdentitySnapshot;
use crate::harness::config::{ConfigError, UserConfig};
use crate::harness::credentials::CredentialStore;
use crate::harness::run_store::{ResumeNext, ResumePoint, ResumeStage, SqliteRunStore, StoreError};

const MAX_RESUME_ENTRIES: usize = 100;
const HANDLE_PREFIX: &str = "r-";

/// A bounded, terminal-safe view of resumable runs in one workspace.
#[derive(Clone, PartialEq, Eq)]
pub struct SelfAgentResumeCatalog {
    pub database_present: bool,
    pub truncated: bool,
    pub entries: Vec<SelfAgentResumeEntry>,
}

impl SelfAgentResumeCatalog {
    pub const MAX_ENTRIES: usize = MAX_RESUME_ENTRIES;
}

impl fmt::Debug for SelfAgentResumeCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SelfAgentResumeCatalog")
            .field("database_present", &self.database_present)
            .field("truncated", &self.truncated)
            .field("entries", &self.entries)
            .finish()
    }
}

/// One user-selectable resume target.  The internal run ID is intentionally
/// absent; it is resolved again from the owner/workspace-scoped store later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentResumeEntry {
    pub handle: String,
    pub availability: SelfAgentResumeAvailability,
    pub step: SelfAgentResumeStep,
    pub latest: bool,
}

/// Whether the coordinator can inspect this point without an explicit human
/// decision or an external-outcome reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfAgentResumeAvailability {
    Ready,
    ApprovalRequired,
    ReconciliationRequired,
}

/// Redacted continuation stage suitable for a CLI list or picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfAgentResumeStep {
    StartStep,
    StartModel,
    ProcessModelOutput,
    EvaluatePolicy,
    PrepareExecution,
    StartNextStep,
    ContinueValidation,
    CreateApprovalRequest,
    AwaitApproval,
    RecoverApprovalCapability,
    ReconcileModelCall,
    ReconcileToolOutcome,
    ManualReconciliation(SelfAgentResumeStage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfAgentResumeStage {
    MissingStep,
    ModelCall,
    ToolOutcome,
    UnboundApproval,
    Unknown,
}

#[derive(Debug, Error)]
pub enum SelfAgentResumeCatalogError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Identity(#[from] super::IdentityError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("self-agent resume catalog path inspection failed")]
    Io(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum SelfAgentResumeTargetError {
    #[error("selected self-agent run is not available in this workspace")]
    Unavailable,
    #[error("selected self-agent run requires approval before it can continue")]
    ApprovalRequired,
    #[error("selected self-agent run requires manual reconciliation before it can continue")]
    ReconciliationRequired,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Load resumable points for the current owner and workspace without creating
/// a state database when one does not already exist.
pub fn load_self_agent_resume_catalog<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<SelfAgentResumeCatalog, SelfAgentResumeCatalogError> {
    let owner_actor_id = owner_actor_id.into();
    let workspace =
        WorkspaceIdentitySnapshot::for_workspace(workspace_root, owner_actor_id.clone())?;
    let state_database = state_database.as_ref();
    if !state_database
        .try_exists()
        .map_err(SelfAgentResumeCatalogError::Io)?
    {
        return Ok(SelfAgentResumeCatalog {
            database_present: false,
            truncated: false,
            entries: Vec::new(),
        });
    }

    let secrets = config.resolve_configured_secrets(credentials)?;
    let store = SqliteRunStore::open_with_terminal_secrets(state_database, secrets.values)?;
    let points = store.resume_points_owned_newest_first(
        &owner_actor_id,
        &workspace.project_id,
        MAX_RESUME_ENTRIES + 1,
    )?;
    project_resume_points(&workspace.project_id, points)
}

/// Resolve one public resume handle inside the exact store, owner, and
/// workspace that will perform the continuation. Hidden or stale handles are
/// deliberately indistinguishable from foreign handles.
pub fn resolve_self_agent_resume_handle(
    store: &SqliteRunStore,
    workspace: &WorkspaceIdentitySnapshot,
    handle: &str,
) -> Result<RunId, SelfAgentResumeTargetError> {
    if !valid_handle(handle) {
        return Err(SelfAgentResumeTargetError::Unavailable);
    }
    let mut points = store.resume_points_owned_newest_first(
        &workspace.owner_actor_id,
        &workspace.project_id,
        MAX_RESUME_ENTRIES + 1,
    )?;
    points.truncate(MAX_RESUME_ENTRIES);
    let point = points
        .into_iter()
        .find(|point| opaque_handle(&workspace.project_id, &point.run_id) == handle)
        .ok_or(SelfAgentResumeTargetError::Unavailable)?;
    match classify(&point.next).0 {
        SelfAgentResumeAvailability::Ready => Ok(point.run_id),
        SelfAgentResumeAvailability::ApprovalRequired => {
            Err(SelfAgentResumeTargetError::ApprovalRequired)
        }
        SelfAgentResumeAvailability::ReconciliationRequired => {
            Err(SelfAgentResumeTargetError::ReconciliationRequired)
        }
    }
}

fn project_resume_points(
    project_id: &str,
    mut points: Vec<ResumePoint>,
) -> Result<SelfAgentResumeCatalog, SelfAgentResumeCatalogError> {
    let truncated = points.len() > MAX_RESUME_ENTRIES;
    points.truncate(MAX_RESUME_ENTRIES);
    let mut handles = HashSet::with_capacity(points.len());
    let entries = points
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            let handle = opaque_handle(project_id, &point.run_id);
            if !handles.insert(handle.clone()) {
                return Err(SelfAgentResumeCatalogError::Store(StoreError::Invariant(
                    "resume handle collision".into(),
                )));
            }
            let (availability, step) = classify(&point.next);
            Ok(SelfAgentResumeEntry {
                handle,
                availability,
                step,
                latest: index == 0,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SelfAgentResumeCatalog {
        database_present: true,
        truncated,
        entries,
    })
}

fn classify(next: &ResumeNext) -> (SelfAgentResumeAvailability, SelfAgentResumeStep) {
    match next {
        ResumeNext::StartStep => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::StartStep,
        ),
        ResumeNext::StartModel { .. } => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::StartModel,
        ),
        ResumeNext::ProcessModelOutput { .. } => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::ProcessModelOutput,
        ),
        ResumeNext::EvaluatePolicy { .. } => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::EvaluatePolicy,
        ),
        ResumeNext::PrepareExecution { .. } => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::PrepareExecution,
        ),
        ResumeNext::StartNextStep => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::StartNextStep,
        ),
        ResumeNext::ContinueValidation { .. } => (
            SelfAgentResumeAvailability::Ready,
            SelfAgentResumeStep::ContinueValidation,
        ),
        ResumeNext::CreateApprovalRequest { .. } => (
            SelfAgentResumeAvailability::ApprovalRequired,
            SelfAgentResumeStep::CreateApprovalRequest,
        ),
        ResumeNext::AwaitApproval { .. } => (
            SelfAgentResumeAvailability::ApprovalRequired,
            SelfAgentResumeStep::AwaitApproval,
        ),
        ResumeNext::RecoverApprovalCapability { .. } => (
            SelfAgentResumeAvailability::ApprovalRequired,
            SelfAgentResumeStep::RecoverApprovalCapability,
        ),
        ResumeNext::ReconcileModelCall { .. } => (
            SelfAgentResumeAvailability::ReconciliationRequired,
            SelfAgentResumeStep::ReconcileModelCall,
        ),
        ResumeNext::ReconcileToolOutcome { .. } => (
            SelfAgentResumeAvailability::ReconciliationRequired,
            SelfAgentResumeStep::ReconcileToolOutcome,
        ),
        ResumeNext::ManualReconciliation { stage } => (
            SelfAgentResumeAvailability::ReconciliationRequired,
            SelfAgentResumeStep::ManualReconciliation(stage_from_store(*stage)),
        ),
    }
}

fn stage_from_store(stage: ResumeStage) -> SelfAgentResumeStage {
    match stage {
        ResumeStage::MissingStep => SelfAgentResumeStage::MissingStep,
        ResumeStage::ModelCall => SelfAgentResumeStage::ModelCall,
        ResumeStage::ToolOutcome => SelfAgentResumeStage::ToolOutcome,
        ResumeStage::UnboundApproval => SelfAgentResumeStage::UnboundApproval,
        ResumeStage::Unknown => SelfAgentResumeStage::Unknown,
    }
}

fn opaque_handle(project_id: &str, run_id: &RunId) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-resume-handle-v1\0");
    hash_field(&mut hasher, project_id.as_bytes());
    hash_field(&mut hasher, run_id.0.as_bytes());
    let digest = hasher.finalize();
    let encoded = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{HANDLE_PREFIX}{encoded}")
}

fn valid_handle(handle: &str) -> bool {
    handle.len() == HANDLE_PREFIX.len() + 32
        && handle.starts_with(HANDLE_PREFIX)
        && handle[HANDLE_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::run_store::RunStatus;
    use orchester_protokoll::{ActionId, ApprovalId, CallId, StepId};

    #[test]
    fn classifies_every_durable_continuation_without_exposing_identifiers() {
        let cases = [
            (
                ResumeNext::StartStep,
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::StartStep,
            ),
            (
                ResumeNext::StartModel {
                    step_id: StepId::from("step"),
                },
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::StartModel,
            ),
            (
                ResumeNext::ProcessModelOutput {
                    call_id: CallId::from("call"),
                },
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::ProcessModelOutput,
            ),
            (
                ResumeNext::EvaluatePolicy {
                    action_id: ActionId::from("action"),
                },
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::EvaluatePolicy,
            ),
            (
                ResumeNext::PrepareExecution {
                    action_id: ActionId::from("action"),
                    call_id: CallId::from("call"),
                },
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::PrepareExecution,
            ),
            (
                ResumeNext::StartNextStep,
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::StartNextStep,
            ),
            (
                ResumeNext::ContinueValidation {
                    step_id: None,
                    mutation_generation: 1,
                },
                SelfAgentResumeAvailability::Ready,
                SelfAgentResumeStep::ContinueValidation,
            ),
            (
                ResumeNext::CreateApprovalRequest {
                    action_id: ActionId::from("action"),
                },
                SelfAgentResumeAvailability::ApprovalRequired,
                SelfAgentResumeStep::CreateApprovalRequest,
            ),
            (
                ResumeNext::AwaitApproval {
                    approval_id: ApprovalId::from("approval"),
                },
                SelfAgentResumeAvailability::ApprovalRequired,
                SelfAgentResumeStep::AwaitApproval,
            ),
            (
                ResumeNext::RecoverApprovalCapability {
                    approval_id: ApprovalId::from("approval"),
                    action_id: ActionId::from("action"),
                },
                SelfAgentResumeAvailability::ApprovalRequired,
                SelfAgentResumeStep::RecoverApprovalCapability,
            ),
            (
                ResumeNext::ReconcileModelCall {
                    call_id: CallId::from("call"),
                },
                SelfAgentResumeAvailability::ReconciliationRequired,
                SelfAgentResumeStep::ReconcileModelCall,
            ),
            (
                ResumeNext::ReconcileToolOutcome {
                    action_id: ActionId::from("action"),
                    call_id: CallId::from("call"),
                },
                SelfAgentResumeAvailability::ReconciliationRequired,
                SelfAgentResumeStep::ReconcileToolOutcome,
            ),
            (
                ResumeNext::ManualReconciliation {
                    stage: ResumeStage::ModelCall,
                },
                SelfAgentResumeAvailability::ReconciliationRequired,
                SelfAgentResumeStep::ManualReconciliation(SelfAgentResumeStage::ModelCall),
            ),
        ];

        for (next, availability, step) in cases {
            assert_eq!(classify(&next), (availability, step));
            let rendered = format!("{step:?}");
            assert!(!rendered.contains("action"));
            assert!(!rendered.contains("call"));
        }
    }

    #[test]
    fn bounds_the_public_catalog_and_marks_only_the_newest_entry() {
        let points = (0..=MAX_RESUME_ENTRIES)
            .map(|index| ResumePoint {
                run_id: RunId::from(format!("private-run-{index}")),
                project_id: "private-project".into(),
                status: RunStatus::Created,
                turn_id: None,
                step_id: None,
                next: ResumeNext::StartStep,
            })
            .collect();

        let catalog = project_resume_points("private-project", points).expect("catalog");

        assert!(catalog.truncated);
        assert_eq!(catalog.entries.len(), MAX_RESUME_ENTRIES);
        assert!(catalog.entries[0].latest);
        assert!(catalog.entries.iter().skip(1).all(|entry| !entry.latest));
        assert!(!format!("{catalog:?}").contains("private-run"));
    }
}
