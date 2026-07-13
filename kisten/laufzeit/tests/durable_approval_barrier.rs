use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalError, ApprovalRequestInput, ApprovalState, DurableApprovalStore,
};
use orchester_laufzeit::harness::audit::{AuditError, AuditInput, AuditSink, JsonlAuditSink};
use orchester_laufzeit::harness::barrier::{ExecutionAuthorization, PreExecutionBarrier};
use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, RunStore, SqliteRunStore, StoreError,
    ResumeNext, Transition,
};
use orchester_laufzeit::harness::transcript::TranscriptRecord;
use orchester_protokoll::{
    ActionId, AgentAction, CallId, FeedbackReport, HarnessEventKind, Observation, ObservationId,
    PolicyDecision, RunId, StepId, TurnId,
};

#[path = "support/allowed_run.rs"]
mod allowed_run;

use allowed_run::create_allowed_run;

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn durable_approval_survives_reopen_and_is_owner_scoped() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let id = durable
        .request(fixture.approval_input(100))
        .expect("approval request persists");
    drop(durable);
    drop(fixture.store.clone());

    let reopened = Arc::new(SqliteRunStore::open(&fixture.db).unwrap());
    let durable = DurableApprovalStore::new(reopened);
    assert_eq!(
        durable.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Awaiting
    );
    assert!(matches!(
        durable.state(&id, "other-owner"),
        Err(ApprovalError::NotFound)
    ));

    let capability = durable
        .approve(&id, &fixture.owner, &fixture.binding())
        .expect("owner can approve exact binding");
    assert_eq!(
        durable.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Approved
    );
    assert!(format!("{capability:?}").contains("REDACTED"));
}

#[test]
fn approval_summary_is_sanitized_consistently_in_row_and_event() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let secret = "sk-embedded-approval-secret-123456";
    let mut input = fixture.approval_input(100);
    input.action_summary = format!("run_command note={secret}");
    let approval_id = durable.request(input).unwrap();

    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    let (summary, payload): (String, String) = connection
        .query_row(
            "SELECT approvals.action_summary, events.sanitized_payload
             FROM approvals JOIN events ON events.event_id = approvals.approval_event_id
             WHERE approvals.approval_id = ?1",
            [&approval_id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    drop(connection);

    assert!(!summary.contains(secret));
    assert!(summary.contains("[REDACTED_TOKEN]"));
    assert!(!payload.contains(secret));
    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(
        payload
            .pointer("/request/action_summary")
            .and_then(serde_json::Value::as_str),
        Some(summary.as_str())
    );
}

#[test]
fn approval_metadata_with_provider_token_is_rejected_atomically() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let secret = "sk-approval-risk-secret-123456";
    let mut input = fixture.approval_input(100);
    input.risk = format!("risk-{secret}");

    assert!(matches!(
        durable.request(input),
        Err(ApprovalError::Storage)
    ));
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    let approval_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM approvals", [], |row| row.get(0))
        .unwrap();
    assert_eq!(approval_count, 0);
}

#[test]
fn lost_capability_can_be_reissued_only_to_the_approval_owner() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let id = durable.request(fixture.approval_input(100)).unwrap();
    let binding = fixture.binding();
    let lost = durable.approve(&id, &fixture.owner, &binding).unwrap();
    let obsolete = lost.clone();
    drop(lost);
    assert!(matches!(
        durable.reissue(&id, "other-owner", &binding),
        Err(ApprovalError::NotFound)
    ));
    let replacement = durable
        .reissue(&id, &fixture.owner, &binding)
        .expect("an approved owner can recover a lost capability");
    assert!(matches!(
        durable.consume(&obsolete, &fixture.owner, &binding),
        Err(ApprovalError::InvalidCapability)
    ));
    let barrier = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(JsonlAuditSink::open(fixture.db.with_file_name("reissue-audit.jsonl")).unwrap()),
    );
    barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &replacement,
                binding: &binding,
            },
            "ignored",
        )
        .unwrap();
    assert_eq!(
        durable.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Executing
    );
}

