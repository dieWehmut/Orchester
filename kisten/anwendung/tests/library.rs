use orchester_anwendung::{library::*, OrchesterPaths, SessionHistory};
use orchester_laufzeit::{SessionRecord, SessionStore};
use orchester_protokoll::{Outcome, Usage};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "orchester-library-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(p.join("workspace")).unwrap();
        Self(p)
    }
    fn paths(&self) -> OrchesterPaths {
        OrchesterPaths::new(self.0.join("home"), self.0.join("workspace"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn reopen_and_compare_and_swap_and_scope() {
    let f = Fixture::new();
    let paths = f.paths();
    let original = library_get(&paths).unwrap();
    let mut update = original.clone();
    update.sections.push(LibrarySectionDto {
        id: "section-one".into(),
        name: "Work".into(),
    });
    let saved = library_put(&paths, update).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(library_get(&f.paths()).unwrap(), saved);
    assert!(matches!(
        library_put(&paths, original),
        Err(LibraryError::Conflict)
    ));
    std::fs::create_dir_all(f.0.join("other")).unwrap();
    assert_eq!(
        library_get(&paths.with_workspace(f.0.join("other")))
            .unwrap()
            .revision,
        0
    );
}
#[test]
fn archive_preserves_history_and_invalid_references_are_rejected() {
    let f = Fixture::new();
    let paths = f.paths();
    SessionStore::new(paths.session_log())
        .append(&SessionRecord {
            recorded_at_unix: 1,
            agent: "codex".into(),
            session_id: None,
            prompt: "hello".into(),
            cwd: paths.workspace().into(),
            model: None,
            outcome: Outcome::Success,
            final_text: "world".into(),
            usage: Usage::default(),
        })
        .unwrap();
    let history = SessionHistory::for_paths(&paths);
    let id = history.page(None, 20).unwrap().items[0].id.clone();
    let before = std::fs::read(paths.session_log()).unwrap();
    let state = LibraryStateDto {
        sessions: vec![LibrarySessionDto {
            id: id.clone(),
            title: None,
            pinned: false,
            unread: false,
            archived: true,
            project_id: None,
            section_id: None,
        }],
        ..Default::default()
    };
    let saved = library_put(&paths, state).unwrap();
    assert!(history.detail(&id).is_ok());
    assert_eq!(before, std::fs::read(paths.session_log()).unwrap());
    let mut invalid = saved.clone();
    invalid.sessions[0].section_id = Some("missing".into());
    assert!(matches!(
        library_put(&paths, invalid),
        Err(LibraryError::Invalid)
    ));
    let mut invalid = saved;
    invalid.sessions[0].id = format!("s-{}", "0".repeat(32));
    assert!(matches!(
        library_put(&paths, invalid),
        Err(LibraryError::Invalid)
    ));
}

#[test]
fn missing_existing_folder_does_not_block_preferences() {
    let f = Fixture::new();
    let paths = f.paths();
    let folder = f.0.join("source");
    std::fs::create_dir(&folder).unwrap();
    let state = LibraryStateDto {
        projects: vec![LibraryProjectDto {
            id: "project".into(),
            name: "Project".into(),
            folders: vec![folder.to_str().unwrap().into()],
        }],
        ..Default::default()
    };
    let mut saved = library_put(&paths, state).unwrap();
    std::fs::remove_dir(folder).unwrap();
    saved.projects[0].name = "Renamed".into();
    let mut saved = library_put(&paths, saved).unwrap();
    saved.projects.clear();
    library_put(&paths, saved).unwrap();
}
