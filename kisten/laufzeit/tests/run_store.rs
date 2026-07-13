use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EffectClass, EventAppend, NewRun, RunStatus, RunStore,
    SqliteRunStore, StoreError, StoredTranscriptRecord, Transition,
};
use orchester_laufzeit::harness::transcript::{
    TranscriptCodec, TranscriptLimits, TranscriptRecord,
};
use orchester_protokoll::{AgentAction, CallId, RunId, StepId, StopReason, TurnId};
use orchester_protokoll::{HarnessEvent, HarnessEventKind};
use secrecy::SecretString;
use sha2::{Digest, Sha256};

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

fn append_model_event(
    store: &SqliteRunStore,
    run_id: &RunId,
    owner: &str,
    step_id: &str,
    call_id: &str,
    kind: HarnessEventKind,
) -> Result<HarnessEvent, StoreError> {
    store.append_event(
        owner,
        run_id,
        EventAppend {
            turn_id: Some(TurnId::from("turn-1")),
            step_id: Some(StepId::from(step_id)),
            call_id: Some(CallId::from(call_id)),
            occurred_at: "2026-07-12T00:00:02Z".into(),
            kind,
        },
    )
}

fn test_action_record(
    run_id: &RunId,
    step_id: &str,
    action_id: &str,
    origin_model_call_id: &str,
    provider_call_id: &str,
    action: AgentAction,
) -> ActionRecord {
    ActionRecord {
        action_id: action_id.into(),
        run_id: run_id.clone(),
        step_id: step_id.into(),
        call_id: provider_call_id.into(),
        origin_model_call_id: origin_model_call_id.into(),
        action_hash: action_hash(&action).unwrap(),
        effect_class: EffectClass::ReadOnlyIdempotent,
        action,
        occurred_at: "2026-07-12T00:00:03Z".into(),
    }
}

fn step_model_state(path: &Path, step_id: &str) -> (String, Option<String>) {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .query_row(
            "SELECT model_phase, model_call_id FROM steps WHERE step_id = ?1",
            [step_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn remove_temp_db(path: &Path) {
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
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
fn model_start_persists_running_phase_and_call_id() {
    let path = temp_db("model-start-phase");
    let run_id = RunId::from("run-model-start");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-model-start", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-1",
            HarnessEventKind::ModelStarted,
        )
        .unwrap();
        let before_duplicate = store.events_owned(&run_id, "owner-a").unwrap();
        let duplicate = append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-2",
            HarnessEventKind::ModelStarted,
        );
        assert!(matches!(duplicate, Err(StoreError::Invariant(_))));
        assert_eq!(
            store.events_owned(&run_id, "owner-a").unwrap(),
            before_duplicate
        );
    }

    let state = step_model_state(&path, "step-1");
    remove_temp_db(&path);
    assert_eq!(state, ("running".into(), Some("model-call-1".into())));
}

#[test]
fn provider_call_id_with_configured_secret_is_rejected_atomically() {
    let path = temp_db("provider-call-secret-rejection");
    let run_id = RunId::from("run-provider-call-secret");
    let secret = "configured-provider-call-credential";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-provider-call-secret", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        let before = store.events_owned(&run_id, "owner-a").unwrap();

        assert!(matches!(
            append_model_event(
                &store,
                &run_id,
                "owner-a",
                "step-1",
                &format!("provider-{secret}"),
                HarnessEventKind::ModelStarted,
            ),
            Err(StoreError::Invariant(_))
        ));
        assert_eq!(store.events_owned(&run_id, "owner-a").unwrap(), before);
    }

    assert_eq!(
        step_model_state(&path, "step-1"),
        ("not_started".into(), None)
    );
    remove_temp_db(&path);
}

#[test]
fn policy_rule_with_configured_secret_is_rejected_atomically() {
    let path = temp_db("policy-rule-secret-rejection");
    let run_id = RunId::from("run-policy-rule-secret");
    let secret = "configured-policy-rule-credential";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-policy-rule-secret", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        complete_model(&store, &run_id, "owner-a", "step-1");
        let action = AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        };
        store
            .record_action(
                "owner-a",
                test_action_record(
                    &run_id,
                    "step-1",
                    "action-policy-rule-secret",
                    "model-call-1",
                    "provider-tool-1",
                    action,
                ),
            )
            .unwrap();
        let before = store.events_owned(&run_id, "owner-a").unwrap();

        assert!(matches!(
            store.append_event(
                "owner-a",
                &run_id,
                EventAppend {
                    turn_id: Some(TurnId::from("turn-1")),
                    step_id: Some(StepId::from("step-1")),
                    call_id: None,
                    occurred_at: "2026-07-12T00:00:04Z".into(),
                    kind: HarnessEventKind::PolicyDecided {
                        action_id: "action-policy-rule-secret".into(),
                        decision: orchester_protokoll::PolicyDecision::Allow,
                        rule_id: format!("workspace-{secret}"),
                    },
                },
            ),
            Err(StoreError::Invariant(_))
        ));
        assert_eq!(store.events_owned(&run_id, "owner-a").unwrap(), before);
    }
    remove_temp_db(&path);
}