#[test]
fn durable_approval_expires_or_invalidates_on_binding_drift() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let id = durable.request(fixture.approval_input(20)).unwrap();
    let drifted = ApprovalBinding {
        action_hash: "changed-action".into(),
        ..fixture.binding()
    };
    assert!(matches!(
        durable.approve(&id, &fixture.owner, &drifted),
        Err(ApprovalError::BindingMismatch)
    ));
    assert_eq!(
        durable.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Invalidated
    );

    let expired = Fixture::new(PolicyDecision::Ask);
    let clocked = DurableApprovalStore::new(expired.store.clone());
    let id = clocked.request(expired.approval_input(100)).unwrap();
    rusqlite::Connection::open(&expired.db)
        .unwrap()
        .execute(
            "UPDATE approvals SET expires_at_unix = 0 WHERE approval_id = ?1",
            [&id.0],
        )
        .unwrap();
    assert!(matches!(
        clocked.approve(&id, &expired.owner, &expired.binding()),
        Err(ApprovalError::Expired)
    ));
    assert_eq!(
        clocked.state(&id, &expired.owner).unwrap(),
        ApprovalState::Expired
    );
}

#[test]
fn durable_approval_rechecks_the_current_database_binding() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let id = durable.request(fixture.approval_input(100)).unwrap();
    let connection = rusqlite::Connection::open(&fixture.db).unwrap();
    connection
        .execute(
            "UPDATE runs SET policy_snapshot_hash = 'changed-policy' WHERE run_id = ?1",
            [&fixture.run_id.0],
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        durable.approve(&id, &fixture.owner, &fixture.binding()),
        Err(ApprovalError::BindingMismatch)
    ));
    assert_eq!(
        durable.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Invalidated
    );
}

#[test]
fn concurrent_durable_consumption_has_one_winner() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let first = DurableApprovalStore::new(fixture.store.clone());
    let id = first.request(fixture.approval_input(100)).unwrap();
    let capability = first
        .approve(&id, &fixture.owner, &fixture.binding())
        .unwrap();

    let audit_path = fixture.db.with_file_name("concurrent-audit.jsonl");
    let sink = JsonlAuditSink::open(&audit_path).unwrap();
    let candidate = fixture
        .store
        .execution_candidate(&fixture.owner, &fixture.run_id, &fixture.action_id)
        .unwrap();
    let receipt = sink
        .append_and_sync(candidate.audit_input("2026-07-12T00:00:10Z"))
        .unwrap();
    fixture
        .store
        .mark_execution_checkpoint(&fixture.owner, &candidate, &receipt)
        .unwrap();

    let second_store = Arc::new(SqliteRunStore::open(&fixture.db).unwrap());
    let second = DurableApprovalStore::new(second_store);
    let owner = fixture.owner.clone();
    let binding = fixture.binding();
    let capability_for_thread = capability.clone();
    let winner = thread::spawn(move || second.consume(&capability_for_thread, &owner, &binding));
    let loser = first.consume(&capability, &fixture.owner, &fixture.binding());
    let outcomes = [winner.join().unwrap(), loser];
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert_eq!(
        first.state(&id, &fixture.owner).unwrap(),
        ApprovalState::Executing
    );
}

#[test]
fn approval_barrier_transitions_to_consumed_only_after_tool_start() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let approval_id = durable.request(fixture.approval_input(100)).unwrap();
    let capability = durable
        .approve(&approval_id, &fixture.owner, &fixture.binding())
        .unwrap();
    let audit_path = fixture.db.with_file_name("approval-audit.jsonl");
    let sink = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(fixture.store.clone(), sink);
    let binding = fixture.binding();
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored-by-candidate",
        )
        .unwrap();
    assert_eq!(permit.action_id(), &fixture.action_id);
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Executing
    );
    barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            permit,
            fixture.tool_started_input(),
        )
        .unwrap();
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Consumed
    );
    let events = fixture
        .store
        .events_owned(&fixture.run_id, &fixture.owner)
        .unwrap();
    assert!(matches!(
        &events.last().unwrap().kind,
        HarnessEventKind::ApprovalResolved {
            approval_id: resolved,
            decision,
        } if resolved == &approval_id && decision == "consumed"
    ));
}

#[test]
fn consumed_approval_keeps_the_original_audit_binding_for_resume() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let approval_id = durable.request(fixture.approval_input(100)).unwrap();
    let binding = fixture.binding();
    let capability = durable
        .approve(&approval_id, &fixture.owner, &binding)
        .unwrap();
    let audit_path = fixture.db.with_file_name("resume-approval-audit.jsonl");
    let sink = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(fixture.store.clone(), sink);
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "2026-07-12T00:00:10Z",
        )
        .unwrap();
    barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            permit,
            fixture.tool_started_input(),
        )
        .unwrap();

    let point = fixture
        .store
        .resume_point_owned(&fixture.run_id, &fixture.owner, "project-durable")
        .unwrap()
        .unwrap();
    assert!(matches!(point.next, ResumeNext::ReconcileToolOutcome { .. }));
    drop(durable);
    std::fs::remove_file(audit_path).ok();
}

