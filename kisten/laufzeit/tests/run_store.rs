use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::run_store::{
    ActionRecord, EffectClass, NewRun, RunStatus, RunStore, SqliteRunStore, StoreError, Transition,
};
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

fn start_step(store: &SqliteRunStore, run_id: &RunId, step_id: &str) {
    store
        .append_transition(
            run_id,
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from(step_id),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            },
        )
        .unwrap();
}

#[test]
fn state_transition_and_event_are_atomic_and_sequences_are_contiguous() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-1", "owner-1")).unwrap();

    let event = store
        .append_transition(
            &run.run_id,
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: StepId::from("step-1"),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            },
        )
        .unwrap();

    assert_eq!(event.sequence, 2);
    let events = store.events(&run.run_id).unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let reopened = store.load_run(&run.run_id).unwrap();
    assert_eq!(reopened.status, RunStatus::Running);
    assert_eq!(reopened.current_step_id, Some(StepId::from("step-1")));
    assert_eq!(reopened.next_sequence, 3);
}

#[test]
fn a_step_cannot_record_two_actions_and_failed_insert_adds_no_event() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store.create_run(new_run("run-2", "owner-1")).unwrap();
    start_step(&store, &run.run_id, "step-1");

    let first = ActionRecord {
        action_id: "action-1".into(),
        run_id: run.run_id.clone(),
        step_id: StepId::from("step-1"),
        call_id: CallId::from("call-1"),
        action: AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        },
        action_hash: "hash-1".into(),
        effect_class: EffectClass::ReadOnlyIdempotent,
        occurred_at: "2026-07-12T00:00:02Z".into(),
    };
    store.record_action(first).unwrap();
    let before = store.events(&run.run_id).unwrap();

    let second = ActionRecord {
        action_id: "action-2".into(),
        run_id: run.run_id.clone(),
        step_id: StepId::from("step-1"),
        call_id: CallId::from("call-2"),
        action: AgentAction::Finish {
            summary: "duplicate".into(),
        },
        action_hash: "hash-2".into(),
        effect_class: EffectClass::ReadOnlyIdempotent,
        occurred_at: "2026-07-12T00:00:03Z".into(),
    };
    assert!(matches!(
        store.record_action(second),
        Err(StoreError::Invariant(_))
    ));

    let after = store.events(&run.run_id).unwrap();
    assert_eq!(after, before);
    assert_eq!(
        after.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3]
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
        start_step(&store, &run_id, "step-1");
        store
            .append_transition(
                &run_id,
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
            .events(&run_id)
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
