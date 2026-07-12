use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_laufzeit::harness::memory::{
    MemoryAccess, MemoryError, MemoryEventKind, MemoryProposal, MemoryState, MemoryStore,
    SecretCategory,
};
use orchester_protokoll::MemoryKind;
use secrecy::SecretString;

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

fn proposal(project: &str, id: &str, content: &str, created_at: &str) -> MemoryProposal {
    MemoryProposal {
        memory_id: id.into(),
        project_id: project.into(),
        kind: MemoryKind::Convention,
        content: content.into(),
        source_run_id: Some(format!("run-{project}")),
        source: "agent".into(),
        confidence: 0.8,
        created_at: created_at.into(),
    }
}

fn database_path(label: &str) -> PathBuf {
    let serial = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir()
        .join(format!(
            "orchester-memory-{label}-{}-{serial}",
            std::process::id()
        ))
        .join("memory.db")
}

fn remove_database(path: &Path) {
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

fn access<'a>(store: &'a MemoryStore, project_id: &str, actor_id: &str) -> MemoryAccess<'a> {
    store
        .register_project(project_id, actor_id, "2026-07-12T08:00:00Z")
        .unwrap();
    store.access(project_id, actor_id).unwrap()
}

fn approve(access: &MemoryAccess<'_>, memory_id: &str, decided_at: &str) {
    let hash = access.get(memory_id).unwrap().content_hash.unwrap();
    access.approve(memory_id, &hash, decided_at).unwrap();
}

fn reject(access: &MemoryAccess<'_>, memory_id: &str, decided_at: &str) {
    let hash = access.get(memory_id).unwrap().content_hash.unwrap();
    access.reject(memory_id, &hash, decided_at).unwrap();
}

fn forget(access: &MemoryAccess<'_>, memory_id: &str, decided_at: &str) {
    let hash = access.get(memory_id).unwrap().content_hash.unwrap();
    access.forget(memory_id, &hash, decided_at).unwrap();
}

#[test]
fn recall_returns_only_accepted_items_for_the_same_project() {
    let store = MemoryStore::in_memory().unwrap();
    let owner_a = access(&store, "project-a", "owner-a");
    let owner_b = access(&store, "project-b", "owner-b");
    let project_a = owner_a
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    let project_b = owner_b
        .agent_access("run-project-b", "2026-07-12T08:01:00Z")
        .unwrap();
    project_a
        .propose(proposal(
            "project-a",
            "m1",
            "Use rustfmt",
            "2026-07-12T09:00:00Z",
        ))
        .unwrap();
    approve(&owner_a, "m1", "2026-07-12T09:01:00Z");
    project_a
        .propose(proposal(
            "project-a",
            "m2",
            "rustfmt is not approved",
            "2026-07-12T09:02:00Z",
        ))
        .unwrap();
    project_b
        .propose(proposal(
            "project-b",
            "m3",
            "Use rustfmt in the other project",
            "2026-07-12T09:03:00Z",
        ))
        .unwrap();
    approve(&owner_b, "m3", "2026-07-12T09:04:00Z");

    let recalled = owner_a.recall("rustfmt", 5).unwrap();
    assert_eq!(
        recalled
            .iter()
            .map(|item| item.memory_id.as_str())
            .collect::<Vec<_>>(),
        ["m1"]
    );
    assert_eq!(
        owner_b
            .approve("m2", &"0".repeat(64), "2026-07-12T09:05:00Z")
            .unwrap_err(),
        MemoryError::NotFound
    );
}

#[test]
fn secret_candidates_are_rejected_before_any_database_write() {
    let configured = "configured-provider-credential-123";
    let store = MemoryStore::in_memory_with_secrets(vec![SecretString::from(configured)]).unwrap();
    let owner = access(&store, "project-a", "owner-a");
    let project = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    let provider_token = format!("{}{}", "sk", "-proj-1234567890abcdef");
    let fixtures = vec![
        (configured.to_owned(), SecretCategory::ConfiguredCredential),
        (
            "Authorization: Bearer hidden-value".to_owned(),
            SecretCategory::AuthorizationHeader,
        ),
        (
            "-----BEGIN PRIVATE KEY-----".to_owned(),
            SecretCategory::PrivateKey,
        ),
        (provider_token, SecretCategory::ProviderToken),
        (
            "A9vQ2mL7xR4pK8wT1zN6cF3hJ0sD5yB2".to_owned(),
            SecretCategory::HighEntropyToken,
        ),
    ];

    for (index, (content, expected_category)) in fixtures.into_iter().enumerate() {
        let error = project
            .propose(proposal(
                "project-a",
                &format!("secret-{index}"),
                &content,
                "2026-07-12T09:00:00Z",
            ))
            .unwrap_err();
        let MemoryError::SecretDetected {
            category,
            start,
            end,
        } = error
        else {
            panic!("expected a typed secret rejection");
        };
        assert_eq!(category, expected_category);
        assert!(end > start);
        assert!(!format!("{error:?}").contains(&content));
    }
    assert_eq!(owner.count().unwrap(), 0);
}