#[test]
fn barrier_fails_closed_without_audit_and_recovers_idempotently_after_append() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    let audit_path = fixture.db.with_file_name("audit.jsonl");
    let sink = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let failing = PreExecutionBarrier::new(fixture.store.clone(), Arc::new(FailingAudit));
    assert!(matches!(
        failing.prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "2026-07-12T00:00:09Z",
        ),
        Err(orchester_laufzeit::harness::barrier::BarrierError::AuditUnavailable(_))
    ));
    assert!(!fixture
        .store
        .has_audit_checkpoint(&fixture.action_id)
        .unwrap());
    let barrier = PreExecutionBarrier::new(fixture.store.clone(), sink.clone());

    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "2026-07-12T00:00:10Z",
        )
        .expect("audit-backed allow gets a permit");
    assert_eq!(permit.action_id(), &fixture.action_id);
    assert!(fixture
        .store
        .has_audit_checkpoint(&fixture.action_id)
        .unwrap());

    let candidate = fixture
        .store
        .execution_candidate(&fixture.owner, &fixture.run_id, &fixture.action_id)
        .unwrap();
    let receipt = sink
        .append_and_sync(candidate.audit_input("2026-07-12T00:00:10Z"))
        .unwrap();
    assert_eq!(receipt.event_id(), candidate.event_id().0.as_str());
    let reopened = Arc::new(SqliteRunStore::open(&fixture.db).unwrap());
    let resumed = PreExecutionBarrier::new(reopened, sink);
    let resumed_permit = resumed
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "2026-07-12T00:00:10Z",
        )
        .expect("existing audit event is idempotently reconciled");
    assert_eq!(resumed_permit.event_id(), permit.event_id());
}

#[test]
fn crash_after_audit_sync_reconciles_the_checkpoint_on_reopen() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    let audit_path = fixture.db.with_file_name("crash-audit.jsonl");
    let sink = JsonlAuditSink::open(&audit_path).unwrap();
    let candidate = fixture
        .store
        .execution_candidate(&fixture.owner, &fixture.run_id, &fixture.action_id)
        .unwrap();
    sink.append_and_sync(candidate.audit_input("ignored"))
        .unwrap();
    assert!(!fixture
        .store
        .has_audit_checkpoint(&fixture.action_id)
        .unwrap());
    drop(sink);

    let reopened = Arc::new(SqliteRunStore::open(&fixture.db).unwrap());
    let sink = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(reopened.clone(), sink.clone());
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .expect("the existing synced audit entry is reconciled");
    assert_eq!(permit.action_id(), &fixture.action_id);
    assert!(reopened.has_audit_checkpoint(&fixture.action_id).unwrap());
    assert_eq!(sink.verify().unwrap().entries, 1);
}

#[test]
fn crash_after_approval_consume_can_recover_but_tool_start_is_still_once() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let approval_id = durable.request(fixture.approval_input(100)).unwrap();
    let binding = fixture.binding();
    let capability = durable
        .approve(&approval_id, &fixture.owner, &binding)
        .unwrap();
    let audit_path = fixture.db.with_file_name("consume-crash-audit.jsonl");
    let first = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(JsonlAuditSink::open(&audit_path).unwrap()),
    );
    let abandoned = first
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        )
        .unwrap();
    drop(abandoned);
    drop(first);
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Executing
    );

    let reopened = Arc::new(SqliteRunStore::open(&fixture.db).unwrap());
    let resumed = PreExecutionBarrier::new(
        reopened,
        Arc::new(JsonlAuditSink::open(&audit_path).unwrap()),
    );
    let recover = || {
        resumed.prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        )
    };
    let first_permit = recover().expect("executing approval recovers after restart");
    let second_permit = recover().expect("recovery may race but execution remains CAS guarded");
    resumed
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            first_permit,
            fixture.tool_started_input(),
        )
        .unwrap();
    assert!(resumed
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            second_permit,
            fixture.tool_started_input(),
        )
        .is_err());
}

