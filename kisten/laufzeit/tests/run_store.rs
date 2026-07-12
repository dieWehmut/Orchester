use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EffectClass, EventAppend, NewRun, RunStatus, RunStore,
    SqliteRunStore, StoreError, Transition,
};
use orchester_protokoll::HarnessEventKind;
use orchester_protokoll::{AgentAction, CallId, RunId, StepId, StopReason, TurnId};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn new_run(id: &str, owner: &str) -> NewRun {
    NewRun {
        run_id: RunId::from(id),
        project_id: format!("project-{id}"),
        owner_actor_id: owner.to_owned(),
        canonical_root: format!("/workspace/{id}"),
        workspace_identity: format!("workspace-{id}"),
        policy_snapshot_hash: "policy-v1".into(),
        config_snapshot_hash: "config-v1".into(),
        max_steps: 8,
        occurred_at: "2026-07-12T00:00:00Z".into(),
    }
}

fn start_step(store: &SqliteRunStore, run_id: &RunId, owner: &str, step_id: &str) {
    store
        .append_transition(
            run_id,
            owner,
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from(step_id),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            },
        )
        .unwrap();
}

fn complete_model(store: &SqliteRunStore, run_id: &RunId, owner: &str, step_id: &str) {
    for kind in [
        HarnessEventKind::ModelStarted,
        HarnessEventKind::ModelCompleted {
            assistant_text: String::new(),
        },
    ] {
        store
            .append_event(
                owner,
                run_id,
                EventAppend {
                    turn_id: Some(TurnId::from("turn-1")),
                    step_id: Some(StepId::from(step_id)),
                    call_id: Some(CallId::from("model-call-1")),
                    occurred_at: "2026-07-12T00:00:02Z".into(),
                    kind,
                },
            )
            .unwrap();
    }
}

#[test]
fn state_transition_and_event_are_atomic_and_sequences_are_contiguous() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-1", "owner-1")).unwrap();

    let event = store
        .append_transition(
            &run.run_id,
            "owner-1",
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from("step-1"),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            },
        )
        .unwrap();

    assert_eq!(event.sequence, 2);
    let events = store.events_owned(&run.run_id, "owner-1").unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let reopened = store.load_run_owned(&run.run_id, "owner-1").unwrap();
    assert_eq!(reopened.status, RunStatus::Running);
    assert_eq!(reopened.current_step_id, Some(StepId::from("step-1")));
    assert_eq!(reopened.next_sequence, 3);

    let model_event = store
        .append_event(
            "owner-1",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-1")),
                call_id: Some(CallId::from("model-call-1")),
                occurred_at: "2026-07-12T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
        )
        .unwrap();
    assert_eq!(model_event.sequence, 3);
    assert_eq!(
        store
            .load_run_owned(&run.run_id, "owner-1")
            .unwrap()
            .next_sequence,
        4
    );
}

#[test]
fn every_write_requires_the_run_owner_and_approval_pause_blocks_new_steps() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-owner", "owner-a")).unwrap();
    assert!(matches!(
        store.append_transition(
            &run.run_id,
            "owner-b",
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from("step-1"),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            }
        ),
        Err(StoreError::NotFound)
    ));

    start_step(&store, &run.run_id, "owner-a", "step-1");
    complete_model(&store, &run.run_id, "owner-a", "step-1");
    let action = AgentAction::RunCommand {
        program: "git".into(),
        args: vec!["commit".into()],
        cwd: None,
    };
    store
        .record_action(
            "owner-a",
            ActionRecord {
                action_id: "approval-action".into(),
                run_id: run.run_id.clone(),
                step_id: StepId::from("step-1"),
                call_id: CallId::from("action-call-1"),
                action: action.clone(),
                action_hash: action_hash(&action).unwrap(),
                effect_class: EffectClass::WorkspaceMutation,
                occurred_at: "2026-07-12T00:00:02Z".into(),
            },
        )
        .unwrap();
    store
        .append_event(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-1")),
                call_id: Some(CallId::from("action-call-1")),
                occurred_at: "2026-07-12T00:00:02Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id: "approval-action".into(),
                    decision: orchester_protokoll::PolicyDecision::Ask,
                    rule_id: "git.write".into(),
                },
            },
        )
        .unwrap();
    assert!(matches!(
        store.append_transition(
            &run.run_id,
            "owner-a",
            Transition::StartStep {
                turn_id: TurnId::from("turn-2"),
                step_id: StepId::from("step-2"),
                occurred_at: "2026-07-12T00:00:03Z".into(),
            }
        ),
        Err(StoreError::Invariant(_))
    ));
}

