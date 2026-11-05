use super::*;
use crate::harness::approval::{
    ApprovalBinding, ApprovalError, ApprovalRequestInput, DurableApprovalStore,
};
use crate::harness::run_store::{action_hash, ResumeNext, RunStore};
use orchester_protokoll::ApprovalId;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfAgentPendingApproval {
    pub summary: String,
    pub risk: String,
    pub details: Vec<String>,
    approval_id: ApprovalId,
    binding: ApprovalBinding,
    call_id: CallId,
}

impl<M: LanguageModel, A: AuditSink> SelfAgentRuntime<M, A> {
    pub fn pending_approval(
        &self,
        outcome: &SelfAgentRunOutcome,
    ) -> Result<Option<SelfAgentPendingApproval>, SelfAgentRuntimeError> {
        let SelfAgentTurn::Action {
            run_id,
            action_id,
            call_id,
            action,
            policy,
            ..
        } = outcome.final_turn()
        else {
            return Ok(None);
        };
        if policy.decision != PolicyDecision::Ask {
            return Ok(None);
        }
        let identity = self.service.identity();
        let store = self.store();
        let run = store
            .load_run_owned(run_id, &identity.owner_actor_id)
            .map_err(GovernedExecutionError::from)?;
        let point = store
            .resume_point_owned(run_id, &identity.owner_actor_id, &identity.project_id)
            .map_err(GovernedExecutionError::from)?
            .ok_or(GovernedExecutionError::NotReady)?;
        let binding = ApprovalBinding {
            run_id: run_id.clone(),
            action_id: action_id.clone(),
            action_hash: action_hash(action).map_err(GovernedExecutionError::from)?,
            workspace_identity: identity.workspace_identity,
            policy_snapshot_hash: run.policy_snapshot_hash,
            config_snapshot_hash: run.config_snapshot_hash,
        };
        let approvals = DurableApprovalStore::new(store.clone());
        let id = match point.next {
            ResumeNext::CreateApprovalRequest {
                action_id: expected,
            } if expected == *action_id => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| ApprovalError::InvalidRequest)?
                    .as_secs();
                approvals.request(ApprovalRequestInput {
                    approval_id: ApprovalId::from(format!("{}-approval", action_id.0)),
                    owner_actor_id: identity.owner_actor_id.clone(),
                    binding: binding.clone(),
                    action_summary: action.action_summary(),
                    risk: "requires approval".into(),
                    rule_id: policy.rule_id.clone(),
                    created_at: format!("unix:{now}"),
                    expires_at: format!("unix:{}", now + 600),
                    created_at_unix: now,
                    expires_at_unix: now + 600,
                })?
            }
            ResumeNext::AwaitApproval { approval_id } => {
                if approvals
                    .snapshot(&approval_id, &identity.owner_actor_id)?
                    .binding
                    != binding
                {
                    return Err(ApprovalError::BindingMismatch.into());
                }
                approval_id
            }
            _ => return Err(GovernedExecutionError::NotReady.into()),
        };
        let preview =
            serde_json::to_string_pretty(action).unwrap_or_else(|_| action.action_summary());
        let mut redactor = self.streaming_redactor();
        redactor.push(&preview);
        let details = redactor.finish().lines().map(str::to_owned).collect();
        Ok(Some(SelfAgentPendingApproval {
            summary: action.action_summary(),
            risk: "This action requires your approval.".into(),
            details,
            approval_id: id,
            binding,
            call_id: call_id.clone(),
        }))
    }
    pub async fn resolve_approval(
        &self,
        pending: &SelfAgentPendingApproval,
        approve: bool,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<Option<SelfAgentRunOutcome>, SelfAgentRuntimeError> {
        let identity = self.service.identity();
        if identity.workspace_identity != pending.binding.workspace_identity {
            return Err(ApprovalError::BindingMismatch.into());
        }
        let approvals = DurableApprovalStore::new(self.store().clone());
        if !approve {
            approvals.deny(&pending.approval_id, &identity.owner_actor_id)?;
            return Ok(None);
        }
        if cancel.is_cancelled() {
            return Err(SelfAgentRuntimeError::Cancelled);
        }
        let capability = approvals.approve(
            &pending.approval_id,
            &identity.owner_actor_id,
            &pending.binding,
        )?;
        let executed = self
            .execution
            .execute_approved_async(
                &pending.binding.run_id,
                &pending.binding.action_id,
                &pending.call_id,
                capability,
                cancel.clone(),
            )
            .await?;
        let turn = self
            .service
            .continue_run_with_events(
                pending.binding.run_id.clone(),
                cancel.clone(),
                events.clone(),
            )
            .await?;
        self.drive_turn(turn, cancel, events, &self.narrator(None))
            .await
            .map(|result| {
                Some(result.with_first_tool_step(SelfAgentToolStep::new(
                    pending.binding.action_id.clone(),
                    pending.call_id.clone(),
                    executed,
                )))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::service::identity::WorkspaceIdentity;
    use crate::harness::{
        agent_loop::AgentLoopConfig,
        audit::JsonlAuditSink,
        config::PermissionMode,
        context::{ContextAssembler, ContextLimits},
        files::FileToolLimits,
        governance::PolicyEngine,
    };
    use orchester_modell::{ModelResponse, ScriptedLlm};
    fn fixture_with_path(
        path: &str,
    ) -> (
        std::path::PathBuf,
        SelfAgentRuntime<ScriptedLlm, JsonlAuditSink>,
    ) {
        let root = std::env::temp_dir().join(format!(
            "orchester-review-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("workspace")).unwrap();
        let engine = SelfAgentLoop::new(
            ScriptedLlm::new([
                Ok(ModelResponse::tool(
                    "write-review",
                    "write_file",
                    serde_json::json!({"path":path,"content":"approved once"}).to_string(),
                )),
                Ok(ModelResponse {
                    assistant_text: "done".into(),
                    tool_call: None,
                    usage: Default::default(),
                    opaque_items: Vec::new(),
                }),
            ]),
            ContextAssembler::new(ContextLimits::default(), Vec::new()),
            AgentLoopConfig {
                model: "test".into(),
                max_steps: 8,
                max_text_bytes: 65536,
                store: false,
            },
        )
        .unwrap();
        let store = Arc::new(
            SqliteRunStore::open_with_terminal_secrets(root.join("state/runs.db"), Vec::new())
                .unwrap(),
        );
        let audit = Arc::new(JsonlAuditSink::open(root.join("audit/events.jsonl")).unwrap());
        let identity = WorkspaceIdentity::for_workspace(root.join("workspace"), "owner").unwrap();
        let service = SelfAgentService::from_identity_with_policy(
            engine,
            store.clone(),
            identity,
            PolicyEngine::new().with_permission_mode(Some(PermissionMode::ReadOnly)),
            SystemCoordinatorClock,
        );
        let execution = GovernedExecution::new(
            store,
            audit,
            ToolExecutor::new(root.join("workspace"), FileToolLimits::default()).unwrap(),
            "owner",
        )
        .unwrap();
        (
            root,
            SelfAgentRuntime::from_parts(service, execution).with_explicit_permissions(true),
        )
    }
    #[tokio::test]
    async fn review_approval_executes_exact_action_once_and_rejects_changed_binding() {
        let (root, runtime) = fixture_with_path("review.txt");
        let result = runtime
            .run("write", CancellationToken::new())
            .await
            .unwrap();
        let pending = runtime.pending_approval(&result).unwrap().unwrap();
        assert!(!root.join("workspace/review.txt").exists());
        let mut wrong = pending.clone();
        wrong.binding.workspace_identity = "wrong".into();
        assert!(runtime
            .resolve_approval(&wrong, true, CancellationToken::new(), None)
            .await
            .is_err());
        assert!(!root.join("workspace/review.txt").exists());
        let result = runtime
            .resolve_approval(&pending, true, CancellationToken::new(), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.final_turn().text(), Some("done"));
        assert_eq!(
            std::fs::read_to_string(root.join("workspace/review.txt")).unwrap(),
            "approved once"
        );
        assert!(runtime
            .resolve_approval(&pending, true, CancellationToken::new(), None)
            .await
            .is_err());
        drop(runtime);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn denying_review_leaves_files_unchanged_and_prevents_later_execution() {
        let (root, runtime) = fixture_with_path("review.txt");
        let result = runtime
            .run("write", CancellationToken::new())
            .await
            .unwrap();
        let pending = runtime.pending_approval(&result).unwrap().unwrap();
        assert!(runtime
            .resolve_approval(&pending, false, CancellationToken::new(), None)
            .await
            .unwrap()
            .is_none());
        assert!(!root.join("workspace/review.txt").exists());
        assert!(runtime
            .resolve_approval(&pending, true, CancellationToken::new(), None)
            .await
            .is_err());
        drop(runtime);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn approved_outside_write_is_single_use_and_does_not_change_policy() {
        let (root, runtime) = fixture_with_path("../outside.txt");
        let result = runtime
            .run("outside", CancellationToken::new())
            .await
            .unwrap();
        let pending = runtime.pending_approval(&result).unwrap().unwrap();
        assert!(!root.join("outside.txt").exists());
        let policy_before = runtime.service.coordinator.policy_snapshot_hash();
        runtime
            .resolve_approval(&pending, true, CancellationToken::new(), None)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("outside.txt")).unwrap(),
            "approved once"
        );
        assert_eq!(
            runtime.service.coordinator.policy_snapshot_hash(),
            policy_before
        );
        std::fs::write(root.join("outside.txt"), "preserve after approval").unwrap();
        assert!(runtime
            .resolve_approval(&pending, true, CancellationToken::new(), None)
            .await
            .is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("outside.txt")).unwrap(),
            "preserve after approval"
        );
        drop(runtime);
        std::fs::remove_dir_all(root).unwrap();
    }
}