#[test]
fn permit_cannot_start_after_durable_approval_expiry() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let approval_id = durable.request(fixture.approval_input(100)).unwrap();
    let binding = fixture.binding();
    let capability = durable
        .approve(&approval_id, &fixture.owner, &binding)
        .unwrap();
    let barrier = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(
            JsonlAuditSink::open(fixture.db.with_file_name("permit-expiry-audit.jsonl")).unwrap(),
        ),
    );
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        )
        .unwrap();
    rusqlite::Connection::open(&fixture.db)
        .unwrap()
        .execute(
            "UPDATE approvals SET expires_at_unix = 0 WHERE approval_id = ?1",
            [&approval_id.0],
        )
        .unwrap();
    assert!(barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            permit,
            fixture.tool_started_input(),
        )
        .is_err());
    assert!(matches!(
        barrier.prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        ),
        Err(orchester_laufzeit::harness::barrier::BarrierError::Approval(ApprovalError::Expired))
    ));
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Expired
    );
}

#[test]
fn tool_start_requires_checkpoint_and_schema_permissions_are_checked() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    assert_eq!(fixture.store.schema_version().unwrap(), 8);
    let before = fixture.store.append_event(
        &fixture.owner,
        &fixture.run_id,
        fixture.tool_started_input(),
    );
    assert!(before.is_err(), "tool start must fail before the barrier");

    let audit_path = fixture.db.with_file_name("tool-audit.jsonl");
    let sink = Arc::new(JsonlAuditSink::open(&audit_path).unwrap());
    let barrier = PreExecutionBarrier::new(fixture.store.clone(), sink);
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "2026-07-12T00:00:10Z",
        )
        .unwrap();
    let after = barrier.start_tool(
        &fixture.owner,
        &fixture.run_id,
        permit,
        fixture.tool_started_input(),
    );
    assert!(
        after.is_ok(),
        "tool start is allowed only after the barrier"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&fixture.db).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let parent_mode = std::fs::metadata(fixture.db.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(parent_mode, 0o700);
        let mut sidecars = 0;
        for suffix in ["-wal", "-shm"] {
            let mut path = fixture.db.as_os_str().to_os_string();
            path.push(suffix);
            let path = PathBuf::from(path);
            if path.exists() {
                sidecars += 1;
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
        assert!(sidecars > 0, "WAL mode should create a protected sidecar");
    }
}

#[test]
fn tool_start_returns_the_durable_action_and_rejects_post_permit_replacement() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    let audit_path = fixture.db.with_file_name("action-binding-audit.jsonl");
    let barrier = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(JsonlAuditSink::open(&audit_path).unwrap()),
    );
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .unwrap();
    let replacement = AgentAction::ListFiles {
        path: "different-path".into(),
        depth: 1,
    };
    let replacement_json = serde_json::to_string(&replacement).unwrap();
    let replacement_hash = action_hash(&replacement).unwrap();
    rusqlite::Connection::open(&fixture.db)
        .unwrap()
        .execute(
            "UPDATE actions SET canonical_json = ?1, action_hash = ?2 WHERE action_id = ?3",
            rusqlite::params![replacement_json, replacement_hash, fixture.action_id.0],
        )
        .unwrap();

    assert!(barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            permit,
            fixture.tool_started_input(),
        )
        .is_err());

    let unchanged = Fixture::new(PolicyDecision::Allow);
    let audit_path = unchanged.db.with_file_name("durable-action-audit.jsonl");
    let barrier = PreExecutionBarrier::new(
        unchanged.store.clone(),
        Arc::new(JsonlAuditSink::open(&audit_path).unwrap()),
    );
    let permit = barrier
        .prepare(
            &unchanged.owner,
            &unchanged.run_id,
            &unchanged.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .unwrap();
    let started = barrier
        .start_tool(
            &unchanged.owner,
            &unchanged.run_id,
            permit,
            unchanged.tool_started_input(),
        )
        .unwrap();
    assert_eq!(started.action(), &unchanged.action);
    assert!(matches!(
        started.event().kind,
        HarnessEventKind::ToolStarted { .. }
    ));
}