#[test]
fn approve_reject_and_forget_are_scoped_compare_and_set_transitions() {
    let store = MemoryStore::in_memory().unwrap();
    let owner = access(&store, "project-a", "owner-a");
    let project = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    project
        .propose(proposal(
            "project-a",
            "forgotten",
            "Prefer cargo nextest",
            "2026-07-12T09:00:00Z",
        ))
        .unwrap();
    approve(&owner, "forgotten", "2026-07-12T09:01:00Z");
    forget(&owner, "forgotten", "2026-07-12T09:02:00Z");

    let tombstone = owner.get("forgotten").unwrap();
    assert_eq!(tombstone.state, MemoryState::Forgotten);
    assert!(tombstone.content.is_empty());
    assert_eq!(tombstone.content_hash, None);
    assert!(owner.recall("nextest", 5).unwrap().is_empty());
    assert_eq!(
        owner
            .approve("forgotten", &"0".repeat(64), "2026-07-12T09:03:00Z",)
            .unwrap_err(),
        MemoryError::InvalidTransition
    );

    project
        .propose(proposal(
            "project-a",
            "rejected",
            "Do not recall me",
            "2026-07-12T09:04:00Z",
        ))
        .unwrap();
    reject(&owner, "rejected", "2026-07-12T09:05:00Z");
    assert_eq!(owner.get("rejected").unwrap().state, MemoryState::Rejected);
    assert_eq!(
        owner
            .approve("rejected", &"0".repeat(64), "2026-07-12T09:06:00Z")
            .unwrap_err(),
        MemoryError::InvalidTransition
    );

    let events = owner.events().unwrap();
    assert_eq!(
        events.iter().map(|event| event.kind).collect::<Vec<_>>(),
        [
            MemoryEventKind::Proposed,
            MemoryEventKind::Approved,
            MemoryEventKind::Forgotten,
            MemoryEventKind::Proposed,
            MemoryEventKind::Rejected,
        ]
    );
    let serialized = format!("{events:?}");
    assert!(!serialized.contains("nextest"));
    assert!(!serialized.contains("recall me"));
}

#[test]
fn recall_is_deterministic_capped_and_treats_query_as_text() {
    let store = MemoryStore::in_memory().unwrap();
    let owner = access(&store, "project-a", "owner-a");
    let project = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    for (id, created_at) in [
        ("m-b", "2026-07-12T09:02:00Z"),
        ("m-a", "2026-07-12T09:02:00Z"),
        ("m-old", "2026-07-12T09:01:00Z"),
    ] {
        project
            .propose(proposal(
                "project-a",
                id,
                "rustfmt workspace convention",
                created_at,
            ))
            .unwrap();
        approve(&owner, id, "2026-07-12T09:03:00Z");
    }

    let recalled = owner.recall("rustfmt", 500).unwrap();
    assert_eq!(
        recalled
            .iter()
            .map(|item| item.memory_id.as_str())
            .collect::<Vec<_>>(),
        ["m-a", "m-b", "m-old"]
    );
    assert!(owner
        .recall(r#"rustfmt\" OR project_id:*"#, 5)
        .unwrap()
        .is_empty());
}

#[test]
fn memory_database_reopens_without_cross_database_state() {
    let path = database_path("reopen");
    remove_database(&path);
    {
        let store = MemoryStore::open(&path).unwrap();
        let owner = access(&store, "project-a", "owner-a");
        let project = owner
            .agent_access("run-project-a", "2026-07-12T08:01:00Z")
            .unwrap();
        project
            .propose(proposal(
                "project-a",
                "m1",
                "Keep run state separate",
                "2026-07-12T09:00:00Z",
            ))
            .unwrap();
        approve(&owner, "m1", "2026-07-12T09:01:00Z");
    }
    {
        let reopened = MemoryStore::open(&path).unwrap();
        let project = reopened.access("project-a", "owner-a").unwrap();
        assert_eq!(
            project
                .recall("separate", 5)
                .unwrap()
                .first()
                .map(|item| item.memory_id.as_str()),
            Some("m1")
        );
    }
    remove_database(&path);
}

#[cfg(unix)]
#[test]
fn memory_database_requires_user_only_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let path = database_path("permissions");
    remove_database(&path);
    drop(MemoryStore::open(&path).unwrap());
    let secure_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(secure_mode & 0o077, 0);

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        MemoryStore::open(&path).unwrap_err(),
        MemoryError::InsecurePermissions
    );
    remove_database(&path);
}