#[test]
fn model_completion_is_call_bound_atomic_and_single_shot() {
    let path = temp_db("model-completion-cas");
    let run_id = RunId::from("run-model-completion");
    let store = SqliteRunStore::open(&path).unwrap();
    store
        .create_run(new_run("run-model-completion", "owner-a"))
        .unwrap();
    start_step(&store, &run_id, "owner-a", "step-1");
    append_model_event(
        &store,
        &run_id,
        "owner-a",
        "step-1",
        "model-call-1",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();

    let before_wrong_call = store.events_owned(&run_id, "owner-a").unwrap();
    assert!(matches!(
        append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-2",
            HarnessEventKind::ModelCompleted {
                assistant_text: "wrong call".into(),
            },
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-a").unwrap(),
        before_wrong_call
    );
    assert_eq!(
        store
            .load_run_owned(&run_id, "owner-a")
            .unwrap()
            .next_sequence,
        4
    );

    let completed = append_model_event(
        &store,
        &run_id,
        "owner-a",
        "step-1",
        "model-call-1",
        HarnessEventKind::ModelCompleted {
            assistant_text: "done".into(),
        },
    )
    .unwrap();
    assert_eq!(completed.sequence, 4);
    let after_completion = store.events_owned(&run_id, "owner-a").unwrap();
    assert!(matches!(
        append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-1",
            HarnessEventKind::ModelCompleted {
                assistant_text: "duplicate".into(),
            },
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-a").unwrap(),
        after_completion
    );
    assert_eq!(
        store
            .load_run_owned(&run_id, "owner-a")
            .unwrap()
            .next_sequence,
        5
    );
    drop(store);

    let state = step_model_state(&path, "step-1");
    remove_temp_db(&path);
    assert_eq!(state, ("completed".into(), Some("model-call-1".into())));
}

#[test]
fn model_completion_redacts_configured_secrets_at_the_persistence_boundary() {
    let path = temp_db("model-completion-redaction");
    let run_id = RunId::from("run-model-redaction");
    let secret = "configured-model-credential-value";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-model-redaction", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-1",
            HarnessEventKind::ModelStarted,
        )
        .unwrap();

        let event = append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-1",
            HarnessEventKind::ModelCompleted {
                assistant_text: format!("Authorization: Bearer {secret}"),
            },
        )
        .unwrap();
        let event_json = serde_json::to_string(&event).unwrap();
        assert!(!event_json.contains(secret));
        assert!(event_json.contains("[REDACTED]"));

        let connection = rusqlite::Connection::open(&path).unwrap();
        let payload: String = connection
            .query_row(
                "SELECT sanitized_payload FROM events WHERE run_id = ?1 AND kind = 'model.completed'",
                [&run_id.0],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!payload.contains(secret));
        assert!(payload.contains("[REDACTED]"));
        drop(connection);
    }

    let reopened = SqliteRunStore::open(&path).unwrap();
    let event = reopened
        .events_owned(&run_id, "owner-a")
        .unwrap()
        .into_iter()
        .find(|event| matches!(event.kind, HarnessEventKind::ModelCompleted { .. }))
        .unwrap();
    let debug = format!("{event:?}");
    assert!(!debug.contains(secret));
    assert!(debug.contains("[REDACTED]"));
    drop(reopened);

    for entry in std::fs::read_dir(path.parent().unwrap()).unwrap() {
        let entry = entry.unwrap();
        let bytes = std::fs::read(entry.path()).unwrap();
        assert!(
            !String::from_utf8_lossy(&bytes).contains(secret),
            "secret leaked into {:?}",
            entry.path()
        );
    }
    remove_temp_db(&path);
}

#[test]
fn action_with_configured_secret_is_rejected_before_durable_persistence() {
    let path = temp_db("action-secret-rejection");
    let run_id = RunId::from("run-action-secret");
    let secret = "configured-action-credential-value";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-action-secret", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        complete_model(&store, &run_id, "owner-a", "step-1");
        let before = store.events_owned(&run_id, "owner-a").unwrap();
        let action = AgentAction::ReadFile {
            path: format!("src/{secret}.rs"),
            start_line: None,
            end_line: None,
        };

        assert!(matches!(
            store.record_action(
                "owner-a",
                test_action_record(
                    &run_id,
                    "step-1",
                    "action-secret",
                    "model-call-1",
                    "provider-tool-1",
                    action,
                ),
            ),
            Err(StoreError::Invariant(_))
        ));
        assert_eq!(store.events_owned(&run_id, "owner-a").unwrap(), before);

        let connection = rusqlite::Connection::open(&path).unwrap();
        let action_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM actions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(action_count, 0);
        drop(connection);
    }
    remove_temp_db(&path);
}

