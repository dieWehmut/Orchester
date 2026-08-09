use std::path::Path;

use orchester_protokoll::PolicyDecision;
use thiserror::Error;

use super::unresolved::unresolved_metadata;
use super::{IdentityError, WorkspaceIdentitySnapshot};
use crate::harness::config::{ConfigError, UserConfig};
use crate::harness::credentials::CredentialStore;
use crate::harness::run_store::{ResumeNext, ResumePoint, SqliteRunStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentStatus {
    pub workspace: WorkspaceIdentitySnapshot,
    pub model: SelfAgentModelReport,
    pub governance: SelfAgentGovernanceStatus,
    pub limits: SelfAgentLimitStatus,
    pub durable: SelfAgentDurableStatus,
}

/// How the active model configuration presents to a read-only caller.
///
/// Status is a diagnostic, so a configuration that cannot be resolved is
/// reported rather than raised: refusing to describe a workspace is exactly
/// the wrong response when the operator is trying to find out why it is
/// misconfigured. Distinguishing [`Self::Unresolved`] from
/// [`Self::NotConfigured`] keeps a broken profile from masquerading as an
/// absent one. Execution paths still resolve strictly and still fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfAgentModelReport {
    /// Neither a provider nor a model is configured.
    NotConfigured,
    /// Model fields are present but do not form a usable transport profile.
    /// Both members are validation metadata and carry no configured value.
    Unresolved { path: String, message: String },
    /// A complete, validated profile.
    Configured(SelfAgentModelStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentModelStatus {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    pub plan_reasoning_effort: Option<String>,
    pub store_responses: bool,
    pub service_tier: Option<String>,
    pub requires_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentGovernanceStatus {
    pub approval_reviewer: String,
    pub network: PolicyDecision,
    pub out_of_workspace: PolicyDecision,
    pub shell_interpreters: PolicyDecision,
    pub approval_ttl_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfAgentLimitStatus {
    pub max_steps: u32,
    pub max_minutes: u32,
    pub max_same_failure: u32,
    pub max_observation_bytes: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelfAgentDurableStatus {
    pub database_present: bool,
    pub resumable_runs: usize,
    pub ready_to_continue: usize,
    pub awaiting_approval: usize,
    pub reconciliation_required: usize,
}

#[derive(Debug, Error)]
pub enum SelfAgentStatusError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("self-agent status path inspection failed")]
    Io(#[source] std::io::Error),
}

pub fn load_self_agent_status<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
    workspace_root: impl AsRef<Path>,
    state_database: impl AsRef<Path>,
    owner_actor_id: impl Into<String>,
) -> Result<SelfAgentStatus, SelfAgentStatusError> {
    let owner_actor_id = owner_actor_id.into();
    let workspace = WorkspaceIdentitySnapshot::for_workspace(workspace_root, &owner_actor_id)?;
    let model = model_status(config);
    let governance = SelfAgentGovernanceStatus {
        approval_reviewer: config.governance.approval_reviewer.clone(),
        network: config.governance.tool_network,
        out_of_workspace: config.governance.out_of_workspace,
        shell_interpreters: config.governance.shell_interpreters,
        approval_ttl_seconds: config.governance.approval_ttl_seconds,
    };
    let limits = SelfAgentLimitStatus {
        max_steps: config.limits.max_steps,
        max_minutes: config.limits.max_minutes,
        max_same_failure: config.limits.max_same_failure,
        max_observation_bytes: config.limits.max_observation_bytes,
    };
    let durable = durable_status(
        config,
        credentials,
        state_database.as_ref(),
        &owner_actor_id,
        &workspace.project_id,
    )?;
    Ok(SelfAgentStatus {
        workspace,
        model,
        governance,
        limits,
        durable,
    })
}

fn model_status(config: &UserConfig) -> SelfAgentModelReport {
    if config.model_provider.is_none() && config.model.is_none() {
        return SelfAgentModelReport::NotConfigured;
    }
    let profile = match config.resolve_model_profile() {
        Ok(profile) => profile,
        Err(error) => {
            let (path, message) = unresolved_metadata(error);
            return SelfAgentModelReport::Unresolved { path, message };
        }
    };
    SelfAgentModelReport::Configured(SelfAgentModelStatus {
        provider: profile.provider,
        provider_name: profile.provider_name,
        model: profile.model,
        reasoning_effort: profile.reasoning_effort,
        plan_reasoning_effort: profile.plan_mode_reasoning_effort,
        store_responses: profile.store,
        service_tier: profile.service_tier,
        requires_auth: profile.requires_auth,
    })
}

fn durable_status<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
    state_database: &Path,
    owner_actor_id: &str,
    project_id: &str,
) -> Result<SelfAgentDurableStatus, SelfAgentStatusError> {
    if !state_database
        .try_exists()
        .map_err(SelfAgentStatusError::Io)?
    {
        return Ok(SelfAgentDurableStatus::default());
    }
    let secrets = config.resolve_configured_secrets(credentials)?;
    let store = SqliteRunStore::open_with_terminal_secrets(state_database, secrets.values)?;
    let points = store.resume_points_owned(owner_actor_id, project_id)?;
    Ok(summarize_resume_points(points))
}

fn summarize_resume_points(points: Vec<ResumePoint>) -> SelfAgentDurableStatus {
    let mut status = SelfAgentDurableStatus {
        database_present: true,
        resumable_runs: points.len(),
        ..SelfAgentDurableStatus::default()
    };
    for point in points {
        match point.next {
            ResumeNext::CreateApprovalRequest { .. }
            | ResumeNext::AwaitApproval { .. }
            | ResumeNext::RecoverApprovalCapability { .. } => {
                status.awaiting_approval += 1;
            }
            ResumeNext::ReconcileModelCall { .. }
            | ResumeNext::ReconcileToolOutcome { .. }
            | ResumeNext::ManualReconciliation { .. } => {
                status.reconciliation_required += 1;
            }
            ResumeNext::StartStep
            | ResumeNext::StartModel { .. }
            | ResumeNext::ProcessModelOutput { .. }
            | ResumeNext::EvaluatePolicy { .. }
            | ResumeNext::PrepareExecution { .. }
            | ResumeNext::StartNextStep
            | ResumeNext::ContinueValidation { .. } => {
                status.ready_to_continue += 1;
            }
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::run_store::{ResumeStage, RunStatus};
    use orchester_protokoll::{ActionId, ApprovalId, CallId};

    fn point(next: ResumeNext) -> ResumePoint {
        ResumePoint {
            run_id: "run-status".into(),
            project_id: "project-status".into(),
            status: RunStatus::Running,
            turn_id: None,
            step_id: None,
            next,
        }
    }

    #[test]
    fn resume_summary_separates_ready_approval_and_reconciliation_states() {
        let status = summarize_resume_points(vec![
            point(ResumeNext::StartStep),
            point(ResumeNext::AwaitApproval {
                approval_id: ApprovalId::from("approval-status"),
            }),
            point(ResumeNext::RecoverApprovalCapability {
                approval_id: ApprovalId::from("approval-recover"),
                action_id: ActionId::from("action-recover"),
            }),
            point(ResumeNext::ReconcileToolOutcome {
                action_id: ActionId::from("action-status"),
                call_id: CallId::from("call-status"),
            }),
            point(ResumeNext::ManualReconciliation {
                stage: ResumeStage::Unknown,
            }),
        ]);

        assert_eq!(status.resumable_runs, 5);
        assert_eq!(status.ready_to_continue, 1);
        assert_eq!(status.awaiting_approval, 2);
        assert_eq!(status.reconciliation_required, 2);
    }
}
