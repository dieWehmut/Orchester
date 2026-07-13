use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalRequestInput, DurableApprovalStore,
};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::barrier::{ExecutionAuthorization, PreExecutionBarrier};
use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, ResumeNext, RunStore, SqliteRunStore,
    StoreError, Transition,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_protokoll::{
    AgentAction, ApprovalId, CallId, HarnessEventKind, RunId, StepId, StopReason, TurnId,
};
use std::sync::Arc;

#[path = "support/allowed_run.rs"]
#[allow(dead_code)]
mod allowed_run;

fn new_run(id: &str, owner: &str) -> NewRun {
    NewRun {
        run_id: RunId::from(id),
        project_id: format!("project-{id}"),
        owner_actor_id: owner.into(),
        canonical_root: format!("/workspace/{id}"),
        workspace_identity: format!("workspace-{id}"),
        policy_snapshot_hash: "policy-v1".into(),
        config_snapshot_hash: "config-v1".into(),
        max_steps: 4,
        occurred_at: "2026-07-13T00:00:00Z".into(),
    }
}

fn start_step(store: &SqliteRunStore, run_id: &RunId, step_id: &str) {
    store
        .append_transition(
            run_id,
            "owner-a",
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from(step_id),
                occurred_at: "2026-07-13T00:00:01Z".into(),
            },
        )
        .unwrap();
}

fn model_event(
    store: &SqliteRunStore,
    run_id: &RunId,
    step_id: &str,
    call_id: &str,
    kind: HarnessEventKind,
) {
    store
        .append_event(
            "owner-a",
            run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from(step_id)),
                call_id: Some(CallId::from(call_id)),
                occurred_at: "2026-07-13T00:00:02Z".into(),
                kind,
            },
        )
        .unwrap();
}

#[test]
fn resume_projection_is_owner_scoped_and_omits_terminal_runs() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-created", "owner-a"))
        .unwrap();
    let points = store.resume_points_owned("owner-a").unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].run_id, run.run_id);
    assert!(matches!(points[0].next, ResumeNext::StartStep));
    assert!(matches!(
        store.resume_points_owned("owner-b"),
        Ok(points) if points.is_empty()
    ));
    assert!(matches!(
        store.resume_point_owned(&run.run_id, "owner-b"),
        Err(StoreError::NotFound)
    ));

    start_step(&store, &run.run_id, "step-resume-created");
    store
        .append_transition(
            &run.run_id,
            "owner-a",
            Transition::Complete {
                reason: StopReason::Succeeded,
                summary: "done".into(),
                occurred_at: "2026-07-13T00:00:03Z".into(),
            },
        )
        .unwrap();
    assert!(store.resume_points_owned("owner-a").unwrap().is_empty());
}

#[test]
fn resume_projection_reconciles_running_model_without_replay() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-model", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-model");
    model_event(
        &store,
        &run.run_id,
        "step-resume-model",
        "model-call-resume",
        HarnessEventKind::ModelStarted,
    );

    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    let rendered = format!("{point:?}");
    assert!(!rendered.contains("run-resume-model"));
    assert!(!rendered.contains("model-call-resume"));
    assert!(matches!(
        point.next,
        ResumeNext::ReconcileModelCall { ref call_id } if call_id.0 == "model-call-resume"
    ));
}

#[test]
fn completed_model_requires_durable_completion_transcript() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-completed", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-completed");
    model_event(
        &store,
        &run.run_id,
        "step-resume-completed",
        "model-call-completed",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-completed",
        "model-call-completed",
        HarnessEventKind::ModelCompleted {
            assistant_text: "response".into(),
        },
    );
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::ProcessModelOutput { .. }));

    let missing = store
        .create_run(new_run("run-resume-missing", "owner-a"))
        .unwrap();
    start_step(&store, &missing.run_id, "step-resume-missing");
    model_event(
        &store,
        &missing.run_id,
        "step-resume-missing",
        "model-call-missing",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &missing.run_id,
        "step-resume-missing",
        "model-call-missing",
        HarnessEventKind::ModelCompleted {
            assistant_text: String::new(),
        },
    );
    assert!(matches!(
        store.resume_point_owned(&missing.run_id, "owner-a"),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn durable_model_start_context_is_accepted_by_resume_projection() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-request", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-request");
    store
        .append_model_started_with_transcript(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-resume-request")),
                call_id: Some(CallId::from("model-call-request")),
                occurred_at: "2026-07-13T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
            vec![TranscriptRecord::user("inspect")],
        )
        .unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::ReconcileModelCall { .. }));
}

#[test]
fn recorded_action_returns_policy_evaluation_resume_point() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-action", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-action");
    model_event(
        &store,
        &run.run_id,
        "step-resume-action",
        "model-call-action",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-action",
        "model-call-action",
        HarnessEventKind::ModelCompleted {
            assistant_text: "inspect".into(),
        },
    );
    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    store
        .record_action(
            "owner-a",
            ActionRecord {
                action_id: "action-resume-action".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-resume-action"),
                call_id: CallId::from("provider-resume-action"),
                origin_model_call_id: CallId::from("model-call-action"),
                action_hash: action_hash(&action).unwrap(),
                effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                action,
                occurred_at: "2026-07-13T00:00:03Z".into(),
            },
        )
        .unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::EvaluatePolicy { ref action_id } if action_id.0 == "action-resume-action"
    ));
}