#[test]
fn action_provider_call_id_with_configured_secret_is_rejected_atomically() {
    let path = temp_db("action-provider-call-secret");
    let run_id = RunId::from("run-action-provider-secret");
    let secret = "configured-action-provider-credential";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-action-provider-secret", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        complete_model(&store, &run_id, "owner-a", "step-1");
        let before = store.events_owned(&run_id, "owner-a").unwrap();
        let action = test_action_record(
            &run_id,
            "step-1",
            "action-provider-secret",
            "model-call-1",
            &format!("provider-{secret}"),
            AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            },
        );

        assert!(matches!(
            store.record_action("owner-a", action),
            Err(StoreError::Invariant(_))
        ));
        assert_eq!(store.events_owned(&run_id, "owner-a").unwrap(), before);
        let connection = rusqlite::Connection::open(&path).unwrap();
        let action_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM actions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(action_count, 0);
        drop(connection);
    }
    remove_temp_db(&path);
}

#[test]
fn run_completion_redacts_configured_secrets_at_the_persistence_boundary() {
    let path = temp_db("run-completion-redaction");
    let run_id = RunId::from("run-completion-redaction");
    let secret = "configured-summary-credential-value";
    {
        let store = SqliteRunStore::open_with_terminal_secrets(
            &path,
            vec![SecretString::new(secret.to_owned().into_boxed_str())],
        )
        .unwrap();
        store
            .create_run(new_run("run-completion-redaction", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");

        let event = store
            .append_transition(
                &run_id,
                "owner-a",
                Transition::Complete {
                    reason: StopReason::Succeeded,
                    summary: format!("completed with {secret}"),
                    occurred_at: "2026-07-12T00:00:03Z".into(),
                },
            )
            .unwrap();
        let event_json = serde_json::to_string(&event).unwrap();
        assert!(!event_json.contains(secret));
        assert!(event_json.contains("[REDACTED]"));

        let connection = rusqlite::Connection::open(&path).unwrap();
        let payload: String = connection
            .query_row(
                "SELECT sanitized_payload FROM events WHERE kind = 'run.completed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!payload.contains(secret));
        assert!(payload.contains("[REDACTED]"));
        drop(connection);
    }

    let reopened = SqliteRunStore::open(&path).unwrap();
    let event = reopened
        .events_owned(&run_id, "owner-a")
        .unwrap()
        .into_iter()
        .find(|event| matches!(event.kind, HarnessEventKind::RunCompleted { .. }))
        .unwrap();
    assert!(!format!("{event:?}").contains(secret));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn concurrent_model_completion_has_one_winner() {
    let path = temp_db("concurrent-model-completion");
    let run_id = RunId::from("run-concurrent-model");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-concurrent-model", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        append_model_event(
            &store,
            &run_id,
            "owner-a",
            "step-1",
            "model-call-1",
            HarnessEventKind::ModelStarted,
        )
        .unwrap();
    }

    let start = Arc::new(Barrier::new(2));
    let stores = [
        SqliteRunStore::open(&path).unwrap(),
        SqliteRunStore::open(&path).unwrap(),
    ];
    let mut completions = Vec::new();
    for store in stores {
        let run_id = run_id.clone();
        let start = start.clone();
        completions.push(thread::spawn(move || {
            start.wait();
            append_model_event(
                &store,
                &run_id,
                "owner-a",
                "step-1",
                "model-call-1",
                HarnessEventKind::ModelCompleted {
                    assistant_text: "done".into(),
                },
            )
        }));
    }
    let outcomes = completions
        .into_iter()
        .map(|completion| completion.join().unwrap())
        .collect::<Vec<_>>();

    let reopened = SqliteRunStore::open(&path).unwrap();
    let events = reopened.events_owned(&run_id, "owner-a").unwrap();
    let completion_events = events
        .iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ModelCompleted { .. }))
        .count();
    drop(reopened);
    let state = step_model_state(&path, "step-1");
    remove_temp_db(&path);

    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        outcomes.iter().filter(|outcome| outcome.is_err()).count(),
        1
    );
    assert!(outcomes
        .iter()
        .any(|outcome| matches!(outcome, Err(StoreError::Invariant(_)))));
    assert_eq!(completion_events, 1);
    assert_eq!(state, ("completed".into(), Some("model-call-1".into())));
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
                origin_model_call_id: CallId::from("model-call-1"),
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
        origin_model_call_id: CallId::from("model-call-1"),
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
        origin_model_call_id: CallId::from("model-call-1"),
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
fn action_before_model_completion_is_rejected_atomically() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-action-before-model", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-1");
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-1",
        "model-call-1",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    let action = test_action_record(
        &run.run_id,
        "step-1",
        "action-before-model",
        "model-call-1",
        "provider-tool-1",
        AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        },
    );

    assert!(matches!(
        store.record_action("owner-a", action),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert_eq!(
        store
            .load_run_owned(&run.run_id, "owner-a")
            .unwrap()
            .next_sequence,
        4
    );
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-1",
        "model-call-1",
        HarnessEventKind::ModelCompleted {
            assistant_text: "done".into(),
        },
    )
    .unwrap();
    store
        .record_action(
            "owner-a",
            test_action_record(
                &run.run_id,
                "step-1",
                "action-after-model",
                "model-call-1",
                "provider-tool-after-model",
                AgentAction::ReadFile {
                    path: "src/lib.rs".into(),
                    start_line: None,
                    end_line: None,
                },
            ),
        )
        .unwrap();
}

