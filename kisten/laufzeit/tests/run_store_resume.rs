use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalRequestInput, DurableApprovalStore,
};
use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::barrier::{ExecutionAuthorization, PreExecutionBarrier};
use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, ResumeNext, ResumeStage, RunStore,
    SqliteRunStore, StoreError, Transition,
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
    let input = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from(step_id)),
        call_id: Some(CallId::from(call_id)),
        occurred_at: "2026-07-13T00:00:02Z".into(),
        kind,
    };
    if matches!(input.kind, HarnessEventKind::ModelStarted) {
        store
            .append_model_started_with_transcript(
                "owner-a",
                run_id,
                input,
                vec![TranscriptRecord::user("resume request context")],
            )
            .unwrap();
    } else {
        store.append_event("owner-a", run_id, input).unwrap();
    }
}

#[test]
fn resume_rejects_a_model_call_without_request_binding() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-unbound-model", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-unbound-model");
    store
        .append_event(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-resume-unbound-model")),
                call_id: Some(CallId::from("model-call-unbound")),
                occurred_at: "2026-07-13T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
        )
        .unwrap();

    assert!(matches!(
        store.resume_point_owned(
            &run.run_id,
            "owner-a",
            "project-run-resume-unbound-model",
        ),
        Err(StoreError::Corrupt)
    ));
}

