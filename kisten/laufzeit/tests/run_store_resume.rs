use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, ResumeNext, RunStore, SqliteRunStore,
    StoreError, Transition,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_protokoll::{
    AgentAction, CallId, HarnessEventKind, RunId, StepId, StopReason, TurnId,
};

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