#[test]
fn action_rejects_a_different_origin_model_call_id() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-action-origin", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-1");
    complete_model(&store, &run.run_id, "owner-a", "step-1");
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    let action = test_action_record(
        &run.run_id,
        "step-1",
        "action-wrong-origin",
        "model-call-2",
        "provider-tool-1",
        AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        },
    );

    assert!(matches!(
        store.record_action("owner-a", action),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert_eq!(
        store
            .load_run_owned(&run.run_id, "owner-a")
            .unwrap()
            .next_sequence,
        5
    );
}

#[test]
fn action_accepts_completed_origin_with_distinct_provider_call_id() {
    let path = temp_db("action-origin-valid");
    let run_id = RunId::from("run-action-origin-valid");
    let (sequence, event_call_id, event_origin, is_action_event) = {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-action-origin-valid", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        complete_model(&store, &run_id, "owner-a", "step-1");
        let event = store
            .record_action(
                "owner-a",
                test_action_record(
                    &run_id,
                    "step-1",
                    "action-valid-origin",
                    "model-call-1",
                    "provider-tool-1",
                    AgentAction::ReadFile {
                        path: "src/lib.rs".into(),
                        start_line: None,
                        end_line: None,
                    },
                ),
            )
            .unwrap();
        (
            event.sequence,
            event.call_id,
            match &event.kind {
                HarnessEventKind::ActionRecorded {
                    origin_model_call_id,
                    ..
                } => origin_model_call_id.clone(),
                _ => None,
            },
            matches!(&event.kind, HarnessEventKind::ActionRecorded { .. }),
        )
    };
    let connection = rusqlite::Connection::open(&path).unwrap();
    let binding: (String, String) = connection
        .query_row(
            "SELECT call_id, origin_model_call_id
             FROM actions WHERE action_id = 'action-valid-origin'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    drop(connection);
    remove_temp_db(&path);

    assert_eq!(sequence, 5);
    assert_eq!(event_call_id, Some("provider-tool-1".into()));
    assert_eq!(event_origin, Some("model-call-1".into()));
    assert!(is_action_event);
    assert_eq!(binding, ("provider-tool-1".into(), "model-call-1".into()));
}

#[test]
fn legacy_action_without_completed_origin_cannot_reenter_policy_or_execution() {
    let path = temp_db("legacy-action-origin");
    let run_id = RunId::from("run-legacy-action-origin");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-legacy-action-origin", "owner-a"))
            .unwrap();
        start_step(&store, &run_id, "owner-a", "step-1");
        complete_model(&store, &run_id, "owner-a", "step-1");
        store
            .record_action(
                "owner-a",
                test_action_record(
                    &run_id,
                    "step-1",
                    "legacy-action",
                    "model-call-1",
                    "provider-tool-1",
                    AgentAction::ReadFile {
                        path: "src/lib.rs".into(),
                        start_line: None,
                        end_line: None,
                    },
                ),
            )
            .unwrap();
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "UPDATE steps SET model_phase = 'running' WHERE step_id = 'step-1';
             UPDATE actions SET origin_model_call_id = NULL
             WHERE action_id = 'legacy-action';",
        )
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&path).unwrap();
    let before = store.events_owned(&run_id, "owner-a").unwrap();
    assert!(matches!(
        store.append_event(
            "owner-a",
            &run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-1")),
                call_id: None,
                occurred_at: "2026-07-12T00:00:04Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id: "legacy-action".into(),
                    decision: orchester_protokoll::PolicyDecision::Ask,
                    rule_id: "workspace.read".into(),
                },
            },
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run_id, "owner-a").unwrap(), before);
    assert!(matches!(
        store.execution_candidate("owner-a", &run_id, &"legacy-action".into()),
        Err(StoreError::NotFound)
    ));
    drop(store);
    remove_temp_db(&path);
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
fn project_identity_is_stable_and_owner_bound_across_runs() {
    let store = SqliteRunStore::in_memory().unwrap();
    let mut first = new_run("run-project-first", "owner-a");
    first.project_id = "project-shared".into();
    first.canonical_root = "/workspace/shared".into();
    first.workspace_identity = "workspace-shared".into();
    store.create_run(first).unwrap();

    let mut same = new_run("run-project-second", "owner-a");
    same.project_id = "project-shared".into();
    same.canonical_root = "/workspace/shared".into();
    same.workspace_identity = "workspace-shared".into();
    store.create_run(same).unwrap();

    let mut drifted = new_run("run-project-drifted", "owner-a");
    drifted.project_id = "project-shared".into();
    drifted.canonical_root = "/workspace/other".into();
    drifted.workspace_identity = "workspace-other".into();
    assert!(matches!(
        store.create_run(drifted),
        Err(StoreError::Invariant(_))
    ));

    let mut foreign = new_run("run-project-foreign", "owner-b");
    foreign.project_id = "project-shared".into();
    foreign.canonical_root = "/workspace/shared".into();
    foreign.workspace_identity = "workspace-shared".into();
    assert!(matches!(
        store.create_run(foreign),
        Err(StoreError::Invariant(_))
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
    let parent = path.parent().unwrap().to_path_buf();
    std::fs::remove_file(&path).ok();
    std::fs::remove_dir(parent).ok();
}

#[test]
fn transcript_records_are_owner_scoped_canonical_and_recover_after_reopen() {
    let path = temp_db("transcript-reopen");
    let run_id = RunId::from("run-transcript");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-transcript", "owner-a"))
            .unwrap();
        assert_eq!(
            store
                .append_transcript_record(
                    "owner-a",
                    &run_id,
                    TranscriptRecord::user("inspect the workspace"),
                    "2026-07-12T00:00:01Z",
                )
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .append_transcript_record(
                    "owner-a",
                    &run_id,
                    TranscriptRecord::tool_call(
                        "call-1",
                        "read_file",
                        r#"{ "path": "src/lib.rs" }"#,
                    ),
                    "2026-07-12T00:00:02Z",
                )
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .append_transcript_record(
                    "owner-a",
                    &run_id,
                    TranscriptRecord::tool_result("call-1", "bounded output"),
                    "2026-07-12T00:00:03Z",
                )
                .unwrap(),
            3
        );
    }

    let reopened = SqliteRunStore::open(&path).unwrap();
    let records = reopened.transcript_owned(&run_id, "owner-a").unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].ordinal, 1);
    assert!(matches!(
        &records[1],
        StoredTranscriptRecord {
            ordinal: 2,
            record: TranscriptRecord::ToolCall { arguments_json, .. },
            ..
        } if arguments_json == r#"{"path":"src/lib.rs"}"#
    ));
    assert!(matches!(
        &records[2].record,
        TranscriptRecord::ToolResult { call_id, output }
            if call_id.0 == "call-1" && output == "bounded output"
    ));
    assert!(matches!(
        reopened.transcript_owned(&run_id, "owner-b"),
        Err(StoreError::NotFound)
    ));

    drop(reopened);
    let parent = path.parent().unwrap().to_path_buf();
    std::fs::remove_file(&path).ok();
    std::fs::remove_dir(parent).ok();
}

