use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, ResumeNext, RunStore, SqliteRunStore,
    StoreError, Transition,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_protokoll::{
    ActionId, AgentAction, CallId, HarnessEventKind, PolicyDecision, RunId, StepId, TurnId,
};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn temp_db(label: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "orchester-policy-store-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
        .join("state.db")
}

fn remove_temp_db(path: &Path) {
    let _ = std::fs::remove_dir_all(path.parent().expect("database parent"));
}

fn seed_action(action: AgentAction, label: &str) -> (PathBuf, SqliteRunStore, RunId, ActionId) {
    let path = temp_db(label);
    let store = SqliteRunStore::open(&path).unwrap();
    let run_id = RunId::from(format!("run-{label}"));
    let step_id = StepId::from(format!("step-{label}"));
    let owner = format!("owner-{label}");
    let model_call_id = CallId::from(format!("model-call-{label}"));
    let provider_call_id = CallId::from(format!("provider-call-{label}"));
    let action_id = ActionId::from(format!("action-{label}"));

    store
        .create_run(NewRun {
            run_id: run_id.clone(),
            project_id: format!("project-{label}"),
            owner_actor_id: owner.clone(),
            canonical_root: format!("/workspace/{label}"),
            workspace_identity: format!("workspace-{label}"),
            policy_snapshot_hash: PolicyEngine::snapshot_hash(),
            config_snapshot_hash: "config-test".into(),
            max_steps: 4,
            occurred_at: "2026-07-13T00:00:00Z".into(),
        })
        .unwrap();
    store
        .append_transition(
            &run_id,
            &owner,
            Transition::StartStep {
                turn_id: TurnId::from("turn-1"),
                step_id: step_id.clone(),
                occurred_at: "2026-07-13T00:00:01Z".into(),
            },
        )
        .unwrap();
    store
        .append_model_started_with_transcript(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(step_id.clone()),
                call_id: Some(model_call_id.clone()),
                occurred_at: "2026-07-13T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
            vec![TranscriptRecord::user("policy test request")],
        )
        .unwrap();
    store
        .append_event(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(step_id.clone()),
                call_id: Some(model_call_id.clone()),
                occurred_at: "2026-07-13T00:00:03Z".into(),
                kind: HarnessEventKind::ModelCompleted {
                    assistant_text: String::new(),
                },
            },
        )
        .unwrap();
    let evaluation = PolicyEngine::new().evaluate(&action).unwrap();
    store
        .record_action(
            &owner,
            ActionRecord {
                action_id: action_id.clone(),
                run_id: run_id.clone(),
                step_id,
                call_id: provider_call_id,
                origin_model_call_id: model_call_id,
                action_hash: action_hash(&action).unwrap(),
                effect_class: evaluation.effect,
                action,
                occurred_at: "2026-07-13T00:00:04Z".into(),
            },
        )
        .unwrap();
    (path, store, run_id, action_id)
}

#[test]
fn store_computes_ask_for_network_action_and_persists_the_decision() {
    let action = AgentAction::RunCommand {
        program: "curl".into(),
        args: vec!["https://example.test".into()],
        cwd: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "network-ask");

    let (event, result) = store
        .decide_policy(
            "owner-network-ask",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        )
        .unwrap();
    assert_eq!(result.decision, PolicyDecision::Ask);
    assert_eq!(result.rule_id, "network.external");
    assert!(matches!(
        event.kind,
        HarnessEventKind::PolicyDecided {
            decision: PolicyDecision::Ask,
            ..
        }
    ));

    let events = store.events_owned(&run_id, "owner-network-ask").unwrap();
    assert_eq!(events.last().unwrap().kind_name(), "policy.decided");
    let connection = rusqlite::Connection::open(&path).unwrap();
    let state: String = connection
        .query_row(
            "SELECT state FROM actions WHERE action_id = ?1",
            [&action_id.0],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "awaiting_approval");
    drop(connection);
    drop(store);
    let reopened = SqliteRunStore::open(&path).unwrap();
    let resume = reopened
        .resume_point_owned(&run_id, "owner-network-ask", "project-network-ask")
        .unwrap()
        .unwrap();
    assert!(matches!(
        resume.next,
        ResumeNext::CreateApprovalRequest { ref action_id }
            if action_id.0 == "action-network-ask"
    ));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn forged_allow_is_rejected_and_does_not_append_an_event() {
    let action = AgentAction::RunCommand {
        program: "curl".into(),
        args: vec!["https://example.test".into()],
        cwd: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "forged-allow");
    let before = store.events_owned(&run_id, "owner-forged-allow").unwrap();

    let error = store
        .append_event(
            "owner-forged-allow",
            &run_id,
            EventAppend {
                turn_id: Some(TurnId::from("turn-1")),
                step_id: Some(StepId::from("step-forged-allow")),
                call_id: None,
                occurred_at: "2026-07-13T00:00:05Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id,
                    decision: PolicyDecision::Allow,
                    rule_id: "workspace.read".into(),
                },
            },
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::Invariant(_)));
    assert_eq!(
        store.events_owned(&run_id, "owner-forged-allow").unwrap(),
        before
    );
    drop(store);
    remove_temp_db(&path);
}