#[test]
fn invalid_metadata_and_oversized_content_fail_closed() {
    let store = MemoryStore::in_memory().unwrap();
    let owner = access(&store, "project-a", "owner-a");
    let project = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    let mut invalid = proposal("project-a", "m1", "valid content", "2026-07-12T09:00:00Z");
    invalid.confidence = f64::NAN;
    assert_eq!(
        project.propose(invalid).unwrap_err(),
        MemoryError::InvalidInput
    );

    let oversized = "x".repeat(16 * 1024 + 1);
    assert_eq!(
        project
            .propose(proposal(
                "project-a",
                "m2",
                &oversized,
                "2026-07-12T09:00:00Z",
            ))
            .unwrap_err(),
        MemoryError::InvalidInput
    );
    assert_eq!(owner.count().unwrap(), 0);
}

#[test]
fn run_bound_agent_can_only_propose_and_owner_approval_is_hash_bound() {
    let store = MemoryStore::in_memory().unwrap();
    let owner = access(&store, "project-a", "owner-a");
    assert_eq!(
        owner
            .propose(proposal(
                "project-a",
                "owner-proposal",
                "owner must not self-propose",
                "2026-07-12T09:00:00Z",
            ))
            .unwrap_err(),
        MemoryError::AuthorizationDenied
    );

    let agent = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();
    let mut wrong_run = proposal(
        "project-a",
        "wrong-run",
        "wrong provenance",
        "2026-07-12T09:00:00Z",
    );
    wrong_run.source_run_id = Some("run-other".into());
    assert_eq!(
        agent.propose(wrong_run).unwrap_err(),
        MemoryError::AuthorizationDenied
    );

    let item = agent
        .propose(proposal(
            "project-a",
            "m1",
            "Bind approval to this exact content",
            "2026-07-12T09:00:00Z",
        ))
        .unwrap();
    let hash = item.content_hash.clone().unwrap();
    assert_eq!(item.proposed_by_actor_id, "agent:run-project-a");
    assert_eq!(
        agent
            .approve("m1", &hash, "2026-07-12T09:01:00Z")
            .unwrap_err(),
        MemoryError::AuthorizationDenied
    );
    assert_eq!(
        owner
            .approve("m1", &"0".repeat(64), "2026-07-12T09:01:00Z")
            .unwrap_err(),
        MemoryError::StaleApproval
    );
    assert_eq!(
        owner
            .approve("m1", &hash, "2026-07-12T09:01:00Z")
            .unwrap()
            .state,
        MemoryState::Accepted
    );
    assert_eq!(
        store.access("project-a", "owner-b").unwrap_err(),
        MemoryError::AuthorizationDenied
    );
}

#[test]
fn control_format_and_metadata_secrets_never_enter_memory() {
    let configured = "metadata-provider-credential-987";
    let store = MemoryStore::in_memory_with_secrets(vec![SecretString::from(configured)]).unwrap();
    let owner = access(&store, "project-a", "owner-a");
    let agent = owner
        .agent_access("run-project-a", "2026-07-12T08:01:00Z")
        .unwrap();

    for (id, content) in [
        ("ansi", "clear\u{1b}[2Jscreen"),
        ("zero-width", "split\u{200b}identifier"),
        ("bidi", "review\u{202e}txt"),
        ("carriage-return", "replace\rline"),
    ] {
        assert_eq!(
            agent
                .propose(proposal("project-a", id, content, "2026-07-12T09:00:00Z",))
                .unwrap_err(),
            MemoryError::InvalidInput
        );
    }

    let mut metadata_secret = proposal(
        "project-a",
        "metadata-secret",
        "ordinary content",
        "2026-07-12T09:00:00Z",
    );
    metadata_secret.source = configured.into();
    assert!(matches!(
        agent.propose(metadata_secret),
        Err(MemoryError::SecretDetected {
            category: SecretCategory::ConfiguredCredential,
            ..
        })
    ));
    assert_eq!(owner.count().unwrap(), 0);
}