#[test]
fn transcript_append_rejects_unpaired_result_without_partial_write() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-transcript-sequence", "owner-a"))
        .unwrap();
    store
        .append_transcript_record(
            "owner-a",
            &run.run_id,
            TranscriptRecord::user("inspect the workspace"),
            "2026-07-12T00:00:01Z",
        )
        .unwrap();

    assert!(matches!(
        store.append_transcript_record(
            "owner-a",
            &run.run_id,
            TranscriptRecord::tool_result("missing-call", "orphan"),
            "2026-07-12T00:00:02Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    let records = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ordinal, 1);
}

#[test]
fn transcript_text_is_sanitized_and_tool_arguments_reject_escaped_secrets() {
    let secret = "provider-transcript-secret";
    let path = std::env::temp_dir().join(format!(
        "orchester-transcript-secret-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let store = SqliteRunStore::open_with_terminal_secrets(
        &path,
        vec![SecretString::new(secret.to_owned().into_boxed_str())],
    )
    .unwrap();
    let run = store
        .create_run(new_run("run-transcript-secret", "owner-a"))
        .unwrap();
    store
        .append_transcript_record(
            "owner-a",
            &run.run_id,
            TranscriptRecord::assistant(format!("answer {secret}")),
            "2026-07-12T00:00:01Z",
        )
        .unwrap();
    assert!(matches!(
        store.append_transcript_record(
            "owner-a",
            &run.run_id,
            TranscriptRecord::tool_call(
                "call-escaped",
                "read_file",
                r#"{"path":"provider-\u0074ranscript-secret"}"#,
            ),
            "2026-07-12T00:00:02Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    let records = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        &records[0].record,
        TranscriptRecord::Assistant(text)
            if text == "answer [REDACTED]"
    ));
    drop(store);
    std::fs::remove_file(path).ok();
}

#[test]
fn transcript_append_classifies_existing_noncanonical_wire_as_corrupt() {
    let path = temp_db("transcript-corrupt-wire");
    let run_id = RunId::from("run-transcript-corrupt");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-transcript-corrupt", "owner-a"))
            .unwrap();
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO transcript_records(
               run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
             ) VALUES(?1, 1, 'user', NULL, ?2, ?3, ?4)",
            rusqlite::params![
                run_id.0,
                r#"{ "kind": "user", "text": "noncanonical" }"#,
                "a".repeat(64),
                "2026-07-12T00:00:01Z",
            ],
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        reopened.append_transcript_record(
            "owner-a",
            &run_id,
            TranscriptRecord::user("next"),
            "2026-07-12T00:00:02Z",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn transcript_reader_rejects_a_cross_record_call_id_mismatch() {
    let path = temp_db("transcript-corrupt-sequence");
    let run_id = RunId::from("run-transcript-corrupt-sequence");
    {
        let store = SqliteRunStore::open(&path).unwrap();
        store
            .create_run(new_run("run-transcript-corrupt-sequence", "owner-a"))
            .unwrap();
    }

    let codec = TranscriptCodec::new(TranscriptLimits::default(), Vec::new());
    let records = [
        TranscriptRecord::tool_call("call-a", "read_file", r#"{"path":"src/lib.rs"}"#),
        TranscriptRecord::tool_result("call-b", "mismatched result"),
    ];
    let connection = rusqlite::Connection::open(&path).unwrap();
    for (index, record) in records.iter().enumerate() {
        let wire = codec.encode(record).unwrap();
        let (kind, call_id) = match record {
            TranscriptRecord::ToolCall { call_id, .. } => ("tool_call", call_id.0.as_str()),
            TranscriptRecord::ToolResult { call_id, .. } => ("tool_result", call_id.0.as_str()),
            _ => unreachable!(),
        };
        let mut hasher = Sha256::new();
        hasher.update(b"orchester-transcript-record-v1");
        hasher.update((wire.len() as u64).to_be_bytes());
        hasher.update(wire.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        connection
            .execute(
                "INSERT INTO transcript_records(
                   run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
                 ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    run_id.0,
                    i64::try_from(index + 1).unwrap(),
                    kind,
                    call_id,
                    wire,
                    hash,
                    "2026-07-12T00:00:01Z",
                ],
            )
            .unwrap();
    }
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        reopened.transcript_owned(&run_id, "owner-a"),
        Err(StoreError::Corrupt)
    ));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn transcript_batch_append_is_atomic_and_returns_one_contiguous_range() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-transcript-batch", "owner-a"))
        .unwrap();
    store
        .append_transcript_record(
            "owner-a",
            &run.run_id,
            TranscriptRecord::user("inspect"),
            "2026-07-12T00:00:01Z",
        )
        .unwrap();

    let appended = store
        .append_transcript_records(
            "owner-a",
            &run.run_id,
            vec![
                TranscriptRecord::assistant("reading the file"),
                TranscriptRecord::tool_call("call-batch", "read_file", r#"{"path":"src/lib.rs"}"#),
            ],
            "2026-07-12T00:00:02Z",
        )
        .unwrap();
    assert_eq!(appended.first_ordinal, 2);
    assert_eq!(appended.last_ordinal, 3);

    let before = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        store.append_transcript_records(
            "owner-a",
            &run.run_id,
            vec![
                TranscriptRecord::tool_result("wrong-call", "invalid"),
                TranscriptRecord::assistant("must not persist"),
            ],
            "2026-07-12T00:00:03Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.transcript_owned(&run.run_id, "owner-a").unwrap(),
        before
    );
}

#[test]
fn model_completion_writes_assistant_transcript_in_the_same_transaction() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-transcript", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-model-transcript");
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-transcript",
        "model-call-transcript",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-transcript",
        "model-call-transcript",
        HarnessEventKind::ModelCompleted {
            assistant_text: "bounded model answer".into(),
        },
    )
    .unwrap();

    store
        .record_action(
            "owner-a",
            test_action_record(
                &run.run_id,
                "step-model-transcript",
                "action-model-transcript",
                "model-call-transcript",
                "provider-call-transcript",
                AgentAction::ReadFile {
                    path: "src/lib.rs".into(),
                    start_line: None,
                    end_line: None,
                },
            ),
        )
        .unwrap();

    let records = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert_eq!(records.len(), 2);
    assert!(matches!(
        &records[0],
        StoredTranscriptRecord {
            ordinal: 1,
            record: TranscriptRecord::Assistant(text),
            ..
        } if text == "bounded model answer"
    ));
    assert!(matches!(
        &records[1],
        StoredTranscriptRecord {
            ordinal: 2,
            record: TranscriptRecord::ToolCall { call_id, name, arguments_json },
            ..
        } if call_id.0 == "provider-call-transcript"
            && name == "read_file"
            && arguments_json == r#"{"end_line":null,"path":"src/lib.rs","start_line":null}"#
    ));
}

#[test]
fn oversized_model_completion_rolls_back_event_and_transcript_together() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-transcript-limit", "owner-a"))
        .unwrap();
    start_step(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-transcript-limit",
    );
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-transcript-limit",
        "model-call-limit",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        append_model_event(
            &store,
            &run.run_id,
            "owner-a",
            "step-model-transcript-limit",
            "model-call-limit",
            HarnessEventKind::ModelCompleted {
                assistant_text: "x".repeat(32 * 1024 + 1),
            },
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert!(store
        .transcript_owned(&run.run_id, "owner-a")
        .unwrap()
        .is_empty());
}

#[test]
fn oversized_action_arguments_roll_back_action_and_tool_call_transcript() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-action-transcript-limit", "owner-a"))
        .unwrap();
    start_step(
        &store,
        &run.run_id,
        "owner-a",
        "step-action-transcript-limit",
    );
    complete_model(
        &store,
        &run.run_id,
        "owner-a",
        "step-action-transcript-limit",
    );
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    let action = AgentAction::WriteFile {
        path: "src/large.txt".into(),
        content: "x".repeat(70 * 1024),
    };
    let record = ActionRecord {
        action_id: "action-transcript-limit".into(),
        run_id: run.run_id.clone(),
        step_id: StepId::from("step-action-transcript-limit"),
        call_id: CallId::from("provider-action-transcript-limit"),
        origin_model_call_id: CallId::from("model-call-1"),
        action_hash: action_hash(&action).unwrap(),
        effect_class: EffectClass::WorkspaceMutation,
        action,
        occurred_at: "2026-07-12T00:00:03Z".into(),
    };
    assert!(matches!(
        store.record_action("owner-a", record),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert!(store
        .transcript_owned(&run.run_id, "owner-a")
        .unwrap()
        .is_empty());
}

#[test]
fn model_start_writes_new_request_records_in_the_same_transaction() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-start-transcript", "owner-a"))
        .unwrap();
    start_step(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-start-transcript",
    );
    let input = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from("step-model-start-transcript")),
        call_id: Some(CallId::from("model-call-start-transcript")),
        occurred_at: "2026-07-12T00:00:02Z".into(),
        kind: HarnessEventKind::ModelStarted,
    };
    store
        .append_model_started_with_transcript(
            "owner-a",
            &run.run_id,
            input,
            vec![TranscriptRecord::user("inspect the workspace")],
        )
        .unwrap();

    let records = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        &records[..],
        [StoredTranscriptRecord {
            ordinal: 1,
            record: TranscriptRecord::User(text),
            ..
        }] if text == "inspect the workspace"
    ));
}

