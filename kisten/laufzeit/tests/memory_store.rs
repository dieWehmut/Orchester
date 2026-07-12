use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use orchester_laufzeit::harness::memory::{
    MemoryError, MemoryEventKind, MemoryProposal, MemoryState, MemoryStore, SecretCategory,
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
        source_run_id: Some("run-1".into()),
        source: "agent".into(),
        confidence: 0.8,
        created_at: created_at.into(),
    }
}

fn database_path(label: &str) -> PathBuf {
    let serial = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "orchester-memory-{label}-{}-{serial}.db",
        std::process::id()
    ))
}

fn remove_database(path: &Path) {
    for candidate in [
        path.to_path_buf(),
        PathBuf::from(format!("{}-wal", path.display())),
        PathBuf::from(format!("{}-shm", path.display())),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}

#[test]
fn recall_returns_only_accepted_items_for_the_same_project() {
    let store = MemoryStore::in_memory().unwrap();
    store
        .propose(proposal(
            "project-a",
            "m1",
            "Use rustfmt",
            "2026-07-12T09:00:00Z",
        ))
        .unwrap();
    store
        .approve("project-a", "m1", "owner-a", "2026-07-12T09:01:00Z")
        .unwrap();
    store
        .propose(proposal(
            "project-a",
            "m2",
            "rustfmt is not approved",
            "2026-07-12T09:02:00Z",
        ))
        .unwrap();
    store
        .propose(proposal(
            "project-b",
            "m3",
            "Use rustfmt in the other project",
            "2026-07-12T09:03:00Z",
        ))
        .unwrap();
    store
        .approve("project-b", "m3", "owner-b", "2026-07-12T09:04:00Z")
        .unwrap();

    let recalled = store.recall("project-a", "rustfmt", 5).unwrap();
    assert_eq!(
        recalled
            .iter()
            .map(|item| item.memory_id.as_str())
            .collect::<Vec<_>>(),
        ["m1"]
    );
    assert_eq!(
        store
            .approve("project-b", "m2", "owner-b", "2026-07-12T09:05:00Z")
            .unwrap_err(),
        MemoryError::NotFound
    );
}

#[test]
fn secret_candidates_are_rejected_before_any_database_write() {
    let configured = "configured-provider-credential-123";
    let store = MemoryStore::in_memory_with_secrets(vec![SecretString::from(configured)]).unwrap();
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
        let error = store
            .propose(proposal(
                "project-a",
                &format!("secret-{index}"),
                content,
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
        assert!(!format!("{error:?}").contains(content));
    }
    assert_eq!(store.count_all().unwrap(), 0);
}

#[test]
fn approve_reject_and_forget_are_scoped_compare_and_set_transitions() {
    let store = MemoryStore::in_memory().unwrap();
    store
        .propose(proposal(
            "project-a",
            "forgotten",
            "Prefer cargo nextest",
            "2026-07-12T09:00:00Z",
        ))
        .unwrap();
    store
        .approve("project-a", "forgotten", "owner-a", "2026-07-12T09:01:00Z")
        .unwrap();
    store
        .forget("project-a", "forgotten", "owner-a", "2026-07-12T09:02:00Z")
        .unwrap();

    let tombstone = store.get("project-a", "forgotten").unwrap();
    assert_eq!(tombstone.state, MemoryState::Forgotten);
    assert!(tombstone.content.is_empty());
    assert_eq!(tombstone.content_hash, None);
    assert!(store.recall("project-a", "nextest", 5).unwrap().is_empty());
    assert_eq!(
        store
            .approve("project-a", "forgotten", "owner-a", "2026-07-12T09:03:00Z",)
            .unwrap_err(),
        MemoryError::InvalidTransition
    );

    store
        .propose(proposal(
            "project-a",
            "rejected",
            "Do not recall me",
            "2026-07-12T09:04:00Z",
        ))
        .unwrap();
    store
        .reject("project-a", "rejected", "owner-a", "2026-07-12T09:05:00Z")
        .unwrap();
    assert_eq!(
        store.get("project-a", "rejected").unwrap().state,
        MemoryState::Rejected
    );
    assert_eq!(
        store
            .approve("project-a", "rejected", "owner-a", "2026-07-12T09:06:00Z",)
            .unwrap_err(),
        MemoryError::InvalidTransition
    );

    let events = store.events("project-a").unwrap();
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
    for (id, created_at) in [
        ("m-b", "2026-07-12T09:02:00Z"),
        ("m-a", "2026-07-12T09:02:00Z"),
        ("m-old", "2026-07-12T09:01:00Z"),
    ] {
        store
            .propose(proposal(
                "project-a",
                id,
                "rustfmt workspace convention",
                created_at,
            ))
            .unwrap();
        store
            .approve("project-a", id, "owner-a", "2026-07-12T09:03:00Z")
            .unwrap();
    }

    let recalled = store.recall("project-a", "rustfmt", 500).unwrap();
    assert_eq!(
        recalled
            .iter()
            .map(|item| item.memory_id.as_str())
            .collect::<Vec<_>>(),
        ["m-a", "m-b", "m-old"]
    );
    assert!(store
        .recall("project-a", r#"rustfmt\" OR project_id:*"#, 5)
        .unwrap()
        .is_empty());
}

#[test]
fn memory_database_reopens_without_cross_database_state() {
    let path = database_path("reopen");
    remove_database(&path);
    {
        let store = MemoryStore::open(&path).unwrap();
        store
            .propose(proposal(
                "project-a",
                "m1",
                "Keep run state separate",
                "2026-07-12T09:00:00Z",
            ))
            .unwrap();
        store
            .approve("project-a", "m1", "owner-a", "2026-07-12T09:01:00Z")
            .unwrap();
    }
    {
        let reopened = MemoryStore::open(&path).unwrap();
        assert_eq!(
            reopened
                .recall("project-a", "separate", 5)
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
    let mut invalid = proposal("project-a", "m1", "valid content", "2026-07-12T09:00:00Z");
    invalid.confidence = f64::NAN;
    assert_eq!(
        store.propose(invalid).unwrap_err(),
        MemoryError::InvalidInput
    );

    let oversized = "x".repeat(16 * 1024 + 1);
    assert_eq!(
        store
            .propose(proposal(
                "project-a",
                "m2",
                &oversized,
                "2026-07-12T09:00:00Z",
            ))
            .unwrap_err(),
        MemoryError::InvalidInput
    );
    assert_eq!(store.count_all().unwrap(), 0);
}