#[test]
fn resume_rejects_tool_running_without_its_started_event() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-tool-start-evidence-{}-{}",
        std::process::id(),
        unix_now()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = Arc::new(SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap());
    let allowed = allowed_run::create_allowed_run(&store, "resume-start-evidence");
    let audit_path = directory.join("audit.jsonl");
    let audit = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(store.clone(), audit);
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
    drop(barrier);
    drop(store);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET sanitized_payload = '{\"action_id\":\"tampered\"}'
             WHERE run_id = ?1 AND kind = 'tool.started'",
            rusqlite::params![allowed.run_id.0],
        )
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap();
    assert!(matches!(
        store.resume_point_owned(
            &allowed.run_id,
            &allowed.owner,
            "project-resume-start-evidence",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn resume_rejects_an_observed_step_without_terminal_evidence() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-observed-evidence-{}-{}",
        std::process::id(),
        unix_now()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = SqliteRunStore::open(&path).unwrap();
    let run = store
        .create_run(new_run("run-resume-observed-evidence", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-observed-evidence");
    drop(store);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE steps SET status = 'observed'
             WHERE run_id = ?1 AND step_id = 'step-resume-observed-evidence'",
            rusqlite::params![run.run_id.0],
        )
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        store.resume_point_owned(
            &run.run_id,
            "owner-a",
            "project-run-resume-observed-evidence",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn policy_denial_is_valid_terminal_evidence_for_the_next_step() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-policy-deny", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-policy-deny");
    model_event(
        &store,
        &run.run_id,
        "step-resume-policy-deny",
        "model-call-policy-deny",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-policy-deny",
        "model-call-policy-deny",
        HarnessEventKind::ModelCompleted {
            assistant_text: "run a command".into(),
        },
    );
    let action = AgentAction::RunCommand {
        program: "git".into(),
        args: vec!["push".into()],
        cwd: None,
    };
    store
        .record_action(
            "owner-a",
            ActionRecord {
                action_id: "action-policy-deny".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-resume-policy-deny"),
                call_id: CallId::from("provider-policy-deny"),
                origin_model_call_id: CallId::from("model-call-policy-deny"),
                action_hash: action_hash(&action).unwrap(),
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
                step_id: Some(StepId::from("step-resume-policy-deny")),
                call_id: None,
                occurred_at: "2026-07-13T00:00:04Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id: "action-policy-deny".into(),
                    decision: orchester_protokoll::PolicyDecision::Deny,
                    rule_id: "command.external_effect".into(),
                },
            },
        )
        .unwrap();

    let point = store
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-policy-deny")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::StartNextStep));
}

#[test]
fn tool_terminal_resume_requires_exact_event_observation_and_result_binding() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-tool-terminal-{}-{}",
        std::process::id(),
        unix_now()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = Arc::new(SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap());
    let allowed = allowed_run::create_allowed_run(&store, "resume-terminal-evidence");
    let audit_path = directory.join("audit.jsonl");
    let audit = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(store.clone(), audit);
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
    store
        .append_event(
            &allowed.owner,
            &allowed.run_id,
            allowed.tool_completed_input(&allowed.provider_call_id),
        )
        .unwrap();
    let point = store
        .resume_point_owned(
            &allowed.run_id,
            &allowed.owner,
            "project-resume-terminal-evidence",
        )
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::StartNextStep));
    drop(barrier);
    drop(store);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET sanitized_payload = '{\"observation\":{\"tampered\":true}}'
             WHERE run_id = ?1 AND kind = 'tool.completed'",
            rusqlite::params![allowed.run_id.0],
        )
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap();
    assert!(matches!(
        store.resume_point_owned(
            &allowed.run_id,
            &allowed.owner,
            "project-resume-terminal-evidence",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn resume_rejects_audit_checkpoint_field_drift() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-audit-drift-{}-{}",
        std::process::id(),
        unix_now()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = Arc::new(SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap());
    let allowed = allowed_run::create_allowed_run(&store, "resume-audit-drift");
    let audit_path = directory.join("audit.jsonl");
    let audit = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(store.clone(), audit);
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
    assert!(store
        .resume_point_owned(
            &allowed.run_id,
            &allowed.owner,
            "project-resume-audit-drift",
        )
        .unwrap()
        .is_some());
    drop(barrier);
    drop(store);

    let connection = rusqlite::Connection::open(&path).unwrap();
    let (event_id, action_sequence, checkpoint_sequence, audit_file, head_hash, synced_at): (
        String,
        i64,
        i64,
        String,
        String,
        String,
    ) = connection
        .query_row(
            "SELECT a.audit_event_id, a.audit_sequence, c.audit_sequence,
                    c.audit_file, c.head_hash, c.synced_at
             FROM actions a JOIN audit_checkpoints c ON c.event_id = a.audit_event_id
             WHERE a.action_id = ?1",
            rusqlite::params![allowed.action_id.0],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(action_sequence, checkpoint_sequence);
    let assert_corrupt = || {
        let store = SqliteRunStore::open_with_terminal_secrets(&path, Vec::new()).unwrap();
        assert!(matches!(
            store.resume_point_owned(
                &allowed.run_id,
                &allowed.owner,
                "project-resume-audit-drift",
            ),
            Err(StoreError::Corrupt)
        ));
    };

    connection
        .execute(
            "UPDATE actions SET audit_sequence = ?1 WHERE action_id = ?2",
            rusqlite::params![action_sequence + 1, allowed.action_id.0],
        )
        .unwrap();
    assert_corrupt();
    connection
        .execute(
            "UPDATE actions SET audit_sequence = ?1 WHERE action_id = ?2",
            rusqlite::params![action_sequence, allowed.action_id.0],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE audit_checkpoints SET synced_at = 'drifted' WHERE event_id = ?1",
            rusqlite::params![event_id],
        )
        .unwrap();
    assert_corrupt();
    connection
        .execute(
            "UPDATE audit_checkpoints SET synced_at = ?1 WHERE event_id = ?2",
            rusqlite::params![synced_at, event_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE audit_checkpoints SET head_hash = upper(head_hash) WHERE event_id = ?1",
            rusqlite::params![event_id],
        )
        .unwrap();
    assert_corrupt();
    connection
        .execute(
            "UPDATE audit_checkpoints SET head_hash = ?1 WHERE event_id = ?2",
            rusqlite::params![head_hash, event_id],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE audit_checkpoints SET audit_file = '' WHERE event_id = ?1",
            rusqlite::params![event_id],
        )
        .unwrap();
    assert_corrupt();
    connection
        .execute(
            "UPDATE audit_checkpoints SET audit_file = ?1 WHERE event_id = ?2",
            rusqlite::params![audit_file, event_id],
        )
        .unwrap();
    drop(connection);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn resume_projection_is_owner_scoped_and_omits_terminal_runs() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-created", "owner-a"))
        .unwrap();
    let other = store
        .create_run(new_run("run-resume-other-project", "owner-a"))
        .unwrap();
    let points = store
        .resume_points_owned("owner-a", "project-run-resume-created")
        .unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].run_id, run.run_id);
    assert!(matches!(points[0].next, ResumeNext::StartStep));
    assert_eq!(
        store
            .resume_points_owned("owner-a", "project-run-resume-other-project")
            .unwrap()
            .first()
            .map(|point| &point.run_id),
        Some(&other.run_id)
    );
    assert!(matches!(
        store.resume_point_owned(
            &other.run_id,
            "owner-a",
            "project-run-resume-created",
        ),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store.resume_points_owned("owner-b", "project-run-resume-created"),
        Ok(points) if points.is_empty()
    ));
    assert!(matches!(
        store.resume_point_owned(
            &run.run_id,
            "owner-b",
            "project-run-resume-created",
        ),
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
    assert!(store
        .resume_points_owned("owner-a", "project-run-resume-created")
        .unwrap()
        .is_empty());
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-model")
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-completed")
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
    store
        .append_transcript_record(
            "owner-a",
            &missing.run_id,
            TranscriptRecord::assistant("unrelated same-timestamp text"),
            "2026-07-13T00:00:02Z",
        )
        .unwrap();
    assert!(matches!(
        store.resume_point_owned(&missing.run_id, "owner-a", "project-run-resume-missing",),
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-request")
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-action")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::EvaluatePolicy { ref action_id } if action_id.0 == "action-resume-action"
    ));
}

#[test]
fn recorded_action_rejects_an_unbound_audit_sequence() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-unbound-audit-sequence-{}-{}",
        std::process::id(),
        unix_now()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = SqliteRunStore::open(&path).unwrap();
    let run = store
        .create_run(new_run("run-resume-unbound-audit-sequence", "owner-a"))
        .unwrap();
    start_step(
        &store,
        &run.run_id,
        "step-resume-unbound-audit-sequence",
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-unbound-audit-sequence",
        "model-call-unbound-audit-sequence",
        HarnessEventKind::ModelStarted,
    );
    model_event(
        &store,
        &run.run_id,
        "step-resume-unbound-audit-sequence",
        "model-call-unbound-audit-sequence",
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
                action_id: "action-resume-unbound-audit-sequence".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-resume-unbound-audit-sequence"),
                call_id: CallId::from("provider-resume-unbound-audit-sequence"),
                origin_model_call_id: CallId::from("model-call-unbound-audit-sequence"),
                action_hash: action_hash(&action).unwrap(),
                effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                action,
                occurred_at: "2026-07-13T00:00:03Z".into(),
            },
        )
        .unwrap();
    assert!(store
        .resume_point_owned(
            &run.run_id,
            "owner-a",
            "project-run-resume-unbound-audit-sequence",
        )
        .unwrap()
        .is_some());
    drop(store);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET audit_sequence = 1 WHERE action_id = ?1",
            rusqlite::params!["action-resume-unbound-audit-sequence"],
        )
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        store.resume_point_owned(
            &run.run_id,
            "owner-a",
            "project-run-resume-unbound-audit-sequence",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn combined_empty_model_response_resumes_from_its_bound_action() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-resume-empty-action", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "step-resume-empty-action");
    model_event(
        &store,
        &run.run_id,
        "step-resume-empty-action",
        "model-call-empty-action",
        HarnessEventKind::ModelStarted,
    );
    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    store
        .append_model_completed_with_action(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-resume-empty-action")),
                call_id: Some(CallId::from("model-call-empty-action")),
                occurred_at: "2026-07-13T00:00:03Z".into(),
                kind: HarnessEventKind::ModelCompleted {
                    assistant_text: String::new(),
                },
            },
            ActionRecord {
                action_id: "action-resume-empty-action".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-resume-empty-action"),
                call_id: CallId::from("provider-resume-empty-action"),
                origin_model_call_id: CallId::from("model-call-empty-action"),
                action_hash: action_hash(&action).unwrap(),
                effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                action,
                occurred_at: "2026-07-13T00:00:09Z".into(),
            },
        )
        .unwrap();

    let point = store
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-empty-action")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::EvaluatePolicy { .. }));
}

#[test]
fn resume_rejects_ready_allow_action_without_its_policy_event() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-policy-binding-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let run_id = RunId::from("run-resume-policy-binding");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-resume-policy-binding", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "step-resume-policy-binding");
        model_event(
            &store,
            &run_id,
            "step-resume-policy-binding",
            "model-call-policy-binding",
            HarnessEventKind::ModelStarted,
        );
        model_event(
            &store,
            &run_id,
            "step-resume-policy-binding",
            "model-call-policy-binding",
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
                    action_id: "action-resume-policy-binding".into(),
                    run_id: run_id.clone(),
                    step_id: StepId::from("step-resume-policy-binding"),
                    call_id: CallId::from("provider-resume-policy-binding"),
                    origin_model_call_id: CallId::from("model-call-policy-binding"),
                    action_hash: action_hash(&action).unwrap(),
                    effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                    action,
                    occurred_at: "2026-07-13T00:00:03Z".into(),
                },
            )
            .unwrap();
        store
            .append_event(
                "owner-a",
                &run_id,
                EventAppend {
                    turn_id: Some(TurnId::from("turn-1")),
                    step_id: Some(StepId::from("step-resume-policy-binding")),
                    call_id: None,
                    occurred_at: "2026-07-13T00:00:04Z".into(),
                    kind: HarnessEventKind::PolicyDecided {
                        action_id: "action-resume-policy-binding".into(),
                        decision: orchester_protokoll::PolicyDecision::Allow,
                        rule_id: "workspace.read".into(),
                    },
                },
            )
            .unwrap();
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET policy_event_id = NULL
             WHERE action_id = 'action-resume-policy-binding'",
            [],
        )
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        store.resume_point_owned(&run_id, "owner-a", "project-run-resume-policy-binding",),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn action_resume_rejects_broken_model_and_hash_bindings() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-action-binding-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let run_id = RunId::from("run-resume-action-binding");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-resume-action-binding", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "step-resume-action-binding");
        model_event(
            &store,
            &run_id,
            "step-resume-action-binding",
            "model-call-action-binding",
            HarnessEventKind::ModelStarted,
        );
        model_event(
            &store,
            &run_id,
            "step-resume-action-binding",
            "model-call-action-binding",
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
                    action_id: "action-resume-binding".into(),
                    run_id: run_id.clone(),
                    step_id: StepId::from("step-resume-action-binding"),
                    call_id: CallId::from("provider-resume-binding"),
                    origin_model_call_id: CallId::from("model-call-action-binding"),
                    action_hash: action_hash(&action).unwrap(),
                    effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                    action,
                    occurred_at: "2026-07-13T00:00:03Z".into(),
                },
            )
            .unwrap();
    }

    let assert_corrupt = || {
        let store = SqliteRunStore::open(&path).unwrap();
        assert!(matches!(
            store.resume_point_owned(
                &run_id,
                "owner-a",
                "project-run-resume-action-binding",
            ),
            Err(StoreError::Corrupt)
        ));
    };
    let connection = rusqlite::Connection::open(&path).unwrap();
    let original_hash: String = connection
        .query_row(
            "SELECT action_hash FROM actions WHERE action_id = 'action-resume-binding'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    connection
        .execute(
            "UPDATE actions SET origin_model_call_id = NULL
             WHERE action_id = 'action-resume-binding'",
            [],
        )
        .unwrap();
    assert_corrupt();

    connection
        .execute(
            "UPDATE actions SET origin_model_call_id = 'model-call-action-binding'
             WHERE action_id = 'action-resume-binding'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE steps SET model_phase = 'running'
             WHERE step_id = 'step-resume-action-binding'",
            [],
        )
        .unwrap();
    assert_corrupt();

    connection
        .execute(
            "UPDATE steps SET model_phase = 'completed'
             WHERE step_id = 'step-resume-action-binding'",
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE actions SET action_hash = ?1
             WHERE action_id = 'action-resume-binding'",
            ["0".repeat(64)],
        )
        .unwrap();
    assert_corrupt();

    connection
        .execute(
            "UPDATE actions SET action_hash = ?1
             WHERE action_id = 'action-resume-binding'",
            [original_hash],
        )
        .unwrap();
    drop(connection);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn interrupted_unknown_model_requires_manual_reconciliation() {
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-unknown")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::ManualReconciliation {
            stage: ResumeStage::ModelCall
        }
    ));
}

