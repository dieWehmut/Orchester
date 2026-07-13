use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EffectClass, EventAppend, NewRun, RunStore, SqliteRunStore,
    StoreError, TranscriptBindingPhase, Transition,
};
use orchester_laufzeit::harness::transcript::{
    TranscriptCodec, TranscriptLimits, TranscriptRecord,
};
use orchester_protokoll::{AgentAction, HarnessEventKind};
use orchester_protokoll::{CallId, RunId, StepId, TurnId};
use sha2::{Digest, Sha256};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn temporary_database(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "orchester-bindings-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    (root.clone(), root.join("state.db"))
}

fn new_run(id: &str) -> NewRun {
    NewRun {
        run_id: RunId::from(id),
        project_id: format!("project-{id}"),
        owner_actor_id: "owner-bindings".into(),
        canonical_root: format!("/workspace/{id}"),
        workspace_identity: format!("workspace-{id}"),
        policy_snapshot_hash: "policy-bindings".into(),
        config_snapshot_hash: "config-bindings".into(),
        max_steps: 4,
        occurred_at: "2026-07-13T00:00:00Z".into(),
    }
}

fn seed_model_request(store: &SqliteRunStore, id: &str) -> (RunId, u64) {
    let run = store.create_run(new_run(id)).unwrap();
    store
        .append_transition(
            &run.run_id,
            "owner-bindings",
            Transition::StartStep {
                turn_id: TurnId::from(format!("turn-{id}")),
                step_id: StepId::from(format!("step-{id}")),
                occurred_at: "2026-07-13T00:00:01Z".into(),
            },
        )
        .unwrap();
    let event = store
        .append_model_started_with_transcript(
            "owner-bindings",
            &run.run_id,
            EventAppend {
                turn_id: Some(TurnId::from(format!("turn-{id}"))),
                step_id: Some(StepId::from(format!("step-{id}"))),
                call_id: Some(CallId::from(format!("model-{id}"))),
                occurred_at: "2026-07-13T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
            vec![TranscriptRecord::user("inspect the workspace")],
        )
        .unwrap();
    (run.run_id, event.sequence)
}

fn enable_foreign_keys(path: &Path) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    connection
}