#[test]
fn a_step_cannot_record_two_actions_and_failed_insert_adds_no_event() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-2", "owner-1")).unwrap();
    start_step(&store, &run.run_id, "owner-1", "step-1");
    complete_model(&store, &run.run_id, "owner-1", "step-1");

    let first_action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let first = ActionRecord {
        action_id: "action-1".into(),
        run_id: run.run_id.clone(),
        step_id: StepId::from("step-1"),
        call_id: CallId::from("call-1"),
        action: first_action.clone(),
        action_hash: action_hash(&first_action).unwrap(),
        effect_class: EffectClass::ReadOnlyIdempotent,
        occurred_at: "2026-07-12T00:00:02Z".into(),
    };
    store.record_action("owner-1", first).unwrap();
    let before = store.events_owned(&run.run_id, "owner-1").unwrap();

    let second_action = AgentAction::Finish {
        summary: "duplicate".into(),
    };
    let second = ActionRecord {
        action_id: "action-2".into(),
        run_id: run.run_id.clone(),
        step_id: StepId::from("step-1"),
        call_id: CallId::from("call-2"),
        action: second_action.clone(),
        action_hash: action_hash(&second_action).unwrap(),
        effect_class: EffectClass::ReadOnlyIdempotent,
        occurred_at: "2026-07-12T00:00:03Z".into(),
    };
    assert!(matches!(
        store.record_action("owner-1", second),
        Err(StoreError::Invariant(_))
    ));

    let after = store.events_owned(&run.run_id, "owner-1").unwrap();
    assert_eq!(after, before);
    assert_eq!(
        after.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
}

#[test]
fn owner_scoped_lookup_does_not_reveal_foreign_run() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-3", "owner-a")).unwrap();

    assert_eq!(
        store.load_run_owned(&run.run_id, "owner-a").unwrap().run_id,
        run.run_id
    );
    assert!(matches!(
        store.load_run_owned(&run.run_id, "owner-b"),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store.events_owned(&run.run_id, "owner-b"),
        Err(StoreError::NotFound)
    ));
}

#[test]
fn on_disk_store_recovers_terminal_state_and_exact_events() {
    let path = temp_db("reopen");
    let run_id = RunId::from("run-4");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store.create_run(new_run("run-4", "owner-a")).unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        store
            .append_transition(
                &run_id,
                "owner-a",
                Transition::Complete {
                    reason: StopReason::Succeeded,
                    summary: "done".into(),
                    occurred_at: "2026-07-12T00:00:03Z".into(),
                },
            )
            .unwrap();
    }

    let reopened = SqliteRunStore::open(&path).unwrap();
    let run = reopened.load_run_owned(&run_id, "owner-a").unwrap();
    assert_eq!(run.status, RunStatus::Succeeded);
    assert_eq!(
        reopened
            .events_owned(&run_id, "owner-a")
            .unwrap()
            .iter()
            .map(|event| event.kind_name())
            .collect::<Vec<_>>(),
        vec!["run.created", "step.started", "run.completed"]
    );
    assert!(reopened.foreign_key_violations().unwrap().is_empty());

    drop(reopened);
    std::fs::remove_file(path).ok();
}

fn temp_db(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "orchester-{label}-{}-{}.db",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}