#[test]
fn interrupted_unknown_model_is_reconciled_without_automatic_retry() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-unknown", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-unknown");
    model_event(
        &store,
        &run.run_id,
        "step-resume-unknown",
        "model-call-unknown",
        HarnessEventKind::ModelStarted,
    );
    store
        .append_transition(
            &run.run_id,
            "owner-a",
            Transition::Complete {
                reason: StopReason::InterruptedUnknownOutcome,
                summary: "provider outcome unknown".into(),
                occurred_at: "2026-07-13T00:00:03Z".into(),
            },
        )
        .unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::ReconcileModelCall { .. }));
}

#[test]
fn approval_resume_points_distinguish_request_wait_and_capability_recovery() {
    let store = std::sync::Arc::new(SqliteRunStore::in_memory().unwrap());
    let run = store
        .create_run(new_run("run-resume-approval", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-approval");
    model_event(
        &store,
        &run.run_id,
        "step-resume-approval",
        "model-call-approval",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-approval",
        "model-call-approval",
        HarnessEventKind::ModelCompleted {
            assistant_text: "request command approval".into(),
        },
    );
    let action = AgentAction::RunCommand {
        program: "git".into(),
        args: vec!["status".into()],
        cwd: None,
    };
    let action_hash_value = action_hash(&action).unwrap();
    store
        .record_action(
            "owner-a",
            ActionRecord {
                action_id: "action-resume-approval".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-resume-approval"),
                call_id: CallId::from("provider-resume-approval"),
                origin_model_call_id: CallId::from("model-call-approval"),
                action_hash: action_hash_value.clone(),
                effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                action,
                occurred_at: "2026-07-13T00:00:03Z".into(),
            },
        )
        .unwrap();
    store
        .append_event(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-resume-approval")),
                call_id: None,
                occurred_at: "2026-07-13T00:00:04Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id: "action-resume-approval".into(),
                    decision: orchester_protokoll::PolicyDecision::Ask,
                    rule_id: "command.external_effect".into(),
                },
            },
        )
        .unwrap();

    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::CreateApprovalRequest { ref action_id }
            if action_id.0 == "action-resume-approval"
    ));

    let now = unix_now();
    let approval = DurableApprovalStore::new(store.clone());
    let approval_id: ApprovalId = "approval-resume-approval".into();
    let binding = ApprovalBinding {
        run_id: run.run_id.clone(),
        action_id: "action-resume-approval".into(),
        action_hash: action_hash_value,
        workspace_identity: "workspace-run-resume-approval".into(),
        policy_snapshot_hash: "policy-v1".into(),
        config_snapshot_hash: "config-v1".into(),
    };
    approval
        .request(ApprovalRequestInput {
            approval_id: approval_id.clone(),
            owner_actor_id: "owner-a".into(),
            binding: binding.clone(),
            action_summary: "run_command program_bytes=4 args_count=1 args_bytes=6 cwd_bytes=0"
                .into(),
            risk: "high".into(),
            rule_id: "command.external_effect".into(),
            created_at: format!("unix:{now}"),
            expires_at: format!("unix:{}", now + 600),
            created_at_unix: now - 1,
            expires_at_unix: now + 600,
        })
        .unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::AwaitApproval { approval_id: ref id } if id.0 == "approval-resume-approval"
    ));

    approval.approve(&approval_id, "owner-a", &binding).unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::RecoverApprovalCapability {
            approval_id: ref id,
            ref action_id,
        }
            if id.0 == "approval-resume-approval" && action_id.0 == "action-resume-approval"
    ));
}

#[test]
fn started_tool_returns_reconcile_resume_point_without_replay() {
    let store = Arc::new(SqliteRunStore::in_memory().unwrap());
    let allowed = allowed_run::create_allowed_run(&store, "resume-tool");
    let audit_path = std::env::temp_dir().join(format!(
        "orchester-resume-audit-{}-{}.jsonl",
        std::process::id(),
        unix_now()
    ));
    let audit = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(store.clone(), audit.clone());
    let permit = barrier
        .prepare(
            &allowed.owner,
            &allowed.run_id,
            &allowed.action_id,
            ExecutionAuthorization::Allow,
            "2026-07-13T00:00:06Z",
        )
        .unwrap();
    barrier
        .start_tool(
            &allowed.owner,
            &allowed.run_id,
            permit,
            allowed.tool_started_input(),
        )
        .unwrap();

    let point = store
        .resume_point_owned(&allowed.run_id, &allowed.owner)
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::ReconcileToolOutcome { ref action_id, ref call_id }
            if action_id == &allowed.action_id && call_id == &allowed.provider_call_id
    ));
    drop(barrier);
    drop(audit);
    std::fs::remove_file(audit_path).ok();
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