#[test]
fn tool_start_rejects_provider_call_mismatch_before_approval_consumption() {
    let fixture = Fixture::new(PolicyDecision::Ask);
    let durable = DurableApprovalStore::new(fixture.store.clone());
    let approval_id = durable.request(fixture.approval_input(100)).unwrap();
    let binding = fixture.binding();
    let capability = durable
        .approve(&approval_id, &fixture.owner, &binding)
        .unwrap();
    let barrier = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(
            JsonlAuditSink::open(fixture.db.with_file_name("call-mismatch-audit.jsonl")).unwrap(),
        ),
    );
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        )
        .unwrap();
    let mut wrong_input = fixture.tool_started_input();
    wrong_input.call_id = Some(CallId::from("mismatched-provider-call"));
    assert!(barrier
        .start_tool(&fixture.owner, &fixture.run_id, permit, wrong_input)
        .is_err());
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Executing
    );

    let retry = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Approval {
                capability: &capability,
                binding: &binding,
            },
            "ignored",
        )
        .unwrap();
    barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            retry,
            fixture.tool_started_input(),
        )
        .unwrap();
    assert_eq!(
        durable.state(&approval_id, &fixture.owner).unwrap(),
        ApprovalState::Consumed
    );
}

#[test]
fn tool_completion_cannot_finish_another_runs_attempt() {
    let first = Fixture::new(PolicyDecision::Allow);
    let second = create_allowed_run(first.store.as_ref(), "second");
    let first_barrier = PreExecutionBarrier::new(
        first.store.clone(),
        Arc::new(JsonlAuditSink::open(first.db.with_file_name("cross-run-first.jsonl")).unwrap()),
    );
    let second_barrier = PreExecutionBarrier::new(
        first.store.clone(),
        Arc::new(JsonlAuditSink::open(first.db.with_file_name("cross-run-second.jsonl")).unwrap()),
    );
    let first_permit = first_barrier
        .prepare(
            &first.owner,
            &first.run_id,
            &first.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .unwrap();
    first_barrier
        .start_tool(
            &first.owner,
            &first.run_id,
            first_permit,
            first.tool_started_input(),
        )
        .unwrap();
    let second_permit = second_barrier
        .prepare(
            &second.owner,
            &second.run_id,
            &second.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .unwrap();
    second_barrier
        .start_tool(
            &second.owner,
            &second.run_id,
            second_permit,
            second.tool_started_input(),
        )
        .unwrap();

    let wrong = second.tool_completed_input(&first.provider_call_id);
    let wrong_result = first
        .store
        .append_event(&second.owner, &second.run_id, wrong);
    let first_result = first.store.append_event(
        &first.owner,
        &first.run_id,
        first.tool_completed_input(&first.provider_call_id),
    );
    let second_result = first.store.append_event(
        &second.owner,
        &second.run_id,
        second.tool_completed_input(&second.provider_call_id),
    );
    let first_events = first
        .store
        .events_owned(&first.run_id, &first.owner)
        .unwrap();
    let second_events = first
        .store
        .events_owned(&second.run_id, &second.owner)
        .unwrap();
    let first_completions = first_events
        .iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolCompleted { .. }))
        .count();
    let second_completions = second_events
        .iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolCompleted { .. }))
        .count();

    assert!(wrong_result.is_err());
    assert!(first_result.is_ok());
    assert!(second_result.is_ok());
    assert_eq!(first_completions, 1);
    assert_eq!(second_completions, 1);
}

#[test]
fn reopened_store_rejects_persisted_attempt_with_wrong_provider_call() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    let barrier = PreExecutionBarrier::new(
        fixture.store.clone(),
        Arc::new(
            JsonlAuditSink::open(fixture.db.with_file_name("legacy-call-audit.jsonl")).unwrap(),
        ),
    );
    let permit = barrier
        .prepare(
            &fixture.owner,
            &fixture.run_id,
            &fixture.action_id,
            ExecutionAuthorization::Allow,
            "ignored",
        )
        .unwrap();
    barrier
        .start_tool(
            &fixture.owner,
            &fixture.run_id,
            permit,
            fixture.tool_started_input(),
        )
        .unwrap();

    let legacy_call_id = CallId::from("legacy-mismatched-provider-call");
    rusqlite::Connection::open(&fixture.db)
        .unwrap()
        .execute(
            "UPDATE tool_attempts SET call_id = ?1 WHERE call_id = ?2",
            [&legacy_call_id.0, &fixture.provider_call_id.0],
        )
        .unwrap();
    drop(barrier);

    assert!(matches!(
        SqliteRunStore::open_with_terminal_secrets(&fixture.db, Vec::new()),
        Err(StoreError::Corrupt)
    ));
    let completions = fixture
        .store
        .events_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .into_iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolCompleted { .. }))
        .count();

    assert_eq!(completions, 0);
}

