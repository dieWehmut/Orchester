use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use orchester_laufzeit::harness::run_store::{SqliteRunStore, StoreError};

static NEXT: AtomicUsize = AtomicUsize::new(0);

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
    assert_eq!(store.schema_version().unwrap(), 3);
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
        assert_eq!(opener.join().unwrap().unwrap(), 3);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v2_state_database_backfills_model_phase() {
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
    assert_eq!(store.schema_version().unwrap(), 3);
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
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v3_migration_rolls_back_when_version_write_fails() {
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
    assert_eq!(store.schema_version().unwrap(), 3);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_v2_openers_converge_on_one_v3_migration() {
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
        assert_eq!(opener.join().unwrap().unwrap(), 3);
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
