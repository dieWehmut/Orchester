use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use orchester_laufzeit::harness::audit::JsonlAuditSink;
use orchester_laufzeit::harness::barrier::{ExecutionAuthorization, PreExecutionBarrier};
use orchester_laufzeit::harness::run_store::{
    EventAppend, RunStore, SqliteRunStore, StoreError, TranscriptBindingPhase,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_protokoll::{CallId, FeedbackReport, HarnessEventKind, ObservationId};
use secrecy::SecretString;

#[path = "support/allowed_run.rs"]
mod allowed_run;

use allowed_run::{create_allowed_run, AllowedRun};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const SECRET: &str = "configured-observation-credential-value";

#[test]
fn completed_observation_is_sanitized_linked_and_recoverable() {
    let fixture = Fixture::new("completed");
    fixture.start_tool();
    let mut input = fixture
        .run
        .tool_completed_input(&fixture.run.provider_call_id);
    let HarnessEventKind::ToolCompleted { observation } = &mut input.kind else {
        unreachable!("fixture must build a completion")
    };
    observation.kind = "read_file\u{1b}[31m".into();
    observation.summary = format!("Authorization: Bearer {SECRET}\u{1b}[0m");
    observation.data = serde_json::json!({
        "token": SECRET,
        "nested": ["safe", format!("prefix {SECRET} suffix")]
    });

    let event = fixture
        .store
        .append_event(&fixture.run.owner, &fixture.run.run_id, input)
        .unwrap();
    let binding = fixture
        .store
        .transcript_binding_owned(
            &fixture.run.run_id,
            &fixture.run.owner,
            event.sequence,
            TranscriptBindingPhase::ToolResult,
        )
        .unwrap()
        .unwrap();
    assert_eq!(binding.first_ordinal, Some(2));
    assert_eq!(binding.last_ordinal, Some(2));
    assert_eq!(binding.record_count, 1);
    let row = fixture.observation_row();
    let HarnessEventKind::ToolCompleted { observation } = event.kind else {
        panic!("terminal event must remain a completion")
    };
    let event_payload = serde_json::to_string(&observation).unwrap();

    assert_eq!(row.observation_id, observation.observation_id.0);
    assert_eq!(row.call_id, fixture.run.provider_call_id.0);
    assert_eq!(row.kind, "tool.completed");
    assert_eq!(row.outcome, "completed");
    assert_eq!(row.payload, event_payload);
    assert_eq!(row.fingerprint.len(), 64);
    assert_eq!(
        row.attempt_observation_id.as_deref(),
        Some(row.observation_id.as_str())
    );
    assert!(!row.payload.contains(SECRET));
    assert!(row.payload.contains("[REDACTED]"));
    assert!(!row.payload.contains('\u{1b}'));
    assert!(row.payload.len() <= 65_536);
    let transcript = fixture
        .store
        .transcript_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap();
    assert_eq!(transcript.len(), 2);
    let TranscriptRecord::ToolResultJson { call_id, payload } = &transcript[1].record else {
        panic!("expected a durable tool result");
    };
    assert_eq!(call_id.0, fixture.run.provider_call_id.0);
    assert_eq!(
        payload,
        &serde_json::from_str::<serde_json::Value>(&row.payload).unwrap()
    );

    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    assert!(connection
        .execute(
            "UPDATE observations SET sanitized_payload = '{}' WHERE observation_id = ?1",
            [&row.observation_id],
        )
        .is_err());
    assert!(connection
        .execute(
            "DELETE FROM observations WHERE observation_id = ?1",
            [&row.observation_id],
        )
        .is_err());
    drop(connection);

    drop(event_payload);
    let reopened = SqliteRunStore::open(&fixture.db).unwrap();
    let events = reopened
        .events_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap();
    let recovered = events
        .into_iter()
        .find_map(|event| match event.kind {
            HarnessEventKind::ToolCompleted { observation } => Some(observation),
            _ => None,
        })
        .unwrap();
    assert_eq!(serde_json::to_string(&recovered).unwrap(), row.payload);
}

#[test]
fn observation_id_with_configured_secret_is_rejected_before_persistence() {
    let fixture = Fixture::new("observation-id");
    fixture.start_tool();
    let mut input = fixture
        .run
        .tool_completed_input(&fixture.run.provider_call_id);
    let HarnessEventKind::ToolCompleted { observation } = &mut input.kind else {
        unreachable!("fixture must build a completion")
    };
    observation.observation_id = ObservationId::from(format!("observation-{SECRET}"));

    assert!(matches!(
        fixture
            .store
            .append_event(&fixture.run.owner, &fixture.run.run_id, input),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(fixture.observation_count(), 0);
    assert_eq!(
        fixture.tool_states(),
        ("started".into(), "executing".into(), "tool_running".into())
    );
}

#[test]
fn failed_observation_rebuilds_sanitized_feedback_and_fingerprint() {
    let fixture = Fixture::new("failed");
    fixture.start_tool();
    let input = tool_failed_input(
        &fixture.run,
        &fixture.run.provider_call_id,
        FeedbackReport {
            source: "run_command\u{1b}[31m".into(),
            validator_id: None,
            exit_code: Some(1),
            classification: "caller_controlled".into(),
            summary: format!("failed with {SECRET}"),
            stdout_tail: format!("Authorization: Bearer {SECRET}"),
            stderr_tail: format!("stderr\0{SECRET}"),
            fingerprint: "caller-controlled-fingerprint".into(),
            retryable: true,
        },
    );

    let event = fixture
        .store
        .append_event(&fixture.run.owner, &fixture.run.run_id, input)
        .unwrap();
    let row = fixture.observation_row();
    let HarnessEventKind::ToolFailed { feedback } = event.kind else {
        panic!("terminal event must remain a failure")
    };
    let event_payload = serde_json::to_string(&feedback).unwrap();

    assert_eq!(row.kind, "tool.failed");
    assert_eq!(row.outcome, "failed");
    assert_eq!(row.payload, event_payload);
    assert_eq!(row.fingerprint, feedback.fingerprint);
    assert_ne!(row.fingerprint, "caller-controlled-fingerprint");
    assert_eq!(feedback.classification, "tool_failed");
    assert!(!row.payload.contains(SECRET));
    assert!(!row.payload.contains('\u{1b}'));
    assert!(row.payload.contains("[REDACTED]"));
    let transcript = fixture
        .store
        .transcript_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap();
    let TranscriptRecord::ToolResultJson { call_id, payload } = &transcript[1].record else {
        panic!("expected a durable failed tool result");
    };
    assert_eq!(call_id.0, fixture.run.provider_call_id.0);
    assert_eq!(
        payload,
        &serde_json::from_str::<serde_json::Value>(&row.payload).unwrap()
    );
}

#[test]
fn validator_feedback_is_sanitized_at_the_event_boundary() {
    let fixture = Fixture::new("validator");
    fixture.start_tool();
    fixture
        .store
        .append_event(
            &fixture.run.owner,
            &fixture.run.run_id,
            fixture
                .run
                .tool_completed_input(&fixture.run.provider_call_id),
        )
        .unwrap();

    let event = fixture
        .store
        .append_event(
            &fixture.run.owner,
            &fixture.run.run_id,
            EventAppend {
                turn_id: Some(fixture.run.turn_id.clone()),
                step_id: Some(fixture.run.step_id.clone()),
                call_id: None,
                occurred_at: "2026-07-12T00:00:12Z".into(),
                kind: HarnessEventKind::ValidatorCompleted {
                    feedback: FeedbackReport {
                        source: "validator\u{1b}[31m".into(),
                        validator_id: Some("cargo-check".into()),
                        exit_code: Some(1),
                        classification: "validator_failed".into(),
                        summary: format!("failed with {SECRET}"),
                        stdout_tail: format!("Authorization: Bearer {SECRET}"),
                        stderr_tail: format!("stderr\0{SECRET}"),
                        fingerprint: "caller-controlled-fingerprint".into(),
                        retryable: true,
                    },
                },
            },
        )
        .unwrap();
    let HarnessEventKind::ValidatorCompleted { feedback } = &event.kind else {
        panic!("validator event must remain a completion")
    };
    let event_payload = serde_json::to_value(feedback).unwrap();
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    let row_payload: String = connection
        .query_row(
            "SELECT sanitized_payload FROM events WHERE kind = 'validator.completed'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(connection);

    let row_value: serde_json::Value = serde_json::from_str(&row_payload).unwrap();
    assert_eq!(row_value.get("feedback"), Some(&event_payload));
    assert!(!row_payload.contains(SECRET));
    assert!(!row_payload.contains('\u{1b}'));
    assert!(row_payload.contains("[REDACTED]"));
    assert_ne!(feedback.fingerprint, "caller-controlled-fingerprint");

    let reopened = SqliteRunStore::open(&fixture.db).unwrap();
    let recovered = reopened
        .events_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap()
        .into_iter()
        .find_map(|event| match event.kind {
            HarnessEventKind::ValidatorCompleted { feedback } => Some(feedback),
            _ => None,
        })
        .unwrap();
    assert_eq!(serde_json::to_value(&recovered).unwrap(), event_payload);
    drop(reopened);
}

#[test]
fn terminal_write_requires_explicit_secret_aware_store_configuration() {
    let fixture = Fixture::new("unconfigured");
    fixture.start_tool();
    let unconfigured = SqliteRunStore::open(&fixture.db).unwrap();
    let input = fixture
        .run
        .tool_completed_input(&fixture.run.provider_call_id);

    assert!(matches!(
        unconfigured.append_event(&fixture.run.owner, &fixture.run.run_id, input),
        Err(StoreError::Invariant(_))
    ));
    assert_eq!(fixture.observation_count(), 0);
    assert_eq!(
        fixture.tool_states(),
        ("started".into(), "executing".into(), "tool_running".into())
    );

    fixture
        .store
        .append_event(
            &fixture.run.owner,
            &fixture.run.run_id,
            fixture
                .run
                .tool_completed_input(&fixture.run.provider_call_id),
        )
        .unwrap();
    assert_eq!(fixture.observation_count(), 1);
}

#[test]
fn oversized_observation_completes_with_a_bounded_truncation_sentinel() {
    let fixture = Fixture::new("oversized");
    fixture.start_tool();
    let before = fixture
        .store
        .load_run_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap()
        .next_sequence;
    let mut input = fixture
        .run
        .tool_completed_input(&fixture.run.provider_call_id);
    let HarnessEventKind::ToolCompleted { observation } = &mut input.kind else {
        unreachable!("fixture must build a completion")
    };
    observation.data = serde_json::json!({"blob": "x".repeat(70_000)});

    let event = fixture
        .store
        .append_event(&fixture.run.owner, &fixture.run.run_id, input)
        .unwrap();
    let HarnessEventKind::ToolCompleted { observation } = event.kind else {
        panic!("oversized terminal event must remain a completion")
    };
    assert_eq!(
        observation.data,
        serde_json::json!({"reason": "limit_exceeded", "truncated": true})
    );
    let row = fixture.observation_row();
    assert_eq!(fixture.observation_count(), 1);
    assert!(row.payload.len() <= 65_536);
    assert!(row.payload.contains("\"truncated\":true"));
    assert_eq!(
        fixture.tool_states(),
        ("completed".into(), "completed".into(), "observed".into())
    );
    assert_eq!(
        fixture
            .store
            .load_run_owned(&fixture.run.run_id, &fixture.run.owner)
            .unwrap()
            .next_sequence,
        before + 1
    );
}

#[test]
fn concurrent_different_observations_leave_no_loser_orphan() {
    let fixture = Fixture::new("concurrent");
    fixture.start_tool();
    let first = SqliteRunStore::open_with_terminal_secrets(
        &fixture.db,
        vec![SecretString::new(SECRET.to_owned().into_boxed_str())],
    )
    .unwrap();
    let second = SqliteRunStore::open_with_terminal_secrets(
        &fixture.db,
        vec![SecretString::new(SECRET.to_owned().into_boxed_str())],
    )
    .unwrap();
    let owner = fixture.run.owner.clone();
    let run_id = fixture.run.run_id.clone();
    let first_input = fixture
        .run
        .tool_completed_input(&fixture.run.provider_call_id);
    let mut second_input = first_input.clone();
    let HarnessEventKind::ToolCompleted { observation } = &mut second_input.kind else {
        unreachable!("fixture must build a completion")
    };
    observation.observation_id = ObservationId::from("observation-concurrent-second");
    let start = Arc::new(Barrier::new(2));
    let first_start = start.clone();
    let first_owner = owner.clone();
    let first_run_id = run_id.clone();
    let first_result = thread::spawn(move || {
        first_start.wait();
        first.append_event(&first_owner, &first_run_id, first_input)
    });
    let second_result = thread::spawn(move || {
        start.wait();
        second.append_event(&owner, &run_id, second_input)
    });

    let results = [first_result.join().unwrap(), second_result.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(StoreError::Invariant(_))))
            .count(),
        1
    );
    let winner_id = results
        .iter()
        .find_map(|result| result.as_ref().ok())
        .and_then(|event| match &event.kind {
            HarnessEventKind::ToolCompleted { observation } => {
                Some(observation.observation_id.0.clone())
            }
            _ => None,
        })
        .unwrap();
    let row = fixture.observation_row();
    assert_eq!(fixture.observation_count(), 1);
    assert_eq!(row.observation_id, winner_id);
    assert_eq!(
        row.attempt_observation_id.as_deref(),
        Some(winner_id.as_str())
    );
    let completions = fixture
        .store
        .events_owned(&fixture.run.run_id, &fixture.run.owner)
        .unwrap()
        .into_iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolCompleted { .. }))
        .count();
    assert_eq!(completions, 1);
}