#[test]
fn wrong_observation_is_atomic_and_matching_completion_can_retry() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    fixture.start_allowed_tool("wrong-observation-audit.jsonl");
    let before = fixture
        .store
        .load_run_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .next_sequence;
    let mut wrong = fixture.tool_completed_input(&fixture.provider_call_id);
    let HarnessEventKind::ToolCompleted { observation } = &mut wrong.kind else {
        unreachable!("fixture must build a completion event")
    };
    observation.call_id = CallId::from("wrong-observation-call");

    assert!(fixture
        .store
        .append_event(&fixture.owner, &fixture.run_id, wrong)
        .is_err());
    assert_eq!(
        fixture.persisted_tool_states(&fixture.provider_call_id),
        ("started".into(), "executing".into(), "tool_running".into())
    );
    assert_eq!(
        fixture
            .store
            .load_run_owned(&fixture.run_id, &fixture.owner)
            .unwrap()
            .next_sequence,
        before
    );

    fixture
        .store
        .append_event(
            &fixture.owner,
            &fixture.run_id,
            fixture.tool_completed_input(&fixture.provider_call_id),
        )
        .unwrap();
    let after_completion = fixture
        .store
        .load_run_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .next_sequence;
    assert_eq!(
        fixture.persisted_tool_states(&fixture.provider_call_id),
        ("completed".into(), "completed".into(), "observed".into())
    );
    assert!(fixture
        .store
        .append_event(
            &fixture.owner,
            &fixture.run_id,
            fixture.tool_completed_input(&fixture.provider_call_id),
        )
        .is_err());
    assert_eq!(
        fixture
            .store
            .load_run_owned(&fixture.run_id, &fixture.owner)
            .unwrap()
            .next_sequence,
        after_completion
    );
}

#[test]
fn tool_failure_is_single_use_and_persists_all_terminal_states() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    fixture.start_allowed_tool("tool-failed-audit.jsonl");
    let failed = fixture.tool_failed_input(&fixture.provider_call_id);

    fixture
        .store
        .append_event(&fixture.owner, &fixture.run_id, failed.clone())
        .unwrap();
    let after_first = fixture
        .store
        .load_run_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .next_sequence;
    assert_eq!(
        fixture.persisted_tool_states(&fixture.provider_call_id),
        ("failed".into(), "failed".into(), "observed".into())
    );

    assert!(fixture
        .store
        .append_event(&fixture.owner, &fixture.run_id, failed)
        .is_err());
    assert!(fixture
        .store
        .append_event(
            &fixture.owner,
            &fixture.run_id,
            fixture.tool_completed_input(&fixture.provider_call_id),
        )
        .is_err());
    assert_eq!(
        fixture
            .store
            .load_run_owned(&fixture.run_id, &fixture.owner)
            .unwrap()
            .next_sequence,
        after_first
    );
    let failures = fixture
        .store
        .events_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .into_iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolFailed { .. }))
        .count();
    assert_eq!(failures, 1);
}

