use std::fmt;
use std::fs;
use std::path::Path;

use getrandom::fill as fill_random;
use orchester_protokoll::{ActionId, CallId, RunId, StepId, TurnId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::super::coordinator::CoordinatorInput;
use super::super::run_store::NewRun;

const MAX_ID_BYTES: usize = 256;
const MAX_PATH_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IdentityError {
    #[error("self-agent workspace is unavailable")]
    Workspace,
    #[error("self-agent workspace identity is invalid")]
    Invalid,
    #[error("self-agent run identifier generation failed")]
    Entropy,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct WorkspaceIdentity {
    canonical_root: String,
    project_id: String,
    workspace_identity: String,
    owner_actor_id: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WorkspaceIdentitySnapshot {
    pub project_id: String,
    pub workspace_identity: String,
    pub canonical_root: String,
    pub owner_actor_id: String,
}

impl fmt::Debug for WorkspaceIdentitySnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceIdentitySnapshot")
            .field("project_id", &self.project_id)
            .field("workspace_identity", &self.workspace_identity)
            .field("canonical_root", &"[REDACTED]")
            .field("owner_actor_id", &"[REDACTED]")
            .finish()
    }
}

impl WorkspaceIdentity {
    pub fn for_workspace(
        workspace_root: impl AsRef<Path>,
        owner_actor_id: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let canonical = fs::canonicalize(workspace_root).map_err(|_| IdentityError::Workspace)?;
        if !canonical.is_dir() {
            return Err(IdentityError::Workspace);
        }
        let canonical_root = canonical.to_str().ok_or(IdentityError::Invalid)?.to_owned();
        let owner_actor_id = owner_actor_id.into();
        if canonical_root.is_empty()
            || canonical_root.len() > MAX_PATH_BYTES
            || canonical_root.chars().any(char::is_control)
            || !valid_identifier(&owner_actor_id)
        {
            return Err(IdentityError::Invalid);
        }
        let project_id = format!("project-{}", digest(b"project", canonical_root.as_bytes()));
        let workspace_identity = format!(
            "workspace-{}",
            digest(b"workspace", canonical_root.as_bytes())
        );
        Ok(Self {
            canonical_root,
            project_id,
            workspace_identity,
            owner_actor_id,
        })
    }

    pub(super) fn snapshot(&self) -> WorkspaceIdentitySnapshot {
        WorkspaceIdentitySnapshot {
            project_id: self.project_id.clone(),
            workspace_identity: self.workspace_identity.clone(),
            canonical_root: self.canonical_root.clone(),
            owner_actor_id: self.owner_actor_id.clone(),
        }
    }

    pub(super) fn coordinator_input(
        &self,
        prompt: String,
        config_snapshot_hash: String,
        max_steps: u64,
        policy_snapshot_hash: String,
    ) -> Result<(CoordinatorInput, RunId), IdentityError> {
        let token = random_token()?;
        let run_id = RunId::from(format!("run-{token}"));
        let input = CoordinatorInput {
            run: NewRun {
                run_id: run_id.clone(),
                project_id: self.project_id.clone(),
                owner_actor_id: self.owner_actor_id.clone(),
                canonical_root: self.canonical_root.clone(),
                workspace_identity: self.workspace_identity.clone(),
                policy_snapshot_hash,
                config_snapshot_hash,
                max_steps,
                occurred_at: "service-pending".into(),
            },
            prompt,
            turn_id: TurnId::from(format!("turn-{token}")),
            step_id: StepId::from(format!("step-{token}")),
            model_call_id: CallId::from(format!("model-call-{token}")),
            action_id: ActionId::from(format!("action-{token}")),
        };
        Ok((input, run_id))
    }
}

impl fmt::Debug for WorkspaceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkspaceIdentity")
            .field("project_id", &self.project_id)
            .field("workspace_identity", &self.workspace_identity)
            .field("canonical_root", &"[REDACTED]")
            .field("owner_actor_id", &"[REDACTED]")
            .finish()
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_ID_BYTES && !value.chars().any(char::is_control)
}

fn digest(domain: &[u8], value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-workspace-identity-v1\0");
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn random_token() -> Result<String, IdentityError> {
    let mut bytes = [0u8; 16];
    fill_random(&mut bytes).map_err(|_| IdentityError::Entropy)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
