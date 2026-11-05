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

#[test]
fn existing_self_agent_transcript_is_visible_after_reopening_history() {
    use orchester_laufzeit::harness::{
        run_store::{EventAppend, NewRun, RunStore, SqliteRunStore, Transition},
        service::WorkspaceIdentitySnapshot,
        transcript::TranscriptRecord,
    };
    use orchester_protokoll::HarnessEventKind;
    let root = TempRoot::new();
    let paths = root.paths();
    fs::create_dir_all(paths.workspace()).unwrap();
    let identity =
        WorkspaceIdentitySnapshot::for_workspace(paths.workspace(), "local-user").unwrap();
    let store = SqliteRunStore::open(paths.run_database()).unwrap();
    let run_id = orchester_protokoll::RunId::from("run-existing-conversation");
    store
        .create_run(NewRun {
            run_id: run_id.clone(),
            project_id: identity.project_id,
            owner_actor_id: "local-user".into(),
            canonical_root: identity.canonical_root,
            workspace_identity: identity.workspace_identity,
            policy_snapshot_hash: "policy".into(),
            config_snapshot_hash: "config".into(),
            max_steps: 4,
            occurred_at: "1793867086".into(),
        })
        .unwrap();
    store
        .append_transition(
            &run_id,
            "local-user",
            Transition::StartStep {
                turn_id: "turn-existing".into(),
                step_id: "step-existing".into(),
                occurred_at: "1793867086".into(),
            },
        )
        .unwrap();
    store
        .append_model_started_with_transcript(
            "local-user",
            &run_id,
            EventAppend {
                turn_id: Some("turn-existing".into()),
                step_id: Some("step-existing".into()),
                call_id: Some("model-existing".into()),
                occurred_at: "1793867086".into(),
                kind: HarnessEventKind::ModelStarted,
            },
            vec![TranscriptRecord::user("remember the existing conversation")],
        )
        .unwrap();
    drop(store);
    let history = SessionHistory::for_paths(&paths);
    let page = history.page(None, 20).unwrap();
    assert_eq!(
        page.items.len(),
        1,
        "an existing self-agent chat must survive reopening the application"
    );
    assert_eq!(
        history.detail(&page.items[0].id).unwrap().prompt,
        "remember the existing conversation"
    );
}