fn transcript_hash(wire: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"orchester-transcript-record-v1");
    hasher.update((wire.len() as u64).to_be_bytes());
    hasher.update(wire.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_sqlite_constraint(result: Result<usize, rusqlite::Error>, extended_code: i32) {
    let Err(rusqlite::Error::SqliteFailure(code, _)) = result else {
        panic!("expected SQLite constraint failure, got {result:?}");
    };
    assert_eq!(code.extended_code, extended_code);
}

#[test]
fn binding_rows_enforce_shape_foreign_keys_and_append_only_updates() {
    let (root, path) = temporary_database("constraints");
    let store = SqliteRunStore::open(&path).unwrap();
    let (run_id, event_sequence) = seed_model_request(&store, "constraints");
    let (other_run_id, _) = seed_model_request(&store, "other");
    let other_event = store
        .append_event(
            "owner-bindings",
            &other_run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-other")),
                step_id: Some(StepId::from("step-other")),
                call_id: Some(CallId::from("model-other")),
                occurred_at: "2026-07-13T00:00:03Z".into(),
                kind: HarnessEventKind::ModelCompleted {
                    assistant_text: "other response".into(),
                },
            },
        )
        .unwrap();
    drop(store);

    let connection = enable_foreign_keys(&path);
    assert_sqlite_constraint(
        connection.execute(
            "INSERT OR REPLACE INTO transcript_records(
               run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
             ) SELECT run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
               FROM transcript_records WHERE run_id = ?1 AND ordinal = 1",
            rusqlite::params![run_id.0],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER,
    );
    assert_sqlite_constraint(
        connection.execute(
            "INSERT OR REPLACE INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) SELECT run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
               FROM transcript_bindings
              WHERE run_id = ?1 AND event_sequence = ?2 AND phase = 'model_request'",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER,
    );
    assert_sqlite_constraint(
        connection.execute(
            "UPDATE transcript_bindings
             SET record_count = 0, first_ordinal = NULL, last_ordinal = NULL
             WHERE run_id = ?1 AND event_sequence = ?2 AND phase = 'model_request'",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER,
    );
    assert_sqlite_constraint(
        connection.execute(
            "DELETE FROM transcript_bindings
             WHERE run_id = ?1 AND event_sequence = ?2 AND phase = 'model_request'",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER,
    );
    assert_sqlite_constraint(
        connection.execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'action', 1, 1, 0)",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_CHECK,
    );
    assert_sqlite_constraint(
        connection.execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'action', NULL, NULL, 1)",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_CHECK,
    );
    assert_sqlite_constraint(
        connection.execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'action', 2, 2, 1)",
            rusqlite::params![run_id.0, event_sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY,
    );
    assert_sqlite_constraint(
        connection.execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'model_response', 1, 1, 1)",
            rusqlite::params![run_id.0, other_event.sequence],
        ),
        rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY,
    );

    // A raw writer can still create a structurally valid but semantically wrong
    // phase; the owner-scoped reader must classify that row as corruption.
    connection
        .execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'action', NULL, NULL, 0)",
            rusqlite::params![run_id.0, event_sequence],
        )
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&path).unwrap();
    assert!(store
        .transcript_binding_owned(
            &run_id,
            "owner-bindings",
            event_sequence,
            TranscriptBindingPhase::Action,
        )
        .is_err_and(|error| matches!(error, StoreError::Corrupt)));
    assert!(store
        .transcript_binding_owned(
            &run_id,
            "owner-bindings",
            event_sequence,
            TranscriptBindingPhase::ModelRequest,
        )
        .unwrap()
        .is_some());
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn binding_read_rejects_a_forged_range_even_when_the_row_shape_is_valid() {
    let (root, path) = temporary_database("range");
    let store = SqliteRunStore::open(&path).unwrap();
    let (run_id, event_sequence) = seed_model_request(&store, "range");
    drop(store);

    let connection = enable_foreign_keys(&path);
    let codec = TranscriptCodec::new(TranscriptLimits::default(), Vec::new());
    let forged_wire = codec
        .encode(&TranscriptRecord::assistant("forged"))
        .unwrap();
    let forged_hash = transcript_hash(&forged_wire);
    connection
        .execute(
            "INSERT INTO transcript_records(
               run_id, ordinal, kind, call_id, wire_json, record_hash, created_at
             ) VALUES(
               ?1, 3, 'assistant', NULL,
               ?2, ?3,
               '2026-07-13T00:00:03Z'
             )",
            rusqlite::params![run_id.0, forged_wire, forged_hash],
        )
        .unwrap();
    let forged_event_sequence = event_sequence + 1;
    connection
        .execute(
            "INSERT INTO events(
               run_id, sequence, schema_version, event_id, turn_id, step_id, call_id,
               kind, sanitized_payload, occurred_at
             ) VALUES(?1, ?2, 1, 'forged-binding-event', 'turn-range', 'step-range',
                      'model-forged', 'model.started', '{}', '2026-07-13T00:00:04Z')",
            rusqlite::params![run_id.0, forged_event_sequence],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO transcript_bindings(
               run_id, event_sequence, phase, first_ordinal, last_ordinal, record_count
             ) VALUES(?1, ?2, 'model_request', 1, 3, 3)",
            rusqlite::params![run_id.0, forged_event_sequence],
        )
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&path).unwrap();
    assert!(store
        .transcript_binding_owned(
            &run_id,
            "owner-bindings",
            forged_event_sequence,
            TranscriptBindingPhase::ModelRequest,
        )
        .is_err_and(|error| matches!(error, StoreError::Corrupt)));
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_lifecycle_boundary_rolls_back_event_transcript_and_binding() {
    let (root, path) = temporary_database("rollback");
    let store = SqliteRunStore::open(&path).unwrap();
    let (run_id, _) = seed_model_request(&store, "rollback");
    let before_events = store.events_owned(&run_id, "owner-bindings").unwrap();
    let before_transcript = store.transcript_owned(&run_id, "owner-bindings").unwrap();
    let action = AgentAction::WriteFile {
        path: "src/oversized.txt".into(),
        content: "x".repeat(70 * 1024),
    };
    assert!(matches!(
        store.append_model_completed_with_action(
            "owner-bindings",
            &run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-rollback")),
                step_id: Some(StepId::from("step-rollback")),
                call_id: Some(CallId::from("model-rollback")),
                occurred_at: "2026-07-13T00:00:03Z".into(),
                kind: HarnessEventKind::ModelCompleted {
                    assistant_text: "this transaction must roll back".into(),
                },
            },
            ActionRecord {
                action_id: "action-rollback".into(),
                run_id: run_id.clone(),
                step_id: StepId::from("step-rollback"),
                call_id: CallId::from("provider-rollback"),
                origin_model_call_id: CallId::from("model-rollback"),
                action_hash: action_hash(&action).unwrap(),
                effect_class: EffectClass::WorkspaceMutation,
                action,
                occurred_at: "2026-07-13T00:00:04Z".into(),
            },
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-bindings").unwrap(),
        before_events
    );
    assert_eq!(
        store.transcript_owned(&run_id, "owner-bindings").unwrap(),
        before_transcript
    );
    drop(store);

    let connection = enable_foreign_keys(&path);
    let binding_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM transcript_bindings WHERE run_id = ?1",
            rusqlite::params![run_id.0],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(binding_count, 1);
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}