fn tool_failed_input(run: &AllowedRun, call_id: &CallId, feedback: FeedbackReport) -> EventAppend {
    EventAppend {
        turn_id: Some(run.turn_id.clone()),
        step_id: Some(run.step_id.clone()),
        call_id: Some(call_id.clone()),
        occurred_at: "2026-07-12T00:00:11Z".into(),
        kind: HarnessEventKind::ToolFailed { feedback },
    }
}

struct ObservationRow {
    observation_id: String,
    call_id: String,
    kind: String,
    payload: String,
    fingerprint: String,
    outcome: String,
    attempt_observation_id: Option<String>,
}

struct Fixture {
    root: PathBuf,
    db: PathBuf,
    store: Arc<SqliteRunStore>,
    run: AllowedRun,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchester-durable-observation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let db = root.join("state.db");
        let store = Arc::new(
            SqliteRunStore::open_with_terminal_secrets(
                &db,
                vec![SecretString::new(SECRET.to_owned().into_boxed_str())],
            )
            .unwrap(),
        );
        let run = create_allowed_run(store.as_ref(), label);
        Self {
            root,
            db,
            store,
            run,
        }
    }

    fn start_tool(&self) {
        let barrier = PreExecutionBarrier::new(
            self.store.clone(),
            Arc::new(JsonlAuditSink::open(self.root.join("audit.jsonl")).unwrap()),
        );
        let permit = barrier
            .prepare(
                &self.run.owner,
                &self.run.run_id,
                &self.run.action_id,
                ExecutionAuthorization::Allow,
                "ignored",
            )
            .unwrap();
        barrier
            .start_tool(
                &self.run.owner,
                &self.run.run_id,
                permit,
                self.run.tool_started_input(),
            )
            .unwrap();
    }

    fn observation_row(&self) -> ObservationRow {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row(
                "SELECT observation.observation_id, observation.call_id,
                        observation.kind, observation.sanitized_payload,
                        observation.fingerprint, observation.outcome,
                        attempt.observation_id
                 FROM observations AS observation
                 JOIN tool_attempts AS attempt ON attempt.call_id = observation.call_id",
                [],
                |row| {
                    Ok(ObservationRow {
                        observation_id: row.get(0)?,
                        call_id: row.get(1)?,
                        kind: row.get(2)?,
                        payload: row.get(3)?,
                        fingerprint: row.get(4)?,
                        outcome: row.get(5)?,
                        attempt_observation_id: row.get(6)?,
                    })
                },
            )
            .unwrap()
    }

    fn observation_count(&self) -> u32 {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM observations", [], |row| row.get(0))
            .unwrap()
    }

    fn tool_states(&self) -> (String, String, String) {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row(
                "SELECT attempt.state, action.state, step.status
                 FROM tool_attempts AS attempt
                 JOIN actions AS action ON action.action_id = attempt.action_id
                 JOIN steps AS step ON step.run_id = action.run_id AND step.step_id = action.step_id
                 WHERE attempt.call_id = ?1",
                [&self.run.provider_call_id.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