#[test]
fn tampered_content_hash_is_rejected_on_reopen() {
    let path = database_path("tamper");
    remove_database(&path);
    {
        let store = MemoryStore::open(&path).unwrap();
        let owner = access(&store, "project-a", "owner-a");
        let agent = owner
            .agent_access("run-project-a", "2026-07-12T08:01:00Z")
            .unwrap();
        agent
            .propose(proposal(
                "project-a",
                "m1",
                "trusted memory content",
                "2026-07-12T09:00:00Z",
            ))
            .unwrap();
    }
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE memory_items SET content = 'poisoned memory content' WHERE memory_id = 'm1'",
                [],
            )
            .unwrap();
    }
    assert_eq!(MemoryStore::open(&path).unwrap_err(), MemoryError::Corrupt);
    remove_database(&path);
}

#[test]
fn forgetting_removes_plaintext_from_database_and_wal_files() {
    let path = database_path("secure-forget");
    remove_database(&path);
    let marker = "forgotten marker plain text 47023";
    {
        let store = MemoryStore::open(&path).unwrap();
        let owner = access(&store, "project-a", "owner-a");
        let agent = owner
            .agent_access("run-project-a", "2026-07-12T08:01:00Z")
            .unwrap();
        agent
            .propose(proposal("project-a", "m1", marker, "2026-07-12T09:00:00Z"))
            .unwrap();
        forget(&owner, "m1", "2026-07-12T09:01:00Z");
    }
    for candidate in [
        path.clone(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        if candidate.exists() {
            let bytes = std::fs::read(candidate).unwrap();
            assert!(!bytes
                .windows(marker.len())
                .any(|window| window == marker.as_bytes()));
        }
    }
    remove_database(&path);
}

#[test]
fn concurrent_first_open_applies_each_memory_migration_once() {
    use std::sync::{Arc, Barrier};

    let path = database_path("concurrent-migration");
    remove_database(&path);
    let barrier = Arc::new(Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                MemoryStore::open(path).map(|_| ())
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    remove_database(&path);
}

#[test]
fn unrelated_state_schema_is_rejected_without_memory_migration() {
    let path = database_path("schema-isolation");
    remove_database(&path);
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_versions(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 INSERT INTO schema_versions VALUES(1, '2026-07-12T09:00:00Z');
                 CREATE TABLE runs(run_id TEXT PRIMARY KEY);",
            )
            .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(
        MemoryStore::open(&path).unwrap_err(),
        MemoryError::UnsupportedSchema
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    let migrated: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'memory_projects')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!migrated);
    drop(connection);
    remove_database(&path);
}

#[test]
fn nonempty_legacy_memory_requires_explicit_review_before_owner_migration() {
    let path = database_path("legacy-owner-migration");
    remove_database(&path);
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("../migrations/0001_memory.sql"))
            .unwrap();
        connection
            .execute(
                "INSERT INTO memory_items(
                   memory_id, project_id, state, kind, content, content_hash,
                   source, confidence, created_at
                 ) VALUES(?1, ?2, 'proposed', 'lesson', ?3, ?4, 'legacy', 1.0, ?5)",
                rusqlite::params![
                    "legacy-1",
                    "project-a",
                    "legacy memory",
                    "50125ee17f19c13973c9324e1c6c26dcfac5c47f26757bf2d1b9e887b9102f3d",
                    "2026-07-12T09:00:00Z"
                ],
            )
            .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(
        MemoryStore::open(&path).unwrap_err(),
        MemoryError::UnsupportedSchema
    );
    let connection = rusqlite::Connection::open(&path).unwrap();
    let versions = connection
        .prepare("SELECT version FROM schema_versions ORDER BY version")
        .unwrap()
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(versions, [1]);
    drop(connection);
    remove_database(&path);
}
