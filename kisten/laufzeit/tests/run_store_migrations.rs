use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use orchester_laufzeit::harness::run_store::{SqliteRunStore, StoreError};

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn latest_schema_contains_bounded_append_only_transcript_records() {
    let root = std::env::temp_dir().join(format!(
        "orchester-transcript-schema-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);

    let connection = rusqlite::Connection::open(&db).unwrap();
    let columns: Vec<(String, String, bool)> = connection
        .prepare("PRAGMA table_info(transcript_records)")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)? == 1,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        vec![
            ("run_id".into(), "TEXT".into(), true),
            ("ordinal".into(), "INTEGER".into(), true),
            ("kind".into(), "TEXT".into(), true),
            ("call_id".into(), "TEXT".into(), false),
            ("wire_json".into(), "TEXT".into(), true),
            ("record_hash".into(), "TEXT".into(), true),
            ("created_at".into(), "TEXT".into(), true),
        ]
    );
    let unique: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_index_list('transcript_records')
             WHERE name = 'sqlite_autoindex_transcript_records_1' AND [unique] = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(unique, 1);
    let append_only_triggers: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'trigger' AND name IN (
               'trg_transcript_records_no_update',
               'trg_transcript_records_no_delete'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(append_only_triggers, 2);
    let binding_columns: Vec<(String, String, bool)> = connection
        .prepare("PRAGMA table_info(transcript_bindings)")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)? == 1,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        binding_columns,
        vec![
            ("run_id".into(), "TEXT".into(), true),
            ("event_sequence".into(), "INTEGER".into(), true),
            ("phase".into(), "TEXT".into(), true),
            ("first_ordinal".into(), "INTEGER".into(), false),
            ("last_ordinal".into(), "INTEGER".into(), false),
            ("record_count".into(), "INTEGER".into(), true),
        ]
    );
    let binding_index: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_index_list('transcript_bindings')
             WHERE name = 'idx_transcript_bindings_run_first' AND [unique] = 0",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(binding_index, 1);
    let binding_triggers: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'trigger' AND name IN (
               'trg_transcript_bindings_no_update',
               'trg_transcript_bindings_no_delete'
             )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(binding_triggers, 2);
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v1_state_database_is_upgraded_to_latest_before_use() {
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
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
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
    let step_columns: Vec<String> = connection
        .prepare("PRAGMA table_info(steps)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(step_columns.iter().any(|column| column == "model_phase"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_v1_openers_converge_on_latest_migration() {
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
        assert_eq!(opener.join().unwrap().unwrap(), 7);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v2_state_database_backfills_model_phase_and_action_origin() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v2-model-phase-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-v2', 'local_user', 'owner-v2-hash', '2026-07-13T00:00:00Z');
             INSERT INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at)
             VALUES('project-v2', '/workspace/v2', 'workspace-v2', '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z');
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-v2', 'project-v2', 'owner-v2', 'running', 2,
               'turn-v2', 'step-tool', 'policy-v2', 'config-v2', 8, 5,
               '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(run_id, step_ordinal, step_id, turn_id, status, model_call_id, started_at)
             VALUES
               ('run-v2', 1, 'step-created', 'turn-v2', 'created', NULL, '2026-07-13T00:00:01Z'),
               ('run-v2', 2, 'step-running', 'turn-v2', 'model_running', 'model-running', '2026-07-13T00:00:02Z'),
               ('run-v2', 3, 'step-model-done', 'turn-v2', 'model_running', 'model-done', '2026-07-13T00:00:03Z'),
               ('run-v2', 4, 'step-action', 'turn-v2', 'action_recorded', 'model-action', '2026-07-13T00:00:04Z'),
               ('run-v2', 5, 'step-tool', 'turn-v2', 'tool_running', 'model-tool', '2026-07-13T00:00:05Z');
             INSERT INTO events(
               run_id, sequence, schema_version, event_id, turn_id, step_id,
               call_id, kind, sanitized_payload, occurred_at
             ) VALUES(
               'run-v2', 1, 1, 'event-model-done', 'turn-v2',
               'step-model-done', 'model-done', 'model.completed', '{}',
               '2026-07-13T00:00:05Z'
             );
             INSERT INTO actions(
               action_id, run_id, step_id, call_id, kind, canonical_json,
               action_hash, effect_class, state, created_at
             ) VALUES(
               'action-v2', 'run-v2', 'step-action', 'provider-v2',
               'read_file', '{}', 'hash-v2', 'read_only_idempotent',
               'recorded', '2026-07-13T00:00:06Z'
             );",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let phases = connection
        .prepare("SELECT step_id, model_phase FROM steps ORDER BY step_ordinal")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        phases,
        vec![
            ("step-created".into(), "not_started".into()),
            ("step-running".into(), "running".into()),
            ("step-model-done".into(), "completed".into()),
            ("step-action".into(), "running".into()),
            ("step-tool".into(), "running".into()),
        ]
    );
    drop(connection);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let origin: String = connection
        .query_row(
            "SELECT origin_model_call_id FROM actions WHERE action_id = 'action-v2'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(origin, "model-action");
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unexpected_v2_trigger_is_rejected_and_upgrade_can_retry_after_removal() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v3-rollback-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-v3', 'local_user', 'owner-v3-hash', '2026-07-13T00:00:00Z');
             INSERT INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at)
             VALUES('project-v3', '/workspace/v3', 'workspace-v3', '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z');
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-v3', 'project-v3', 'owner-v3', 'running', 2,
               'turn-v3', 'step-v3', 'policy-v3', 'config-v3', 8, 1,
               '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(
               run_id, step_ordinal, step_id, turn_id, status, model_call_id, started_at
             ) VALUES(
               'run-v3', 1, 'step-v3', 'turn-v3', 'created', NULL,
               '2026-07-13T00:00:01Z'
             );
             CREATE TRIGGER fail_v3_version_write
             BEFORE INSERT ON schema_versions
             WHEN NEW.version = 3
             BEGIN
               SELECT RAISE(ABORT, 'injected v3 version write failure');
             END;",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    assert!(SqliteRunStore::open(&db).is_err());
    let connection = rusqlite::Connection::open(&db).unwrap();
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(steps)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let schema_version: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let user_version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert!(!columns.iter().any(|column| column == "model_phase"));
    assert_eq!(schema_version, 2);
    assert_eq!(user_version, 2);
    drop(connection);

    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch("DROP TRIGGER fail_v3_version_write")
        .unwrap();
    drop(connection);
    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unexpected_v3_trigger_is_rejected_and_upgrade_can_retry_after_removal() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v4-rollback-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0003_model_phase.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-v4', 'local_user', 'owner-v4-hash', '2026-07-13T00:00:00Z');
             INSERT INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at)
             VALUES('project-v4', '/workspace/v4', 'workspace-v4', '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z');
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-v4', 'project-v4', 'owner-v4', 'running', 2,
               'turn-v4', 'step-v4', 'policy-v4', 'config-v4', 8, 1,
               '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(
               run_id, step_ordinal, step_id, turn_id, status, model_call_id,
               started_at, model_phase
             ) VALUES(
               'run-v4', 1, 'step-v4', 'turn-v4', 'action_recorded',
               'model-v4', '2026-07-13T00:00:01Z', 'completed'
             );
             INSERT INTO actions(
               action_id, run_id, step_id, call_id, kind, canonical_json,
               action_hash, effect_class, state, created_at
             ) VALUES(
               'action-v4', 'run-v4', 'step-v4', 'provider-v4', 'read_file',
               '{}', 'hash-v4', 'read_only_idempotent', 'recorded',
               '2026-07-13T00:00:02Z'
             );
             CREATE TRIGGER fail_v4_version_write
             BEFORE INSERT ON schema_versions
             WHEN NEW.version = 4
             BEGIN
               SELECT RAISE(ABORT, 'injected v4 version write failure');
             END;",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    assert!(SqliteRunStore::open(&db).is_err());
    let connection = rusqlite::Connection::open(&db).unwrap();
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(actions)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let schema_version: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let user_version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert!(!columns
        .iter()
        .any(|column| column == "origin_model_call_id"));
    assert_eq!(schema_version, 3);
    assert_eq!(user_version, 3);
    connection
        .execute_batch("DROP TRIGGER fail_v4_version_write")
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let origin: String = connection
        .query_row(
            "SELECT origin_model_call_id FROM actions WHERE action_id = 'action-v4'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(origin, "model-v4");
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_v2_openers_converge_on_latest_migrations() {
    let root = std::env::temp_dir().join(format!(
        "orchester-concurrent-v3-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
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
        assert_eq!(opener.join().unwrap().unwrap(), 7);
    }
    std::fs::remove_dir_all(root).unwrap();
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
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_claiming_v3_without_model_phase_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-invalid-v3-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO schema_versions(version, applied_at) VALUES(3, CURRENT_TIMESTAMP);
             PRAGMA user_version = 3;",
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
        Err(StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_claiming_v3_with_weak_model_phase_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-weak-v3-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(
            "ALTER TABLE steps
               ADD COLUMN model_phase TEXT NOT NULL DEFAULT 'not_started';
             INSERT INTO schema_versions(version, applied_at) VALUES(3, CURRENT_TIMESTAMP);
             PRAGMA user_version = 3;",
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
        Err(StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_claiming_v4_without_action_origin_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-invalid-v4-{}-{}",
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
        .execute_batch(include_str!("../migrations/0002_approval_barrier.sql"))
        .unwrap();
    connection
        .execute_batch(include_str!("../migrations/0003_model_phase.sql"))
        .unwrap();
    connection
        .execute_batch(
            "INSERT INTO schema_versions(version, applied_at) VALUES(4, CURRENT_TIMESTAMP);
             PRAGMA user_version = 4;",
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
        Err(StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unexpected_v4_trigger_is_rejected_and_upgrade_can_retry_after_removal() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v5-rollback-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .execute_batch(
            "CREATE TRIGGER fail_v5_version_write
             BEFORE INSERT ON schema_versions
             WHEN NEW.version = 5
             BEGIN
               SELECT RAISE(ABORT, 'injected v5 version write failure');
             END;",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    assert!(SqliteRunStore::open(&db).is_err());
    let connection = rusqlite::Connection::open(&db).unwrap();
    let columns: Vec<String> = connection
        .prepare("PRAGMA table_info(observations)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let schema_version: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let user_version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert!(!columns.iter().any(|column| column == "outcome"));
    assert_eq!(schema_version, 4);
    assert_eq!(user_version, 4);
    connection
        .execute_batch("DROP TRIGGER fail_v5_version_write")
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let outcome_shape: (String, u32, Option<String>) = connection
        .query_row(
            "SELECT type, \"notnull\", dflt_value FROM pragma_table_info('observations')
             WHERE name = 'outcome'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        outcome_shape,
        ("TEXT".into(), 1, Some("'completed'".into()))
    );
    for index in ["idx_observations_call", "idx_tool_attempts_observation"] {
        let unique: u32 = connection
            .query_row(
                "SELECT 1 FROM pragma_index_list(?1) WHERE name = ?2 AND \"unique\" = 1",
                if index == "idx_observations_call" {
                    ["observations", index]
                } else {
                    ["tool_attempts", index]
                },
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unique, 1);
    }
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v4_terminal_attempts_receive_typed_absence_while_started_stays_unlinked() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v4-observation-backfill-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-v4-observation', 'local_user', 'owner-hash', '2026-07-13T00:00:00Z');
             INSERT INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at)
             VALUES('project-v4-observation', '/workspace/v4-observation', 'workspace-v4-observation',
                    '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z');
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-v4-observation', 'project-v4-observation', 'owner-v4-observation',
               'running', 2, 'turn-v4-observation', 'step-v4-started', 'policy-v4',
               'config-v4', 4, 2, '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(
               run_id, step_ordinal, step_id, turn_id, status, model_call_id,
               action_id, started_at, finished_at, model_phase
             ) VALUES
               ('run-v4-observation', 1, 'step-v4-completed', 'turn-v4-observation',
                'observed', 'model-v4-completed', 'action-v4-completed',
                '2026-07-13T00:00:01Z', '2026-07-13T00:00:03Z', 'completed'),
               ('run-v4-observation', 2, 'step-v4-started', 'turn-v4-observation',
                'tool_running', 'model-v4-started', 'action-v4-started',
                '2026-07-13T00:00:04Z', NULL, 'completed');
             INSERT INTO actions(
               action_id, run_id, step_id, call_id, kind, canonical_json,
               action_hash, effect_class, state, created_at, terminal_at,
               origin_model_call_id
             ) VALUES
               ('action-v4-completed', 'run-v4-observation', 'step-v4-completed',
                'call-v4-completed', 'read_file', '{}', 'hash-v4-completed',
                'read_only_idempotent', 'completed', '2026-07-13T00:00:01Z',
                '2026-07-13T00:00:03Z', 'model-v4-completed'),
               ('action-v4-started', 'run-v4-observation', 'step-v4-started',
                'call-v4-started', 'read_file', '{}', 'hash-v4-started',
                'read_only_idempotent', 'executing', '2026-07-13T00:00:04Z',
                NULL, 'model-v4-started');
             INSERT INTO tool_attempts(
               call_id, action_id, attempt_no, state, started_at, terminal_at,
               observation_id
             ) VALUES
               ('call-v4-completed', 'action-v4-completed', 1, 'completed',
                '2026-07-13T00:00:02Z', '2026-07-13T00:00:03Z', NULL),
               ('call-v4-started', 'action-v4-started', 1, 'started',
                '2026-07-13T00:00:05Z', NULL, NULL);",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    let connection = rusqlite::Connection::open(&db).unwrap();
    let attempts = connection
        .prepare(
            "SELECT attempt.call_id, attempt.state, attempt.observation_id,
                    observation.kind, observation.outcome, observation.sanitized_payload
             FROM tool_attempts AS attempt
             LEFT JOIN observations AS observation
               ON observation.observation_id = attempt.observation_id
             ORDER BY attempt.call_id",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        attempts,
        vec![
            (
                "call-v4-completed".into(),
                "completed".into(),
                Some("legacy-observation:call-v4-completed".into()),
                Some("tool.absent".into()),
                Some("absent".into()),
                Some("{\"reason\":\"legacy_unrecorded\"}".into()),
            ),
            (
                "call-v4-started".into(),
                "started".into(),
                None,
                None,
                None,
                None,
            ),
        ]
    );
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v5_migration_rejects_legacy_attempt_action_call_mismatch_atomically() {
    let root = std::env::temp_dir().join(format!(
        "orchester-v5-call-mismatch-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-v5-mismatch', 'local_user', 'owner-hash', '2026-07-13T00:00:00Z');
             INSERT INTO projects(project_id, canonical_root, workspace_identity, created_at, updated_at)
             VALUES('project-v5-mismatch', '/workspace/v5-mismatch', 'workspace-v5-mismatch',
                    '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z');
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-v5-mismatch', 'project-v5-mismatch', 'owner-v5-mismatch',
               'running', 2, 'turn-v5-mismatch', 'step-v5-mismatch', 'policy-v5',
               'config-v5', 4, 1, '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(
               run_id, step_ordinal, step_id, turn_id, status, model_call_id,
               action_id, started_at, finished_at, model_phase
             ) VALUES(
               'run-v5-mismatch', 1, 'step-v5-mismatch', 'turn-v5-mismatch',
               'observed', 'model-v5-mismatch', 'action-v5-mismatch',
               '2026-07-13T00:00:01Z', '2026-07-13T00:00:03Z', 'completed'
             );
             INSERT INTO actions(
               action_id, run_id, step_id, call_id, kind, canonical_json,
               action_hash, effect_class, state, created_at, terminal_at,
               origin_model_call_id
             ) VALUES(
               'action-v5-mismatch', 'run-v5-mismatch', 'step-v5-mismatch',
               'action-call-v5', 'read_file', '{}', 'hash-v5',
               'read_only_idempotent', 'completed', '2026-07-13T00:00:01Z',
               '2026-07-13T00:00:03Z', 'model-v5-mismatch'
             );
             INSERT INTO tool_attempts(
               call_id, action_id, attempt_no, state, started_at, terminal_at
             ) VALUES(
               'attempt-call-v5', 'action-v5-mismatch', 1, 'completed',
               '2026-07-13T00:00:02Z', '2026-07-13T00:00:03Z'
             );",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    assert!(SqliteRunStore::open(&db).is_err());
    let connection = rusqlite::Connection::open(&db).unwrap();
    let schema_version: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let outcome_exists: u32 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('observations') WHERE name = 'outcome'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, 4);
    assert_eq!(outcome_exists, 0);
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_claiming_v5_without_observation_shape_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-invalid-v5-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .execute_batch(
            "INSERT INTO schema_versions(version, applied_at) VALUES(5, CURRENT_TIMESTAMP);
             PRAGMA user_version = 5;",
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
        Err(StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn database_claiming_v5_with_weak_observation_links_is_rejected() {
    let root = std::env::temp_dir().join(format!(
        "orchester-weak-v5-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    let connection = rusqlite::Connection::open(&db).unwrap();
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ] {
        connection.execute_batch(migration).unwrap();
    }
    connection
        .execute_batch(
            "ALTER TABLE observations ADD COLUMN outcome TEXT NOT NULL DEFAULT 'completed'
               CHECK(outcome IN ('completed', 'failed', 'absent'));
             CREATE UNIQUE INDEX idx_observations_call ON observations(call_id);
             CREATE UNIQUE INDEX idx_observations_id_call
               ON observations(observation_id, call_id);
             CREATE UNIQUE INDEX idx_tool_attempts_observation
               ON tool_attempts(observation_id) WHERE observation_id IS NOT NULL;
             INSERT INTO schema_versions(version, applied_at) VALUES(5, CURRENT_TIMESTAMP);
             PRAGMA user_version = 5;",
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
        Err(StoreError::Corrupt)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn newer_schema_is_rejected_without_bootstrapping_v1_objects() {
    let (root, db) = temporary_database("future-schema");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE schema_versions(
               version INTEGER PRIMARY KEY,
               applied_at TEXT NOT NULL
             );
             INSERT INTO schema_versions(version, applied_at)
               VALUES(8, CURRENT_TIMESTAMP);
             PRAGMA user_version = 8;",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Invariant(_))
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![8]);
    assert_eq!(user_version(&connection), 8);
    assert!(!schema_object_exists(&connection, "table", "actors"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn claimed_v4_shape_is_rejected_before_v5_can_mutate_it() {
    let (root, db) = temporary_database("claimed-v4-shape");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 3);
    connection
        .execute_batch(
            "INSERT INTO schema_versions(version, applied_at)
               VALUES(4, CURRENT_TIMESTAMP);
             PRAGMA user_version = 4;",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1, 2, 3, 4]);
    assert_eq!(user_version(&connection), 4);
    assert!(!column_exists(&connection, "observations", "outcome"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn disagreeing_version_markers_are_rejected_without_advancing() {
    let (root, db) = temporary_database("marker-disagreement");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 4);
    connection
        .execute_batch("PRAGMA user_version = 3;")
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1, 2, 3, 4]);
    assert_eq!(user_version(&connection), 3);
    assert!(!column_exists(&connection, "observations", "outcome"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn gapped_version_ledger_is_rejected_without_advancing() {
    let (root, db) = temporary_database("gapped-ledger");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 4);
    connection
        .execute("DELETE FROM schema_versions WHERE version = 3", [])
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1, 2, 4]);
    assert_eq!(user_version(&connection), 4);
    assert!(!column_exists(&connection, "observations", "outcome"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn later_failure_rolls_the_entire_v1_to_latest_upgrade_back() {
    let (root, db) = temporary_database("atomic-full-upgrade");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 1);
    connection
        .execute_batch(
            "INSERT INTO actors(actor_id, kind, subject_hash, created_at)
             VALUES('owner-atomic', 'local_user', 'owner-atomic-hash',
                    '2026-07-13T00:00:00Z');
             INSERT INTO projects(
               project_id, canonical_root, workspace_identity, created_at, updated_at
             ) VALUES(
               'project-atomic', '/workspace/atomic', 'workspace-atomic',
               '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO runs(
               run_id, project_id, owner_actor_id, status, next_sequence,
               current_turn_id, current_step_id, policy_snapshot_hash,
               config_snapshot_hash, max_steps, steps_used, created_at, updated_at
             ) VALUES(
               'run-atomic', 'project-atomic', 'owner-atomic', 'running', 1,
               'turn-atomic', 'step-atomic', 'policy-atomic', 'config-atomic',
               4, 1, '2026-07-13T00:00:00Z', '2026-07-13T00:00:00Z'
             );
             INSERT INTO steps(
               run_id, step_ordinal, step_id, turn_id, status, model_call_id,
               started_at, finished_at
             ) VALUES(
               'run-atomic', 1, 'step-atomic', 'turn-atomic', 'observed',
               'model-atomic', '2026-07-13T00:00:01Z',
               '2026-07-13T00:00:03Z'
             );
             INSERT INTO actions(
               action_id, run_id, step_id, call_id, kind, canonical_json,
               action_hash, effect_class, state, created_at, terminal_at
             ) VALUES(
               'action-atomic', 'run-atomic', 'step-atomic', 'action-call-atomic',
               'read_file', '{}', 'hash-atomic', 'read_only_idempotent',
               'completed', '2026-07-13T00:00:01Z', '2026-07-13T00:00:03Z'
             );
             UPDATE steps SET action_id = 'action-atomic'
             WHERE step_id = 'step-atomic';
             INSERT INTO tool_attempts(
               call_id, action_id, attempt_no, state, started_at, terminal_at
             ) VALUES(
               'attempt-call-atomic', 'action-atomic', 1, 'completed',
               '2026-07-13T00:00:02Z', '2026-07-13T00:00:03Z'
             );",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(SqliteRunStore::open(&db).is_err());

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1]);
    assert_eq!(user_version(&connection), 0);
    assert!(!column_exists(&connection, "steps", "model_phase"));
    assert!(!column_exists(
        &connection,
        "actions",
        "origin_model_call_id"
    ));
    assert!(!column_exists(&connection, "observations", "outcome"));
    connection
        .execute(
            "UPDATE tool_attempts SET call_id = 'action-call-atomic'
             WHERE action_id = 'action-atomic'",
            [],
        )
        .unwrap();
    drop(connection);

    let store = SqliteRunStore::open(&db).unwrap();
    assert_eq!(store.schema_version().unwrap(), 7);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sqlite_with_user_objects_but_no_ledger_is_not_claimed() {
    let (root, db) = temporary_database("foreign-sqlite");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE user_notes(id INTEGER PRIMARY KEY, body TEXT NOT NULL);
             INSERT INTO user_notes(body) VALUES('leave me alone');",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert!(!schema_object_exists(
        &connection,
        "table",
        "schema_versions"
    ));
    assert!(!schema_object_exists(&connection, "table", "actors"));
    let note: String = connection
        .query_row("SELECT body FROM user_notes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(note, "leave me alone");
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_exact_historical_prefix_converges_on_latest_once() {
    for through in 1..=4 {
        let (root, db) = temporary_database(&format!("prefix-v{through}"));
        let connection = rusqlite::Connection::open(&db).unwrap();
        apply_state_migrations(&connection, through);
        drop(connection);
        prepare_database_permissions(&root, &db);

        let store = SqliteRunStore::open(&db).unwrap();
        assert_eq!(store.schema_version().unwrap(), 7);
        drop(store);

        let connection = rusqlite::Connection::open(&db).unwrap();
        assert_eq!(schema_versions(&connection), vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(user_version(&connection), 7);
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn marker_without_a_ledger_is_rejected_without_bootstrap() {
    let (root, db) = temporary_database("marker-without-ledger");
    let connection = rusqlite::Connection::open(&db).unwrap();
    connection
        .execute_batch("PRAGMA user_version = 8;")
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Invariant(_))
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(user_version(&connection), 8);
    assert!(!schema_object_exists(
        &connection,
        "table",
        "schema_versions"
    ));
    assert!(!schema_object_exists(&connection, "table", "actors"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn claimed_v1_with_weakened_table_constraints_is_rejected_without_advancing() {
    let (root, db) = temporary_database("weak-v1-constraints");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 1);
    connection
        .execute_batch(
            "DROP INDEX idx_messages_run_turn;
             ALTER TABLE messages RENAME TO messages_strong;
             CREATE TABLE messages(
               run_id TEXT,
               ordinal INTEGER,
               message_id TEXT,
               turn_id TEXT,
               role TEXT,
               item_kind TEXT,
               call_id TEXT,
               sanitized_payload TEXT,
               payload_hash TEXT,
               occurred_at TEXT
             );
             CREATE INDEX idx_messages_run_turn
               ON messages(run_id, turn_id, ordinal);
             DROP TABLE messages_strong;",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1]);
    assert_eq!(user_version(&connection), 0);
    assert!(!column_exists(&connection, "steps", "model_phase"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn claimed_v2_with_weakened_partial_index_is_rejected_without_advancing() {
    let (root, db) = temporary_database("weak-v2-partial-index");
    let connection = rusqlite::Connection::open(&db).unwrap();
    apply_state_migrations(&connection, 2);
    connection
        .execute_batch(
            "DROP INDEX idx_actions_policy_event;
             CREATE UNIQUE INDEX idx_actions_policy_event
               ON actions(policy_event_id) WHERE 0;",
        )
        .unwrap();
    drop(connection);
    prepare_database_permissions(&root, &db);

    assert!(matches!(
        SqliteRunStore::open(&db),
        Err(StoreError::Corrupt)
    ));

    let connection = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(schema_versions(&connection), vec![1, 2]);
    assert_eq!(user_version(&connection), 2);
    assert!(!column_exists(&connection, "steps", "model_phase"));
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

fn temporary_database(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "orchester-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("state.db");
    (root, db)
}

fn prepare_database_permissions(_root: &std::path::Path, _db: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(_root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(_db, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn apply_state_migrations(connection: &rusqlite::Connection, through: usize) {
    let migrations = [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_approval_barrier.sql"),
        include_str!("../migrations/0003_model_phase.sql"),
        include_str!("../migrations/0004_action_model_binding.sql"),
    ];
    for migration in migrations.iter().take(through) {
        connection.execute_batch(migration).unwrap();
    }
}

fn schema_versions(connection: &rusqlite::Connection) -> Vec<u32> {
    connection
        .prepare("SELECT version FROM schema_versions ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn user_version(connection: &rusqlite::Connection) -> u32 {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn schema_object_exists(connection: &rusqlite::Connection, kind: &str, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2
             )",
            rusqlite::params![kind, name],
            |row| row.get(0),
        )
        .unwrap()
}

fn column_exists(connection: &rusqlite::Connection, table: &str, column: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2
             )",
            rusqlite::params![table, column],
            |row| row.get(0),
        )
        .unwrap()
}
