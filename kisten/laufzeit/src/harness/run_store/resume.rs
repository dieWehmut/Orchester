use std::fmt;

use orchester_protokoll::{ActionId, CallId, RunId, StepId, TurnId};
use rusqlite::{params, Connection, OptionalExtension};

use crate::harness::transcript::TranscriptCodec;

use super::{database::load_snapshot, RunSnapshot, RunStatus, SqliteRunStore, StoreError};

mod evidence;

#[derive(Clone, PartialEq, Eq)]
pub struct ResumePoint {
    pub run_id: RunId,
    pub project_id: String,
    pub status: RunStatus,
    pub turn_id: Option<TurnId>,
    pub step_id: Option<StepId>,
    pub next: ResumeNext,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResumeNext {
    StartStep,
    StartModel {
        step_id: StepId,
    },
    ReconcileModelCall {
        call_id: CallId,
    },
    ProcessModelOutput {
        call_id: CallId,
    },
    EvaluatePolicy {
        action_id: ActionId,
    },
    PrepareExecution {
        action_id: ActionId,
        call_id: CallId,
    },
    CreateApprovalRequest {
        action_id: ActionId,
    },
    AwaitApproval {
        approval_id: orchester_protokoll::ApprovalId,
    },
    RecoverApprovalCapability {
        approval_id: orchester_protokoll::ApprovalId,
        action_id: ActionId,
    },
    ReconcileToolOutcome {
        action_id: ActionId,
        call_id: CallId,
    },
    StartNextStep,
    ContinueValidation {
        step_id: Option<StepId>,
        mutation_generation: u64,
    },
    ManualReconciliation {
        stage: ResumeStage,
    },
}

impl fmt::Debug for ResumeNext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::StartStep => "StartStep",
            Self::StartModel { .. } => "StartModel",
            Self::ReconcileModelCall { .. } => "ReconcileModelCall",
            Self::ProcessModelOutput { .. } => "ProcessModelOutput",
            Self::EvaluatePolicy { .. } => "EvaluatePolicy",
            Self::PrepareExecution { .. } => "PrepareExecution",
            Self::CreateApprovalRequest { .. } => "CreateApprovalRequest",
            Self::AwaitApproval { .. } => "AwaitApproval",
            Self::RecoverApprovalCapability { .. } => "RecoverApprovalCapability",
            Self::ReconcileToolOutcome { .. } => "ReconcileToolOutcome",
            Self::StartNextStep => "StartNextStep",
            Self::ContinueValidation { .. } => "ContinueValidation",
            Self::ManualReconciliation { .. } => "ManualReconciliation",
        };
        formatter.debug_tuple(name).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeStage {
    MissingStep,
    ModelCall,
    ToolOutcome,
    UnboundApproval,
    Unknown,
}

impl fmt::Debug for ResumePoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResumePoint")
            .field("run_id", &"<redacted>")
            .field("project_id", &"<redacted>")
            .field("status", &self.status)
            .field("turn_present", &self.turn_id.is_some())
            .field("step_present", &self.step_id.is_some())
            .field("next", &self.next)
            .finish()
    }
}