#[test]
fn interrupted_unknown_tool_requires_manual_reconciliation() {
    let store = Arc::new(SqliteRunStore::in_memory().unwrap());
    let allowed = allowed_run::create_allowed_run(&store, "resume-unknown-tool");
    let audit_path = std::env::temp_dir().join(format!(
        "orchester-resume-unknown-audit-{}-{}.jsonl",
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
    store
        .append_transition(
            &allowed.run_id,
            &allowed.owner,
            Transition::Complete {
                reason: StopReason::InterruptedUnknownOutcome,
                summary: "tool outcome unknown".into(),
                occurred_at: "2026-07-13T00:00:07Z".into(),
            },
        )
        .unwrap();

    let point = store
        .resume_point_owned(
            &allowed.run_id,
            &allowed.owner,
            "project-resume-unknown-tool",
        )
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::ManualReconciliation {
            stage: ResumeStage::ToolOutcome
        }
    ));
    drop(barrier);
    drop(audit);
    std::fs::remove_file(audit_path).ok();
}

#[test]
fn approval_resume_points_distinguish_request_wait_and_capability_recovery() {
    let directory = std::env::temp_dir().join(format!(
        "orchester-resume-approval-binding-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("state.db");
    let store = std::sync::Arc::new(SqliteRunStore::open(&path).unwrap());
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-approval")
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
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-approval")
        .unwrap()
        .unwrap();
    assert!(matches!(
        point.next,
        ResumeNext::AwaitApproval { approval_id: ref id } if id.0 == "approval-resume-approval"
    ));

    approval.approve(&approval_id, "owner-a", &binding).unwrap();
    let point = store
        .resume_point_owned(&run.run_id, "owner-a", "project-run-resume-approval")
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

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE approvals SET config_snapshot_hash = 'drifted-config'
             WHERE approval_id = 'approval-resume-approval'",
            [],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        store.resume_point_owned(&run.run_id, "owner-a", "project-run-resume-approval",),
        Err(StoreError::Corrupt)
    ));
    drop(approval);
    drop(store);
    std::fs::remove_dir_all(directory).unwrap();
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
        .resume_point_owned(&allowed.run_id, &allowed.owner, "project-resume-tool")
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