#[test]
fn model_start_rejects_an_unclosed_request_transcript_atomically() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-start-open", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-model-start-open");
    let input = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from("step-model-start-open")),
        call_id: Some(CallId::from("model-call-start-open")),
        occurred_at: "2026-07-12T00:00:02Z".into(),
        kind: HarnessEventKind::ModelStarted,
    };
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        store.append_model_started_with_transcript(
            "owner-a",
            &run.run_id,
            input,
            vec![
                TranscriptRecord::user("inspect the workspace"),
                TranscriptRecord::tool_call(
                    "call-open",
                    "read_file",
                    r#"{"path":"src/lib.rs"}"#,
                ),
            ],
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert!(store
        .transcript_owned(&run.run_id, "owner-a")
        .unwrap()
        .is_empty());

    store
        .append_transcript_records(
            "owner-a",
            &run.run_id,
            vec![
                TranscriptRecord::user("inspect the workspace"),
                TranscriptRecord::tool_call(
                    "call-existing-open",
                    "read_file",
                    r#"{"path":"src/lib.rs"}"#,
                ),
            ],
            "2026-07-12T00:00:03Z",
        )
        .unwrap();
    assert!(matches!(
        store.append_model_started_with_transcript(
            "owner-a",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-model-start-open")),
                call_id: Some(CallId::from("model-call-start-open-2")),
                occurred_at: "2026-07-12T00:00:04Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
            Vec::new(),
        ),
        Err(StoreError::Invariant(_))
    ));
}