#[test]
fn concurrent_tool_completion_has_one_durable_winner() {
    let fixture = Fixture::new(PolicyDecision::Allow);
    fixture.start_allowed_tool("concurrent-terminal-audit.jsonl");
    let first = SqliteRunStore::open_with_terminal_secrets(&fixture.db, Vec::new()).unwrap();
    let second = SqliteRunStore::open_with_terminal_secrets(&fixture.db, Vec::new()).unwrap();
    let start = Arc::new(Barrier::new(2));
    let owner = fixture.owner.clone();
    let run_id = fixture.run_id.clone();
    let input = fixture.tool_completed_input(&fixture.provider_call_id);
    let first_start = start.clone();
    let first_owner = owner.clone();
    let first_run_id = run_id.clone();
    let first_input = input.clone();
    let first_result = thread::spawn(move || {
        first_start.wait();
        first.append_event(&first_owner, &first_run_id, first_input)
    });
    let second_result = thread::spawn(move || {
        start.wait();
        second.append_event(&owner, &run_id, input)
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
    let completions = fixture
        .store
        .events_owned(&fixture.run_id, &fixture.owner)
        .unwrap()
        .into_iter()
        .filter(|event| matches!(&event.kind, HarnessEventKind::ToolCompleted { .. }))
        .count();
    assert_eq!(completions, 1);
    assert_eq!(
        fixture.persisted_tool_states(&fixture.provider_call_id),
        ("completed".into(), "completed".into(), "observed".into())
    );
}

#[cfg(unix)]
#[test]
fn insecure_existing_state_directory_is_rejected_without_chmod() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!(
        "orchester-insecure-state-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let result = SqliteRunStore::open(root.join("state.db"));
    assert!(matches!(
        result,
        Err(orchester_laufzeit::harness::run_store::StoreError::InsecurePermissions)
    ));
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o755
    );
    std::fs::remove_dir_all(root).ok();
}

struct Fixture {
    db: PathBuf,
    store: Arc<SqliteRunStore>,
    run_id: RunId,
    action_id: ActionId,
    step_id: StepId,
    owner: String,
    action: AgentAction,
    hash: String,
    provider_call_id: CallId,
}

impl Fixture {
    fn new(decision: PolicyDecision) -> Self {
        let fixture_id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "orchester-durable-{}-{}",
            std::process::id(),
            fixture_id
        ));
        let db = root.join("state.db");
        let store = Arc::new(SqliteRunStore::open_with_terminal_secrets(&db, Vec::new()).unwrap());
        let run_id = RunId::from("run-durable");
        let owner = "owner-durable".to_owned();
        store
            .create_run(NewRun {
                run_id: run_id.clone(),
                project_id: "project-durable".into(),
                owner_actor_id: owner.clone(),
                canonical_root: root.to_string_lossy().into_owned(),
                workspace_identity: "workspace-durable".into(),
                policy_snapshot_hash: PolicyEngine::snapshot_hash(),
                config_snapshot_hash: "config-durable".into(),
                max_steps: 4,
                occurred_at: "2026-07-12T00:00:00Z".into(),
            })
            .unwrap();
        let turn_id = TurnId::from("turn-durable");
        let step_id = StepId::from("step-durable");
        let call_id = CallId::from("call-durable");
        store
            .append_transition(
                &run_id,
                &owner,
                Transition::StartStep {
                    turn_id: turn_id.clone(),
                    step_id: step_id.clone(),
                    occurred_at: "2026-07-12T00:00:01Z".into(),
                },
            )
            .unwrap();
        store
            .append_model_started_with_transcript(
                &owner,
                &run_id,
                orchester_laufzeit::harness::run_store::EventAppend {
                    turn_id: Some(turn_id.clone()),
                    step_id: Some(step_id.clone()),
                    call_id: Some(call_id.clone()),
                    occurred_at: "2026-07-12T00:00:02Z".into(),
                    kind: HarnessEventKind::ModelStarted,
                },
                vec![TranscriptRecord::user("durable approval request context")],
            )
            .unwrap();
        store
            .append_event(
                &owner,
                &run_id,
                orchester_laufzeit::harness::run_store::EventAppend {
                    turn_id: Some(turn_id.clone()),
                    step_id: Some(step_id.clone()),
                    call_id: Some(call_id.clone()),
                    occurred_at: "2026-07-12T00:00:02Z".into(),
                    kind: HarnessEventKind::ModelCompleted {
                        assistant_text: String::new(),
                    },
                },
            )
            .unwrap();
        let action = if decision == PolicyDecision::Allow {
            AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            }
        } else {
            AgentAction::RunCommand {
                program: "cargo".into(),
                args: vec!["add".into(), "serde".into()],
                cwd: None,
            }
        };
        let hash = action_hash(&action).unwrap();
        let effect = PolicyEngine::new().evaluate(&action).unwrap().effect;
        let action_id = ActionId::from("action-durable");
        let provider_call_id = CallId::from(format!("provider-tool-durable-{fixture_id}"));
        store
            .record_action(
                &owner,
                ActionRecord {
                    action_id: action_id.clone(),
                    run_id: run_id.clone(),
                    step_id: step_id.clone(),
                    call_id: provider_call_id.clone(),
                    origin_model_call_id: call_id.clone(),
                    action: action.clone(),
                    action_hash: hash.clone(),
                    effect_class: effect,
                    occurred_at: "2026-07-12T00:00:03Z".into(),
                },
            )
            .unwrap();
        store
            .decide_policy(
                &owner,
                &run_id,
                &action_id,
                "2026-07-12T00:00:04Z",
            )
            .unwrap();
        Self {
            db,
            store,
            run_id,
            action_id,
            step_id,
            owner,
            action,
            hash,
            provider_call_id,
        }
    }

    fn binding(&self) -> ApprovalBinding {
        ApprovalBinding {
            run_id: self.run_id.clone(),
            action_id: self.action_id.clone(),
            action_hash: self.hash.clone(),
            workspace_identity: "workspace-durable".into(),
            policy_snapshot_hash: PolicyEngine::snapshot_hash(),
            config_snapshot_hash: "config-durable".into(),
        }
    }

    fn approval_input(&self, ttl_seconds: u64) -> ApprovalRequestInput {
        let now = unix_now();
        ApprovalRequestInput {
            approval_id: "approval-durable".into(),
            owner_actor_id: self.owner.clone(),
            binding: self.binding(),
            action_summary: self.action.action_summary(),
            risk: "medium".into(),
            rule_id: "dependency.install".into(),
            created_at: "2026-07-12T00:00:05Z".into(),
            expires_at: "2026-07-12T00:01:40Z".into(),
            created_at_unix: now.saturating_sub(1),
            expires_at_unix: now.saturating_add(ttl_seconds),
        }
    }

    fn tool_started_input(&self) -> orchester_laufzeit::harness::run_store::EventAppend {
        orchester_laufzeit::harness::run_store::EventAppend {
            turn_id: Some(TurnId::from("turn-durable")),
            step_id: Some(self.step_id.clone()),
            call_id: Some(self.provider_call_id.clone()),
            occurred_at: "2026-07-12T00:00:10Z".into(),
            kind: HarnessEventKind::ToolStarted {
                action_id: self.action_id.clone(),
            },
        }
    }

    fn tool_completed_input(&self, call_id: &CallId) -> EventAppend {
        EventAppend {
            turn_id: Some(TurnId::from("turn-durable")),
            step_id: Some(self.step_id.clone()),
            call_id: Some(call_id.clone()),
            occurred_at: "2026-07-12T00:00:11Z".into(),
            kind: HarnessEventKind::ToolCompleted {
                observation: Observation {
                    observation_id: ObservationId::from(format!("observation-{}", call_id.0)),
                    call_id: call_id.clone(),
                    kind: "read_file".into(),
                    summary: "ok".into(),
                    data: serde_json::json!({"bytes": 0}),
                },
            },
        }
    }

    fn tool_failed_input(&self, call_id: &CallId) -> EventAppend {
        EventAppend {
            turn_id: Some(TurnId::from("turn-durable")),
            step_id: Some(self.step_id.clone()),
            call_id: Some(call_id.clone()),
            occurred_at: "2026-07-12T00:00:11Z".into(),
            kind: HarnessEventKind::ToolFailed {
                feedback: FeedbackReport {
                    source: "read_file".into(),
                    validator_id: None,
                    exit_code: None,
                    classification: "tool_error".into(),
                    summary: "bounded failure".into(),
                    stdout_tail: String::new(),
                    stderr_tail: String::new(),
                    fingerprint: "tool-failure".into(),
                    retryable: false,
                },
            },
        }
    }

    fn start_allowed_tool(&self, audit_name: &str) {
        let barrier = PreExecutionBarrier::new(
            self.store.clone(),
            Arc::new(JsonlAuditSink::open(self.db.with_file_name(audit_name)).unwrap()),
        );
        let permit = barrier
            .prepare(
                &self.owner,
                &self.run_id,
                &self.action_id,
                ExecutionAuthorization::Allow,
                "ignored",
            )
            .unwrap();
        barrier
            .start_tool(&self.owner, &self.run_id, permit, self.tool_started_input())
            .unwrap();
    }

    fn persisted_tool_states(&self, call_id: &CallId) -> (String, String, String) {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row(
                "SELECT ta.state, a.state, s.status
                 FROM tool_attempts ta
                 JOIN actions a ON a.action_id = ta.action_id
                 JOIN steps s ON s.run_id = a.run_id AND s.step_id = a.step_id
                 WHERE a.action_id = ?1 AND ta.call_id = ?2",
                [&self.action_id.0, &call_id.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.db.parent().unwrap_or(Path::new(".")));
    }
}

struct FailingAudit;

impl AuditSink for FailingAudit {
    fn append_and_sync(
        &self,
        _input: AuditInput,
    ) -> Result<orchester_laufzeit::harness::audit::AuditReceipt, AuditError> {
        Err(AuditError::Io(std::io::Error::other(
            "injected audit failure",
        )))
    }
}
