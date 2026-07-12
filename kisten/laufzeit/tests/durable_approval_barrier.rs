use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalError, ApprovalRequestInput, ApprovalState, DurableApprovalStore,
};
use orchester_laufzeit::harness::audit::{AuditError, AuditInput, AuditSink, JsonlAuditSink};
use orchester_laufzeit::harness::barrier::{ExecutionAuthorization, PreExecutionBarrier};
use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, NewRun, RunStore, SqliteRunStore, Transition,
};
use orchester_protokoll::{
    ActionId, AgentAction, CallId, HarnessEventKind, PolicyDecision, RunId, StepId, TurnId,
};

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
    assert_eq!(fixture.store.schema_version().unwrap(), 2);
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
fn v1_state_database_is_upgraded_before_use() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v1-migration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(include_str!("../migrations/0001_state.sql"))
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 2);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(approvals)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(columns.iter().any(|column| column == "run_id"));
    assert!(columns.iter().any(|column| column == "expires_at_unix"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn concurrent_v1_openers_converge_on_one_v2_migration() {
    let root = std::env::temp_dir().join(format!(
        "orchester-concurrent-migration-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(include_str!("../migrations/0001_state.sql"))
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let start = Arc::new(std::sync::Barrier::new(2));
    let mut openers = Vec::new();
    for _ in 0..2 {
        let db = db.clone();
        let start = start.clone();
        openers.push(thread::spawn(move || {
            start.wait();
            SqliteRunStore::open(db).map(|store| store.schema_version().unwrap())
        }));
    }
    for opener in openers {
        assert_eq!(opener.join().unwrap().unwrap(), 2);
    }
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn database_claiming_v2_without_v2_shape_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-invalid-v2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(include_str!("../migrations/0001_state.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO schema_versions(version, applied_at) VALUES(2, CURRENT_TIMESTAMP);
             PRAGMA user_version = 2;",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(orchester_laufzeit::harness::run_store::StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).ok();
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
}

impl Fixture {
    fn new(decision: PolicyDecision) -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchester-durable-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let db = root.join("state.db");
        let store = Arc::new(SqliteRunStore::open(&db).unwrap());
        let run_id = RunId::from("run-durable");
        let owner = "owner-durable".to_owned();
        store
            .create_run(NewRun {
                run_id: run_id.clone(),
                project_id: "project-durable".into(),
                owner_actor_id: owner.clone(),
                canonical_root: root.to_string_lossy().into_owned(),
                workspace_identity: "workspace-durable".into(),
                policy_snapshot_hash: "policy-durable".into(),
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
            .append_event(
                &owner,
                &run_id,
                orchester_laufzeit::harness::run_store::EventAppend {
                    turn_id: Some(turn_id.clone()),
                    step_id: Some(step_id.clone()),
                    call_id: Some(call_id.clone()),
                    occurred_at: "2026-07-12T00:00:02Z".into(),
                    kind: HarnessEventKind::ModelStarted,
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
        store
            .record_action(
                &owner,
                ActionRecord {
                    action_id: action_id.clone(),
                    run_id: run_id.clone(),
                    step_id: step_id.clone(),
                    call_id: call_id.clone(),
                    action: action.clone(),
                    action_hash: hash.clone(),
                    effect_class: effect,
                    occurred_at: "2026-07-12T00:00:03Z".into(),
                },
            )
            .unwrap();
        store
            .append_event(
                &owner,
                &run_id,
                orchester_laufzeit::harness::run_store::EventAppend {
                    turn_id: Some(turn_id),
                    step_id: Some(step_id.clone()),
                    call_id: None,
                    occurred_at: "2026-07-12T00:00:04Z".into(),
                    kind: HarnessEventKind::PolicyDecided {
                        action_id: action_id.clone(),
                        decision,
                        rule_id: if decision == PolicyDecision::Allow {
                            "workspace.read".into()
                        } else {
                            "dependency.install".into()
                        },
                    },
                },
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
        }
    }

    fn binding(&self) -> ApprovalBinding {
        ApprovalBinding {
            run_id: self.run_id.clone(),
            action_id: self.action_id.clone(),
            action_hash: self.hash.clone(),
            workspace_identity: "workspace-durable".into(),
            policy_snapshot_hash: "policy-durable".into(),
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
            call_id: Some(CallId::from("tool-call-durable")),
            occurred_at: "2026-07-12T00:00:10Z".into(),
            kind: HarnessEventKind::ToolStarted {
                action_id: self.action_id.clone(),
            },
        }
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