#[test]
fn oversized_model_start_input_rolls_back_event_phase_and_transcript() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-start-limit", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-model-start-limit");
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    let input = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from("step-model-start-limit")),
        call_id: Some(CallId::from("model-call-start-limit")),
        occurred_at: "2026-07-12T00:00:02Z".into(),
        kind: HarnessEventKind::ModelStarted,
    };
    assert!(matches!(
        store.append_model_started_with_transcript(
            "owner-a",
            &run.run_id,
            input,
            vec![TranscriptRecord::user("x".repeat(32 * 1024 + 1))],
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert!(store
        .transcript_owned(&run.run_id, "owner-a")
        .unwrap()
        .is_empty());
}

#[test]
fn model_completion_with_action_commits_response_and_tool_call_as_one_boundary() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-action-boundary", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-model-action-boundary");
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-action-boundary",
        "model-call-action-boundary",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();
    let action = test_action_record(
        &run.run_id,
        "step-model-action-boundary",
        "action-model-action-boundary",
        "model-call-action-boundary",
        "provider-model-action-boundary",
        AgentAction::ReadFile {
            path: "src/lib.rs".into(),
            start_line: None,
            end_line: None,
        },
    );
    let completion = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from("step-model-action-boundary")),
        call_id: Some(CallId::from("model-call-action-boundary")),
        occurred_at: "2026-07-12T00:00:03Z".into(),
        kind: HarnessEventKind::ModelCompleted {
            assistant_text: "I will inspect the file".into(),
        },
    };
    let (model_event, action_event) = store
        .append_model_completed_with_action("owner-a", &run.run_id, completion, action)
        .unwrap();
    assert!(matches!(
        model_event.kind,
        HarnessEventKind::ModelCompleted { .. }
    ));
    assert!(matches!(
        action_event.kind,
        HarnessEventKind::ActionRecorded { .. }
    ));
    let records = store.transcript_owned(&run.run_id, "owner-a").unwrap();
    assert!(matches!(
        &records[..],
        [
            StoredTranscriptRecord {
                record: TranscriptRecord::Assistant(_),
                ..
            },
            StoredTranscriptRecord {
                record: TranscriptRecord::ToolCall { .. },
                ..
            }
        ]
    ));
}

