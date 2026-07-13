use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EffectClass, EventAppend, NewRun, RunStore, SqliteRunStore,
    StoreError, TranscriptBindingPhase, Transition,
};
use orchester_protokoll::{AgentAction, CallId, HarnessEventKind, RunId, StepId, TurnId};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn temporary_database(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "orchester-action-events-{label}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    (root.clone(), root.join("state.db"))
}

fn new_run(id: &str) -> NewRun {
    NewRun {
        run_id: RunId::from(id),
        project_id: format!("project-{id}"),
        owner_actor_id: "owner-action-events".into(),
        canonical_root: format!("/workspace/{id}"),
        workspace_identity: format!("workspace-{id}"),
        policy_snapshot_hash: PolicyEngine::snapshot_hash(),
        config_snapshot_hash: "config-action-events".into(),
        max_steps: 4,
        occurred_at: "2026-07-14T00:00:00Z".into(),
    }
}

fn seed_action(store: &SqliteRunStore, id: &str, action: AgentAction) -> RunId {
    let run = store.create_run(new_run(id)).unwrap();
    let turn_id = TurnId::from(format!("turn-{id}"));
    let step_id = StepId::from(format!("step-{id}"));
    let model_call_id = CallId::from(format!("model-{id}"));
    store
        .append_transition(
            &run.run_id,
            "owner-action-events",
            Transition::StartStep {
                turn_id: turn_id.clone(),
                step_id: step_id.clone(),
                occurred_at: "2026-07-14T00:00:01Z".into(),
            },
        )
        .unwrap();
    for kind in [
        HarnessEventKind::ModelStarted,
        HarnessEventKind::ModelCompleted {
            assistant_text: String::new(),
        },
    ] {
        store
            .append_event(
                "owner-action-events",
                &run.run_id,
                EventAppend {
                    turn_id: Some(turn_id.clone()),
                    step_id: Some(step_id.clone()),
                    call_id: Some(model_call_id.clone()),
                    occurred_at: "2026-07-14T00:00:02Z".into(),
                    kind,
                },
            )
            .unwrap();
    }
    store
        .record_action(
            "owner-action-events",
            ActionRecord {
                action_id: format!("action-{id}").into(),
                run_id: run.run_id.clone(),
                step_id,
                call_id: format!("provider-{id}").into(),
                origin_model_call_id: model_call_id,
                action_hash: action_hash(&action).unwrap(),
                effect_class: EffectClass::ReadOnlyIdempotent,
                action,
                occurred_at: "2026-07-14T00:00:03Z".into(),
            },
        )
        .unwrap();
    run.run_id
}

#[test]
fn durable_action_event_keeps_full_action_out_of_event_payload() {
    let (root, path) = temporary_database("redaction");
    let action = AgentAction::ReadFile {
        path: "workspace/private-project-input-opaque-123".into(),
        start_line: None,
        end_line: None,
    };
    let expected_summary = action.action_summary();
    let expected_hash = action_hash(&action).unwrap();
    let expected_canonical = serde_json::to_string(&action).unwrap();
    let run_id = {
        let store = SqliteRunStore::open(&path).unwrap();
        seed_action(&store, "redaction", action)
    };

    let connection = rusqlite::Connection::open(&path).unwrap();
    let payload: String = connection
        .query_row(
            "SELECT sanitized_payload FROM events
             WHERE run_id = ?1 AND kind = 'action.recorded'",
            [&run_id.0],
            |row| row.get(0),
        )
        .unwrap();
    let (canonical, stored_hash): (String, String) = connection
        .query_row(
            "SELECT canonical_json, action_hash FROM actions
             WHERE run_id = ?1 AND action_id = 'action-redaction'",
            [&run_id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(!payload.contains("\"action\""));
    assert!(!payload.contains("private-project-input-opaque-123"));
    assert!(payload.contains("action_summary"));
    assert!(payload.contains("action_hash"));
    assert_eq!(canonical, expected_canonical);
    assert_eq!(stored_hash, expected_hash);
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    let restored = reopened
        .events_owned(&run_id, "owner-action-events")
        .unwrap()
        .into_iter()
        .find_map(|event| match event.kind {
            HarnessEventKind::ActionRecorded {
                action_summary,
                action_hash,
                ..
            } => Some((action_summary, action_hash)),
            _ => None,
        })
        .unwrap();
    assert_eq!(restored, (expected_summary, expected_hash));
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn durable_action_event_rejects_tampered_summary_or_hash() {
    let (root, path) = temporary_database("tamper");
    let run_id = {
        let store = SqliteRunStore::open(&path).unwrap();
        seed_action(
            &store,
            "tamper",
            AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            },
        )
    };
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET sanitized_payload =
             '{\"action_id\":\"action-tamper\",\"action_summary\":\"read_file path_bytes=999\",\"action_hash\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"origin_model_call_id\":\"model-tamper\"}'
             WHERE run_id = ?1 AND kind = 'action.recorded'",
            [&run_id.0],
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        reopened.events_owned(&run_id, "owner-action-events"),
        Err(StoreError::Corrupt)
    ));
    assert!(matches!(
        reopened.transcript_binding_owned(
            &run_id,
            "owner-action-events",
            5,
            TranscriptBindingPhase::Action,
        ),
        Err(StoreError::Corrupt)
    ));
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn action_binding_rejects_event_step_drift() {
    let (root, path) = temporary_database("step-tamper");
    let run_id = {
        let store = SqliteRunStore::open(&path).unwrap();
        seed_action(
            &store,
            "step-tamper",
            AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            },
        )
    };
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET step_id = 'forged-step'
             WHERE run_id = ?1 AND kind = 'action.recorded'",
            [&run_id.0],
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    assert!(matches!(
        reopened.events_owned(&run_id, "owner-action-events"),
        Err(StoreError::Corrupt)
    ));
    assert!(matches!(
        reopened.transcript_binding_owned(
            &run_id,
            "owner-action-events",
            5,
            TranscriptBindingPhase::Action,
        ),
        Err(StoreError::Corrupt)
    ));
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_full_action_event_is_verified_and_projected_without_raw_action() {
    let (root, path) = temporary_database("legacy");
    let action = AgentAction::ReadFile {
        path: "workspace/legacy-private-input".into(),
        start_line: None,
        end_line: None,
    };
    let expected_summary = action.action_summary();
    let expected_hash = action_hash(&action).unwrap();
    let run_id = {
        let store = SqliteRunStore::open(&path).unwrap();
        seed_action(&store, "legacy", action.clone())
    };
    let legacy_payload = serde_json::json!({
        "action_id": "action-legacy",
        "action": action,
        "origin_model_call_id": "model-legacy",
    })
    .to_string();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE events SET sanitized_payload = ?1
             WHERE run_id = ?2 AND kind = 'action.recorded'",
            rusqlite::params![legacy_payload, run_id.0],
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteRunStore::open(&path).unwrap();
    let projected = reopened
        .events_owned(&run_id, "owner-action-events")
        .unwrap()
        .into_iter()
        .find(|event| matches!(event.kind, HarnessEventKind::ActionRecorded { .. }))
        .unwrap();
    let encoded = serde_json::to_string(&projected).unwrap();
    assert!(!encoded.contains("legacy-private-input"));
    assert!(!encoded.contains("\"action\""));
    assert!(encoded.contains(&expected_summary));
    assert!(encoded.contains(&expected_hash));
    reopened
        .transcript_binding_owned(
            &run_id,
            "owner-action-events",
            projected.sequence,
            TranscriptBindingPhase::Action,
        )
        .unwrap()
        .unwrap();
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
