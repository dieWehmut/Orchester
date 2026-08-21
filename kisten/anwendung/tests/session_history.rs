use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use orchester_anwendung::{OrchesterPaths, SessionHistory, SessionHistoryError};
use orchester_laufzeit::{SessionRecord, SessionStore};
use orchester_protokoll::{Outcome, Usage};

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("orchester-session-history-{nonce}"));
        fs::create_dir_all(&root).expect("temp root");
        Self(root)
    }

    fn paths(&self) -> OrchesterPaths {
        OrchesterPaths::new(self.0.join("home"), self.0.join("workspace"))
    }

    fn session_log(&self) -> PathBuf {
        self.0.join("home").join("sessions.jsonl")
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(index: u64, prompt: &str, native_id: Option<&str>) -> SessionRecord {
    SessionRecord {
        recorded_at_unix: 1_800_000_000 + index,
        agent: "codex".to_owned(),
        session_id: native_id.map(str::to_owned),
        prompt: prompt.to_owned(),
        cwd: PathBuf::from(r"C:\private\workspace"),
        model: Some("gpt-5.6".to_owned()),
        outcome: Outcome::Success,
        final_text: format!("result {index}"),
        usage: Usage {
            input_tokens: index,
            output_tokens: index + 1,
            ..Usage::default()
        },
    }
}

#[test]
fn history_pages_newest_first_with_stable_opaque_cursors() {
    let root = TempRoot::new();
    let store = SessionStore::new(root.session_log());
    store.append(&record(1, "first", Some("native-1"))).unwrap();
    store
        .append(&record(2, "second", Some("native-2")))
        .unwrap();
    store.append(&record(3, "third", None)).unwrap();
    let paths = root.paths();

    let history = SessionHistory::for_paths(&paths);
    let first = history.page(None, 2).expect("first page");
    assert_eq!(first.items.len(), 2);
    assert_eq!(first.items[0].title, "third");
    assert_eq!(first.items[1].title, "second");
    assert!(first.items.iter().all(|item| item.id.starts_with("s-")));
    assert!(first.items.iter().all(|item| item.id.len() == 34));
    assert!(first.items.iter().all(|item| !item.resumable));

    let cursor = first.next_cursor.clone().expect("next cursor");
    let second = history.page(Some(&cursor), 2).expect("second page");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].title, "first");
    assert_eq!(second.next_cursor, None);

    let reloaded = SessionHistory::for_paths(&paths)
        .page(None, 3)
        .expect("reloaded page");
    let ids = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        reloaded
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn history_detail_is_path_free_and_keeps_native_ids_private() {
    let root = TempRoot::new();
    let store = SessionStore::new(root.session_log());
    store
        .append(&record(1, "inspect this project", Some("native-secret")))
        .unwrap();
    let history = SessionHistory::for_paths(&root.paths());
    let item = history.page(None, 1).expect("page").items.remove(0);

    let detail = history.detail(&item.id).expect("detail");
    assert_eq!(detail.prompt, "inspect this project");
    assert_eq!(detail.final_text, "result 1");
    assert!(!format!("{detail:?}").contains("native-secret"));
    assert!(!format!("{detail:?}").contains("private"));
    assert!(matches!(
        history.detail("not-a-session"),
        Err(SessionHistoryError::NotFound)
    ));
}

#[test]
fn history_bounds_page_size_and_rejects_unknown_cursors() {
    let root = TempRoot::new();
    let store = SessionStore::new(root.session_log());
    for index in 0..105 {
        store
            .append(&record(index, &format!("session {index}"), None))
            .unwrap();
    }
    let history = SessionHistory::for_paths(&root.paths());

    let minimum = history.page(None, 0).expect("minimum page");
    assert_eq!(minimum.items.len(), 1);
    assert!(minimum.next_cursor.is_some());

    let maximum = history.page(None, usize::MAX).expect("maximum page");
    assert_eq!(maximum.items.len(), 100);
    assert!(maximum.next_cursor.is_some());

    assert!(matches!(
        history.page(Some("s-00000000000000000000000000000000"), 20),
        Err(SessionHistoryError::InvalidCursor)
    ));
}