impl SqliteRunStore {
    pub fn resume_points_owned(
        &self,
        owner_actor_id: &str,
        project_id: &str,
    ) -> Result<Vec<ResumePoint>, StoreError> {
        ensure_owner(owner_actor_id, &self.event_sanitizer)?;
        ensure_project(project_id, &self.event_sanitizer)?;
        let connection = self.connection()?;
        let codec = self.codec();
        let mut statement = connection.prepare(
            "SELECT run_id FROM runs
             WHERE owner_actor_id = ?1 AND project_id = ?2
             ORDER BY updated_at, run_id",
        )?;
        let run_ids = statement
            .query_map(params![owner_actor_id, project_id], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut points = Vec::new();
        for run_id in run_ids {
            let run_id = RunId::from(run_id);
            if let Some(point) = resume_point_from_connection(
                &connection,
                &run_id,
                owner_actor_id,
                project_id,
                &codec,
            )? {
                points.push(point);
            }
        }
        Ok(points)
    }

    pub fn resume_point_owned(
        &self,
        run_id: &RunId,
        owner_actor_id: &str,
        project_id: &str,
    ) -> Result<Option<ResumePoint>, StoreError> {
        ensure_owner(owner_actor_id, &self.event_sanitizer)?;
        ensure_project(project_id, &self.event_sanitizer)?;
        let connection = self.connection()?;
        let codec = self.codec();
        resume_point_from_connection(&connection, run_id, owner_actor_id, project_id, &codec)
    }
}

fn resume_point_from_connection(
    connection: &Connection,
    run_id: &RunId,
    owner_actor_id: &str,
    project_id: &str,
    codec: &TranscriptCodec,
) -> Result<Option<ResumePoint>, StoreError> {
    let snapshot = load_snapshot(connection, run_id, Some(owner_actor_id))?;
    if snapshot.project_id != project_id {
        return Err(StoreError::NotFound);
    }
    if snapshot.status.is_terminal() && snapshot.status != RunStatus::InterruptedUnknownOutcome {
        return Ok(None);
    }
    let next = derive_next(connection, &snapshot, codec)?;
    Ok(Some(ResumePoint {
        run_id: snapshot.run_id.clone(),
        project_id: snapshot.project_id.clone(),
        status: snapshot.status,
        turn_id: snapshot.current_turn_id.clone(),
        step_id: snapshot.current_step_id.clone(),
        next,
    }))
}

fn derive_next(
    connection: &Connection,
    run: &RunSnapshot,
    codec: &TranscriptCodec,
) -> Result<ResumeNext, StoreError> {
    match run.status {
        RunStatus::Created => {
            if run.current_step_id.is_none() && run.steps_used == 0 {
                Ok(ResumeNext::StartStep)
            } else {
                Err(StoreError::Corrupt)
            }
        }
        RunStatus::Running => derive_running(connection, run, codec),
        RunStatus::AwaitingApproval => derive_approval(connection, run, codec),
        RunStatus::Validating => {
            let step = load_step(connection, run)?.ok_or(StoreError::Corrupt)?;
            if matches!(
                step.status.as_str(),
                "observed" | "completed" | "failed" | "cancelled"
            ) {
                Ok(ResumeNext::ContinueValidation {
                    step_id: Some(StepId::from(step.step_id)),
                    mutation_generation: run.mutation_generation,
                })
            } else {
                Err(StoreError::Corrupt)
            }
        }
        RunStatus::InterruptedUnknownOutcome => derive_unknown(connection, run, codec),
        RunStatus::Succeeded
        | RunStatus::Failed
        | RunStatus::Cancelled
        | RunStatus::BudgetExceeded
        | RunStatus::RepeatedFailure => Err(StoreError::Corrupt),
    }
}

fn derive_running(
    connection: &Connection,
    run: &RunSnapshot,
    codec: &TranscriptCodec,
) -> Result<ResumeNext, StoreError> {
    let Some(step) = load_step(connection, run)? else {
        return if run.current_step_id.is_none() && run.steps_used == 0 {
            Ok(ResumeNext::StartStep)
        } else {
            Err(StoreError::Corrupt)
        };
    };
    match step.status.as_str() {
        "created" if step.model_phase == "not_started" && step.model_call_id.is_none() => {
            if step.action_id.is_none() {
                Ok(ResumeNext::StartModel {
                    step_id: StepId::from(step.step_id),
                })
            } else {
                Err(StoreError::Corrupt)
            }
        }
        "model_running" => {
            let call_id = step
                .model_call_id
                .as_ref()
                .ok_or(StoreError::Corrupt)?
                .clone();
            evidence::require_model_request_binding(connection, run, &step, codec)?;
            match step.model_phase.as_str() {
                "running" => Ok(ResumeNext::ReconcileModelCall {
                    call_id: CallId::from(call_id),
                }),
                "completed" => {
                    let call_id = CallId::from(call_id);
                    require_completed_transcript(connection, run, &step, codec)?;
                    Ok(ResumeNext::ProcessModelOutput { call_id })
                }
                _ => Err(StoreError::Corrupt),
            }
        }
        "action_recorded" => {
            let action_id =
                ActionId::from(step.action_id.as_ref().ok_or(StoreError::Corrupt)?.clone());
            let action = load_action(connection, run, &step, &action_id, codec)?;
            match action.state.as_str() {
                "recorded" => {
                    require_unprocessed_policy(&action)?;
                    Ok(ResumeNext::EvaluatePolicy { action_id })
                }
                "ready" if action.policy_decision.as_deref() == Some("allow") => {
                    require_policy_event(connection, run, &step, &action_id, &action, "allow")?;
                    validate_optional_audit_checkpoint(connection, run, &action)?;
                    if action.approval_id.is_some() || action.approval_state.is_some() {
                        Err(StoreError::Corrupt)
                    } else {
                        Ok(ResumeNext::PrepareExecution {
                            action_id,
                            call_id: CallId::from(action.call_id),
                        })
                    }
                }
                "ready"
                    if action.policy_decision.as_deref() == Some("ask")
                        && matches!(
                            action.approval_state.as_deref(),
                            Some("approved" | "executing")
                        ) =>
                {
                    require_policy_event(connection, run, &step, &action_id, &action, "ask")?;
                    validate_approval_binding(connection, run, &action_id, &action)?;
                    let approval_id = action.approval_id.ok_or(StoreError::Corrupt)?;
                    Ok(ResumeNext::RecoverApprovalCapability {
                        approval_id: approval_id.into(),
                        action_id,
                    })
                }
                _ => Err(StoreError::Corrupt),
            }
        }
        "tool_running" => {
            let action_id =
                ActionId::from(step.action_id.as_ref().ok_or(StoreError::Corrupt)?.clone());
            let action = load_action(connection, run, &step, &action_id, codec)?;
            if action.state != "executing" {
                return Err(StoreError::Corrupt);
            }
            validate_execution_evidence(connection, run, &step, &action_id, &action)?;
            let attempt_state: Option<String> = connection
                .query_row(
                    "SELECT state FROM tool_attempts
                     WHERE action_id = ?1 AND call_id = ?2",
                    params![action_id.0, action.call_id],
                    |row| row.get(0),
                )
                .optional()?;
            if attempt_state.as_deref() != Some("started") {
                return Err(StoreError::Corrupt);
            }
            Ok(ResumeNext::ReconcileToolOutcome {
                action_id,
                call_id: CallId::from(action.call_id),
            })
        }
        "observed" | "completed" | "failed" | "cancelled" => Ok(ResumeNext::StartNextStep),
        _ => Err(StoreError::Corrupt),
    }
}

fn derive_approval(
    connection: &Connection,
    run: &RunSnapshot,
    codec: &TranscriptCodec,
) -> Result<ResumeNext, StoreError> {
    let step = load_step(connection, run)?.ok_or(StoreError::Corrupt)?;
    if step.status != "awaiting_approval" {
        return Err(StoreError::Corrupt);
    }
    let action_id = ActionId::from(step.action_id.as_ref().ok_or(StoreError::Corrupt)?.clone());
    let action = load_action(connection, run, &step, &action_id, codec)?;
    if action.state != "awaiting_approval" || action.policy_decision.as_deref() != Some("ask") {
        return Err(StoreError::Corrupt);
    }
    require_policy_event(connection, run, &step, &action_id, &action, "ask")?;
    let approval: Option<(String, String)> = connection
        .query_row(
            "SELECT approval_id, state FROM approvals
             WHERE run_id = ?1 AND action_id = ?2 AND owner_actor_id = ?3",
            params![run.run_id.0, action_id.0, run.owner_actor_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((approval_id, state)) = approval else {
        return Ok(ResumeNext::CreateApprovalRequest { action_id });
    };
    validate_approval_binding(connection, run, &action_id, &action)?;
    match state.as_str() {
        "awaiting" => Ok(ResumeNext::AwaitApproval {
            approval_id: approval_id.into(),
        }),
        "approved" | "executing" => Ok(ResumeNext::RecoverApprovalCapability {
            approval_id: approval_id.into(),
            action_id,
        }),
        _ => Err(StoreError::Corrupt),
    }
}

fn derive_unknown(
    connection: &Connection,
    run: &RunSnapshot,
    codec: &TranscriptCodec,
) -> Result<ResumeNext, StoreError> {
    let Some(step) = load_step(connection, run)? else {
        return Ok(ResumeNext::ManualReconciliation {
            stage: ResumeStage::MissingStep,
        });
    };
    if step.status == "model_running" && step.model_phase == "running" {
        step.model_call_id.as_ref().ok_or(StoreError::Corrupt)?;
        evidence::require_model_request_binding(connection, run, &step, codec)?;
        return Ok(ResumeNext::ManualReconciliation {
            stage: ResumeStage::ModelCall,
        });
    }
    if step.status == "tool_running" {
        let action_id = ActionId::from(step.action_id.as_ref().ok_or(StoreError::Corrupt)?.clone());
        let action = load_action(connection, run, &step, &action_id, codec)?;
        if action.state != "executing" {
            return Err(StoreError::Corrupt);
        }
        validate_execution_evidence(connection, run, &step, &action_id, &action)?;
        let attempt_state: Option<String> = connection
            .query_row(
                "SELECT state FROM tool_attempts
                 WHERE action_id = ?1 AND call_id = ?2",
                params![action_id.0, action.call_id],
                |row| row.get(0),
            )
            .optional()?;
        if attempt_state.as_deref() != Some("started") {
            return Err(StoreError::Corrupt);
        }
        return Ok(ResumeNext::ManualReconciliation {
            stage: ResumeStage::ToolOutcome,
        });
    }
    Ok(ResumeNext::ManualReconciliation {
        stage: ResumeStage::Unknown,
    })
}

struct StepRow {
    step_id: String,
    status: String,
    model_phase: String,
    model_call_id: Option<String>,
    action_id: Option<String>,
}

fn load_step(connection: &Connection, run: &RunSnapshot) -> Result<Option<StepRow>, StoreError> {
    let Some(step_id) = run.current_step_id.as_ref() else {
        return Ok(None);
    };
    let row = connection
        .query_row(
            "SELECT step_id, status, model_phase, model_call_id, action_id
             FROM steps WHERE run_id = ?1 AND step_id = ?2",
            params![run.run_id.0, step_id.0],
            |row| {
                Ok(StepRow {
                    step_id: row.get(0)?,
                    status: row.get(1)?,
                    model_phase: row.get(2)?,
                    model_call_id: row.get(3)?,
                    action_id: row.get(4)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}
use evidence::{
    load_action, require_completed_transcript, require_policy_event, require_unprocessed_policy,
    validate_approval_binding, validate_execution_evidence, validate_optional_audit_checkpoint,
};
fn ensure_owner(
    owner: &str,
    sanitizer: &crate::harness::feedback::FeedbackEngine,
) -> Result<(), StoreError> {
    if owner.is_empty() || owner.len() > 512 || sanitizer.sanitize_text(owner) != owner {
        Err(StoreError::Invariant(
            "resume owner is not eligible for lookup".into(),
        ))
    } else {
        Ok(())
    }
}

fn ensure_project(
    project: &str,
    sanitizer: &crate::harness::feedback::FeedbackEngine,
) -> Result<(), StoreError> {
    if project.is_empty() || project.len() > 512 || sanitizer.sanitize_text(project) != project {
        Err(StoreError::Invariant(
            "resume project identifier is not eligible for lookup".into(),
        ))
    } else {
        Ok(())
    }
}