#[test]
fn invalid_combined_model_action_rolls_back_model_completion_and_transcript() {
    let store = SqliteRunStore::in_memory().unwrap();
    let run = store
        .create_run(new_run("run-model-action-rollback", "owner-a"))
        .unwrap();
    start_step(&store, &run.run_id, "owner-a", "step-model-action-rollback");
    append_model_event(
        &store,
        &run.run_id,
        "owner-a",
        "step-model-action-rollback",
        "model-call-action-rollback",
        HarnessEventKind::ModelStarted,
    )
    .unwrap();
    let before = store.events_owned(&run.run_id, "owner-a").unwrap();
    let action = test_action_record(
        &run.run_id,
        "step-model-action-rollback",
        "action-model-action-rollback",
        "model-call-action-rollback",
        "provider-model-action-rollback",
        AgentAction::WriteFile {
            path: "src/large.txt".into(),
            content: "x".repeat(70 * 1024),
        },
    );
    let completion = EventAppend {
        turn_id: Some(TurnId::from("turn-1")),
        step_id: Some(StepId::from("step-model-action-rollback")),
        call_id: Some(CallId::from("model-call-action-rollback")),
        occurred_at: "2026-07-12T00:00:03Z".into(),
        kind: HarnessEventKind::ModelCompleted {
            assistant_text: "must roll back".into(),
        },
    };
    assert!(matches!(
        store.append_model_completed_with_action("owner-a", &run.run_id, completion, action),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(store.events_owned(&run.run_id, "owner-a").unwrap(), before);
    assert!(store
        .transcript_owned(&run.run_id, "owner-a")
        .unwrap()
        .is_empty());
}

fn temp_db(label: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "orchester-state-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
        .join("state.db")
}