#[tokio::test]
async fn conversation_survives_two_restarts_and_continues_with_native_roles() {
    use orchester_laufzeit::harness::{
        agent_loop::{AgentLoopConfig, SelfAgentLoop},
        context::{ContextAssembler, ContextLimits},
        run_store::SqliteRunStore,
        service::SelfAgentService,
        transcript::TranscriptRecord,
    };
    use orchester_modell::{ModelResponse, ModelRole, ModelUsage, ScriptedLlm};
    use orchester_protokoll::{RunId, StopReason, UiEventKind};
    use tokio_util::sync::CancellationToken;

    let root = TempRoot::new();
    let paths = root.paths();
    fs::create_dir_all(paths.workspace()).unwrap();
    let mut conversation_id = None;
    for (index, prompt, answer) in [
        (1, "Remember my number: 42", "Your number is 42."),
        (2, "What was my number?", "42"),
    ] {
        // A new history and runtime on every turn reproduce an application restart.
        let history = SessionHistory::for_paths(&paths);
        let context = conversation_id
            .as_deref()
            .map(|id| history.conversation_context(id).unwrap())
            .unwrap_or_default();
        if index == 2 {
            assert_eq!(
                context,
                vec![
                    TranscriptRecord::user("Remember my number: 42"),
                    TranscriptRecord::assistant("Your number is 42.")
                ]
            );
        }
        let public_id = RunId::from(format!("public-turn-{index}"));
        let id = history
            .begin_conversation_turn(
                &public_id,
                conversation_id.as_deref(),
                prompt,
                Some("test-model"),
                context.len(),
            )
            .unwrap();
        let engine = SelfAgentLoop::new(
            ScriptedLlm::new([Ok(ModelResponse {
                assistant_text: answer.into(),
                tool_call: None,
                opaque_items: Vec::new(),
                usage: ModelUsage::default(),
            })]),
            ContextAssembler::new(ContextLimits::default(), Vec::new()),
            AgentLoopConfig {
                model: "test-model".into(),
                max_steps: 4,
                max_text_bytes: 65536,
                store: false,
            },
        )
        .unwrap();
        let service = SelfAgentService::new(
            engine,
            SqliteRunStore::open(paths.run_database()).unwrap(),
            paths.workspace(),
            "local-user",
        )
        .unwrap();
        service
            .start_with_history_and_run_id(
                prompt,
                context,
                Some(SessionHistory::planned_runtime_run_id(&public_id)),
                CancellationToken::new(),
                None,
            )
            .await
            .unwrap();
        let roles = &service.model().request_summaries()[0].message_roles;
        if index == 2 {
            assert_eq!(
                roles,
                &[
                    ModelRole::System,
                    ModelRole::User,
                    ModelRole::Assistant,
                    ModelRole::User
                ]
            );
        }
        history
            .record_conversation_event(
                &public_id,
                &UiEventKind::RunStopped {
                    reason: StopReason::Succeeded,
                },
            )
            .unwrap();
        conversation_id = Some(id);
    }
    let history = SessionHistory::for_paths(&paths);
    let page = history.page(None, 20).unwrap();
    assert_eq!(page.items.len(), 1, "two runs belong to one conversation");
    let detail = history.detail(conversation_id.as_deref().unwrap()).unwrap();
    assert_eq!(detail.run_ids.len(), 2);
    let text = detail
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::UserMessage { text } | UiEventKind::Message { text } => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        text,
        [
            "Remember my number: 42",
            "Your number is 42.",
            "What was my number?",
            "42"
        ]
    );
    assert!(detail
        .events
        .iter()
        .enumerate()
        .all(|(index, event)| event.sequence == index as u64 + 1));
    assert!(detail
        .events
        .iter()
        .all(|event| time::OffsetDateTime::parse(
            &event.occurred_at,
            &time::format_description::well_known::Rfc3339
        )
        .is_ok()));

    let other_workspace = root.0.join("other-workspace");
    fs::create_dir_all(&other_workspace).unwrap();
    let foreign = SessionHistory::for_paths(&OrchesterPaths::new(paths.home(), other_workspace));
    assert!(foreign.page(None, 20).unwrap().items.is_empty());
    assert!(matches!(
        foreign.conversation_context(conversation_id.as_deref().unwrap()),
        Err(SessionHistoryError::NotFound)
    ));
}

#[test]
fn failed_start_keeps_original_prompt_and_terminal_error_after_restart() {
    use orchester_protokoll::{RunId, StopReason, UiEventKind};
    let root = TempRoot::new();
    let paths = root.paths();
    fs::create_dir_all(paths.workspace()).unwrap();
    let history = SessionHistory::for_paths(&paths);
    let run_id = RunId::from("failed-before-runtime");
    let id = history
        .begin_conversation_turn(&run_id, None, "Please keep this question", None, 0)
        .unwrap();
    history
        .record_conversation_event(
            &run_id,
            &UiEventKind::Error {
                code: "provider".into(),
                message: "private provider failure".into(),
            },
        )
        .unwrap();
    history
        .record_conversation_event(
            &run_id,
            &UiEventKind::RunStopped {
                reason: StopReason::Failed,
            },
        )
        .unwrap();
    let detail = SessionHistory::for_paths(&paths).detail(&id).unwrap();
    assert_eq!(detail.prompt, "Please keep this question");
    assert_eq!(detail.summary.outcome, Outcome::Failed);
    assert_eq!(detail.events.len(), 3);
    assert!(matches!(detail.events[1].kind, UiEventKind::Error { .. }));
}