#[test]
fn store_computes_deny_for_root_destructive_action() {
    let action = AgentAction::RunCommand {
        program: "rm".into(),
        args: vec!["-rf".into(), "/".into()],
        cwd: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "root-deny");

    let (event, result) = store
        .decide_policy(
            "owner-root-deny",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        )
        .unwrap();
    assert_eq!(result.decision, PolicyDecision::Deny);
    assert_eq!(result.rule_id, "system.destructive");
    assert!(matches!(
        event.kind,
        HarnessEventKind::PolicyDecided {
            decision: PolicyDecision::Deny,
            ..
        }
    ));
    let connection = rusqlite::Connection::open(&path).unwrap();
    let state: String = connection
        .query_row(
            "SELECT state FROM actions WHERE action_id = ?1",
            [&action_id.0],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "denied");
    drop(connection);
    drop(store);
    let reopened = SqliteRunStore::open(&path).unwrap();
    let resume = reopened
        .resume_point_owned(&run_id, "owner-root-deny", "project-root-deny")
        .unwrap()
        .unwrap();
    assert!(matches!(resume.next, ResumeNext::StartNextStep));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn store_computes_allow_and_reopen_prepares_the_recorded_action() {
    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "read-allow");

    let (_, result) = store
        .decide_policy(
            "owner-read-allow",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        )
        .unwrap();
    assert_eq!(result.decision, PolicyDecision::Allow);
    assert_eq!(result.rule_id, "workspace.read");
    drop(store);

    let reopened = SqliteRunStore::open(&path).unwrap();
    let resume = reopened
        .resume_point_owned(&run_id, "owner-read-allow", "project-read-allow")
        .unwrap()
        .unwrap();
    assert!(matches!(
        resume.next,
        ResumeNext::PrepareExecution {
            ref action_id,
            ref call_id,
        } if action_id.0 == "action-read-allow" && call_id.0 == "provider-call-read-allow"
    ));
    drop(reopened);
    remove_temp_db(&path);
}

#[test]
fn snapshot_drift_and_tampered_action_state_fail_without_new_events() {
    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "snapshot-drift");
    let before = store.events_owned(&run_id, "owner-snapshot-drift").unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE runs SET policy_snapshot_hash = 'drifted-policy' WHERE run_id = ?1",
            [&run_id.0],
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        store.decide_policy(
            "owner-snapshot-drift",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-snapshot-drift").unwrap(),
        before
    );
    drop(store);
    remove_temp_db(&path);

    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "tampered-rule");
    let before = store.events_owned(&run_id, "owner-tampered-rule").unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET policy_rule_id = 'forged-rule' WHERE action_id = ?1",
            [&action_id.0],
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        store.decide_policy(
            "owner-tampered-rule",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-tampered-rule").unwrap(),
        before
    );
    drop(store);
    remove_temp_db(&path);

    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let (path, store, run_id, action_id) = seed_action(action.clone(), "tampered-json");
    let canonical = serde_json::to_string(&action).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET canonical_json = ?1 WHERE action_id = ?2",
            rusqlite::params![format!(" {canonical}"), action_id.0.as_str()],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        store.decide_policy(
            "owner-tampered-json",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Corrupt)
    ));
    assert!(matches!(
        store.events_owned(&run_id, "owner-tampered-json"),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    remove_temp_db(&path);

    let (path, store, run_id, action_id) = seed_action(action.clone(), "tampered-effect");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET effect_class = 'external_effect' WHERE action_id = ?1",
            [&action_id.0],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        store.decide_policy(
            "owner-tampered-effect",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Corrupt)
    ));
    drop(store);
    remove_temp_db(&path);

    let (path, store, run_id, action_id) = seed_action(action, "tampered-origin");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE actions SET origin_model_call_id = NULL WHERE action_id = ?1",
            [&action_id.0],
        )
        .unwrap();
    drop(connection);
    assert!(matches!(
        store.decide_policy(
            "owner-tampered-origin",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Invariant(_))
    ));
    drop(store);
    remove_temp_db(&path);
}

#[test]
fn failed_policy_event_insert_rolls_back_action_step_and_run() {
    let action = AgentAction::ReadFile {
        path: "src/lib.rs".into(),
        start_line: None,
        end_line: None,
    };
    let (path, store, run_id, action_id) = seed_action(action, "event-rollback");
    let before = store.events_owned(&run_id, "owner-event-rollback").unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_policy_event
             BEFORE INSERT ON events
             WHEN NEW.kind = 'policy.decided'
             BEGIN
               SELECT RAISE(ABORT, 'policy event rejected by test');
             END;",
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        store.decide_policy(
            "owner-event-rollback",
            &run_id,
            &action_id,
            "2026-07-13T00:00:05Z",
        ),
        Err(StoreError::Database(_))
    ));
    assert_eq!(
        store.events_owned(&run_id, "owner-event-rollback").unwrap(),
        before
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    let state: (String, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT state, policy_decision, policy_event_id
             FROM actions WHERE action_id = ?1",
            [&action_id.0],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, ("recorded".into(), None, None));
    let step_status: String = connection
        .query_row(
            "SELECT status FROM steps WHERE step_id = 'step-event-rollback'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(step_status, "action_recorded");
    drop(connection);
    drop(store);
    remove_temp_db(&path);
}
