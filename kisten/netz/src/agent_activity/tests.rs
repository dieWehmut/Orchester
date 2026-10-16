use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tower::ServiceExt;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "orchester-activity-{}-{}",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn file(&self, relative: &str, records: &[serde_json::Value]) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let body = records.iter().map(|v| format!("{v}\n")).collect::<String>();
        fs::write(&path, body).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.0.starts_with(std::env::temp_dir())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchester-activity-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn codex_message(role: &str, text: &str) -> serde_json::Value {
    json!({"type":"response_item","payload":{"type":"message","role":role,"content":[{"type":"output_text","text":text}]}})
}

#[test]
fn title_is_not_replaced_by_followup_and_tools_share_opaque_call_ids() {
    let records = [codex_message("user", "Build the sidebar"), codex_message("user", "Continue"),
        json!({"type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"private-call","arguments":"echo done"}}),
        json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"private-call","output":"done"}}),
    ].into_iter().enumerate().map(|(i,v)| (i as u64,v)).collect::<Vec<_>>();
    let row = transcript::parse("codex", "task", now_ms(), &records);
    assert_eq!(row.summary.title, "Build the sidebar");
    assert!(row.entries[2].call_id.is_some());
    assert_eq!(row.entries[2].call_id, row.entries[3].call_id);
    assert!(!serde_json::to_string(&row)
        .unwrap()
        .contains("private-call"));
}

#[test]
fn codex_index_uses_source_titles_and_projects_without_following_outside_paths() {
    use rusqlite::{params, Connection};
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let source = fixture.file(
        "sessions/2026/one.jsonl",
        &[codex_message("user", "Initial prompt")],
    );
    let other = outside.file("outside.jsonl", &[codex_message("user", "OUTSIDE")]);
    let db = Connection::open(fixture.0.join("state_5.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE threads (id TEXT, rollout_path TEXT, title TEXT, cwd TEXT, updated_at INTEGER, archived INTEGER, is_pinned INTEGER, project_id TEXT); CREATE TABLE projects (id TEXT, name TEXT);").unwrap();
    db.execute("INSERT INTO projects VALUES ('project', 'Research')", [])
        .unwrap();
    for (id, path) in [("one", source), ("other", other)] {
        db.execute(
            "INSERT INTO threads VALUES (?1, ?2, 'Saved title', '/work/demo', 10, 0, 1, 'project')",
            params![id, path.to_string_lossy()],
        )
        .unwrap();
    }
    let mut cache = HashMap::new();
    let (_, rows) = files::read_provider("codex", Some(&fixture.0.join("sessions")), &mut cache);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].summary.title, "Saved title");
    assert_eq!(rows[0].summary.project, "Research");
    assert_eq!(rows[0].summary.pinned, Some(true));
    assert!(rows[0].summary.project_id.is_some());
    db.execute("UPDATE threads SET title='Renamed task' WHERE id='one'", [])
        .unwrap();
    let (_, rows) = files::read_provider("codex", Some(&fixture.0.join("sessions")), &mut cache);
    assert_eq!(rows[0].summary.title, "Renamed task");
    assert!(!serde_json::to_string(&rows[0]).unwrap().contains("OUTSIDE"));
    db.execute("ALTER TABLE threads ADD COLUMN name TEXT", [])
        .unwrap();
    db.execute(
        "UPDATE threads SET name='Sidebar name', is_pinned=0 WHERE id='one'",
        [],
    )
    .unwrap();
    fs::write(
        fixture.0.join(".codex-global-state.json"),
        r#"{"pinned-thread-ids":["one"]}"#,
    )
    .unwrap();
    let (_, rows) = files::read_provider("codex", Some(&fixture.0.join("sessions")), &mut cache);
    assert_eq!(rows[0].summary.title, "Sidebar name");
    assert_eq!(rows[0].summary.pinned, Some(true));
}

#[test]
fn codex_preserves_source_turn_timing_and_visible_message_phases() {
    let values = [
        json!({"type":"event_msg","timestamp":"2026-10-12T01:00:00Z","payload":{"type":"task_started","turn_id":"source-turn"}}),
        json!({"type":"turn_context","payload":{"turn_id":"source-turn"}}),
        codex_message("user", "Continue"),
        json!({"type":"response_item","timestamp":"2026-10-12T01:00:01Z","payload":{"type":"message","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"Checking"}]}}),
        json!({"type":"response_item","timestamp":"2026-10-12T01:21:58Z","payload":{"type":"message","role":"assistant","phase":"final","content":[{"type":"output_text","text":"**Done**"}]}}),
        json!({"type":"event_msg","timestamp":"2026-10-12T01:21:59Z","payload":{"type":"task_complete","turn_id":"source-turn"}}),
        codex_message("user", "Next prompt without lifecycle"),
    ];
    let records = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as u64, v))
        .collect::<Vec<_>>();
    let row = transcript::parse("codex", "session", now_ms(), &records);
    assert_eq!(row.turns.len(), 1);
    let turn = &row.turns[0];
    assert_eq!(
        turn.completed_at.unwrap() - turn.started_at.unwrap(),
        1_319_000
    );
    assert_eq!(row.entries[1].phase, Some("commentary"));
    assert_eq!(row.entries[2].phase, Some("final"));
    assert_eq!(row.entries[0].turn_id.as_ref(), Some(&turn.id));
    assert!(row.entries[3].turn_id.is_none());
    assert!(row.entries[1].timestamp.is_some());
    assert!(!serde_json::to_string(&row).unwrap().contains("source-turn"));
}

#[test]
fn truncated_or_invalid_turn_times_do_not_invent_elapsed_time() {
    let values = [
        json!({"type":"turn_context","payload":{"turn_id":"tail-turn"}}),
        codex_message("assistant", "Only the tail is available"),
        json!({"type":"event_msg","timestamp":"not a date","payload":{"type":"task_complete","turn_id":"tail-turn"}}),
    ];
    let records = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as u64, v))
        .collect::<Vec<_>>();
    let row = transcript::parse("codex", "session", now_ms(), &records);
    assert_eq!(row.turns.len(), 1);
    assert!(row.turns[0].started_at.is_none());
    assert!(row.turns[0].completed_at.is_none());
    let json = serde_json::to_value(row).unwrap();
    assert!(json["entries"][0].get("timestamp").is_none());
}

#[test]
fn rewritten_claude_message_keeps_latest_timestamp_and_phase() {
    let values = [
        json!({"type":"assistant","uuid":"same","timestamp":"2026-10-12T01:00:01Z","message":{"content":[{"type":"text","text":"Checking"}],"stop_reason":"tool_use"}}),
        json!({"type":"assistant","uuid":"same","timestamp":"2026-10-12T01:00:02Z","message":{"content":[{"type":"text","text":"Done"}],"stop_reason":"end_turn"}}),
    ];
    let records = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as u64, v))
        .collect::<Vec<_>>();
    let row = transcript::parse("claude", "session", now_ms(), &records);
    assert_eq!(row.entries.len(), 1);
    assert_eq!(row.entries[0].text, "Done");
    assert_eq!(row.entries[0].phase, Some("final"));
    let expected = OffsetDateTime::parse(
        "2026-10-12T01:00:02Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    assert_eq!(
        row.entries[0].timestamp,
        Some((expected.unix_timestamp_nanos() / 1_000_000) as i64)
    );
}

#[test]
fn codex_reads_visible_messages_tools_and_lifecycle_without_private_payloads() {
    let records = vec![
        json!({"type":"session_meta","payload":{"cwd":"D:\\work\\demo","base_instructions":"DO_NOT_EXPOSE"}}),
        json!({"type":"event_msg","payload":{"type":"task_started"}}),
        codex_message("developer", "DO_NOT_EXPOSE"),
        codex_message("user", "Fix the sidebar"),
        json!({"type":"response_item","payload":{"type":"reasoning","summary":"DO_NOT_EXPOSE","encrypted_content":"DO_NOT_EXPOSE"}}),
        json!({"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"cargo test"}}),
        json!({"type":"response_item","payload":{"type":"function_call_output","output":"PASS\n3 checks"}}),
        codex_message("assistant", "Done"),
        json!({"type":"event_msg","payload":{"type":"task_complete"}}),
    ];
    let records = records
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as u64, v))
        .collect::<Vec<_>>();
    let row = transcript::parse("codex", "session", now_ms(), &records);
    assert_eq!(row.summary.title, "Fix the sidebar");
    assert_eq!(row.summary.project, "demo");
    assert_eq!(row.summary.state, "completed");
    assert_eq!(row.entries.len(), 4);
    assert!(row
        .entries
        .iter()
        .any(|e| e.kind == "result" && e.text.contains('\n')));
    assert!(!serde_json::to_string(&row)
        .unwrap()
        .contains("DO_NOT_EXPOSE"));
}

#[test]
fn claude_keeps_tool_results_separate_from_prompts_and_skips_thinking() {
    let values = [
        json!({"type":"user","uuid":"a","cwd":"/work/example","message":{"content":"Build a search view"}}),
        json!({"type":"assistant","uuid":"b","message":{"content":[{"type":"thinking","thinking":"PRIVATE"},{"type":"tool_use","name":"Read","input":{"file_path":"view.vue"}}],"stop_reason":"tool_use"}}),
        json!({"type":"user","uuid":"c","message":{"content":[{"type":"tool_result","content":[{"type":"text","text":"file content"}]}]}}),
        json!({"type":"assistant","uuid":"d","message":{"content":[{"type":"text","text":"Finished"}],"stop_reason":"end_turn"}}),
    ];
    let records = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i as u64, v))
        .collect::<Vec<_>>();
    let row = transcript::parse("claude", "session", now_ms(), &records);
    assert_eq!(row.summary.state, "completed");
    assert_eq!(row.summary.title, "Build a search view");
    assert_eq!(row.entries.last().unwrap().phase, Some("final"));
    assert_eq!(
        row.entries.iter().map(|e| e.kind).collect::<Vec<_>>(),
        vec!["user", "tool", "result", "assistant"]
    );
    assert!(!serde_json::to_string(&row).unwrap().contains("PRIVATE"));
}

#[test]
fn appended_and_partially_written_records_are_refreshed_without_duplicates() {
    let fixture = Fixture::new();
    let path = fixture.file("2026/session.jsonl", &[codex_message("user", "Task")]);
    let mut cache = HashMap::new();
    let (_, before) = files::read_provider("codex", Some(&fixture.0), &mut cache);
    assert_eq!(before[0].entries.len(), 1);
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    write!(file, "{}", codex_message("assistant", "New output")).unwrap();
    let (_, partial) = files::read_provider("codex", Some(&fixture.0), &mut cache);
    assert_eq!(partial[0].entries.len(), 1);
    writeln!(file).unwrap();
    drop(file);
    let (_, after) = files::read_provider("codex", Some(&fixture.0), &mut cache);
    assert_eq!(after[0].entries.len(), 2);
    assert_eq!(after[0].entries[0].id, before[0].entries[0].id);
    assert_eq!(after[0].entries[1].text, "New output");
    fs::remove_file(path).unwrap();
    let (_, absent) = files::read_provider("codex", Some(&fixture.0), &mut cache);
    assert!(absent.is_empty());
}

#[test]
fn large_transcripts_use_bounded_tail_and_ignore_malformed_rows() {
    let fixture = Fixture::new();
    let path = fixture.file("session.jsonl", &[codex_message("user", "Task")]);
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(file, "{}", "x".repeat(900_000)).unwrap();
    writeln!(file, "{}", codex_message("assistant", &"尾".repeat(20_000))).unwrap();
    drop(file);
    let (_, rows) = files::read_provider("codex", Some(&fixture.0), &mut HashMap::new());
    assert!(rows[0].summary.truncated);
    assert_eq!(
        rows[0].entries.last().unwrap().text.chars().count(),
        TEXT_LIMIT
    );
}

#[test]
fn file_discovery_does_not_follow_external_symlinks() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.file("secret.jsonl", &[codex_message("user", "OUTSIDE")]);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(&outside.0, fixture.0.join("linked"));
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_dir(&outside.0, fixture.0.join("linked"));
    if result.is_err() {
        return;
    }
    let (_, rows) = files::read_provider("codex", Some(&fixture.0), &mut HashMap::new());
    assert!(rows.is_empty());
}

fn opencode_fixture(path: &Path) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("CREATE TABLE session(id TEXT,title TEXT,directory TEXT,time_updated INTEGER,time_archived INTEGER); CREATE TABLE message(id TEXT,session_id TEXT,time_created INTEGER,data TEXT); CREATE TABLE part(id TEXT,message_id TEXT,session_id TEXT,time_created INTEGER,time_updated INTEGER,data TEXT);").unwrap();
    db.execute(
        "INSERT INTO session VALUES ('session','OpenCode task','/work/example',?1,NULL)",
        [now_ms()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO message VALUES ('m','session',1,?1)",
        [json!({"role":"assistant","finish":"stop"}).to_string()],
    )
    .unwrap();
    for (index,value) in [json!({"type":"text","text":"Visible answer"}),json!({"type":"reasoning","text":"PRIVATE"}),json!({"type":"tool","tool":"shell","state":{"input":{"command":"test"},"output":"Passed"}})].into_iter().enumerate() {
        db.execute("INSERT INTO part VALUES (?1,'m','session',?2,?3,?4)",rusqlite::params![index.to_string(),index as i64,now_ms(),value.to_string()]).unwrap();
    }
}

#[test]
fn opencode_reads_sqlite_without_modifying_source() {
    let fixture = Fixture::new();
    let path = fixture.0.join("opencode.db");
    opencode_fixture(&path);
    let before = fs::read(&path).unwrap();
    let (status, rows) = opencode::read_provider(Some(&path));
    assert_eq!(status, "available");
    assert_eq!(rows[0].summary.state, "completed");
    assert_eq!(rows[0].entries.len(), 3);
    assert_eq!(rows[0].entries[0].timestamp, Some(0));
    assert_eq!(rows[0].entries[0].phase, Some("final"));
    assert!(!serde_json::to_string(&rows).unwrap().contains("PRIVATE"));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn unavailable_database_and_missing_roots_are_distinct() {
    let fixture = Fixture::new();
    let path = fixture.0.join("opencode.db");
    fs::write(&path, "not sqlite").unwrap();
    assert_eq!(opencode::read_provider(Some(&path)).0, "unavailable");
    assert_eq!(
        opencode::read_provider(Some(&fixture.0.join("absent.db"))).0,
        "not_found"
    );
}

#[tokio::test]
async fn routes_require_a_session_reject_paths_and_return_no_store_content() {
    let fixture = Fixture::new();
    fixture.file("session.jsonl", &[codex_message("user", "Read this task")]);
    let context = ServerContext::new(None, crate::ServerControl::new()).with_agent_activity_roots(
        AgentActivityRoots {
            codex: Some(fixture.0.clone()),
            ..Default::default()
        },
    );
    let issued = context.sessions().issue().unwrap();
    let cookie = format!("{SESSION_COOKIE_NAME}={}", issued.session_cookie);
    let router = crate::app_router(context);
    let unauthorized = router
        .clone()
        .oneshot(
            Request::get("/api/v1/agents/activity")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = router
        .clone()
        .oneshot(
            Request::get("/api/v1/agents/activity?provider=codex")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    let id = value["sessions"][0]["id"].as_str().unwrap();
    assert_eq!(id.len(), 64);
    assert!(value["sessions"][0].get("entries").is_none());
    let response = router
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/agents/activity/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for (url, status) in [
        (
            "/api/v1/agents/activity?provider=arbitrary",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("/api/v1/agents/activity/not-a-file", StatusCode::NOT_FOUND),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::get(url)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
}

#[tokio::test]
async fn managed_runs_expose_current_content_without_reasoning() {
    let context = ServerContext::new(None, crate::ServerControl::new());
    let run = context.runs().create().unwrap();
    run.append(orchester_protokoll::UiEventKind::UserMessage {
        text: "Managed task".into(),
    })
    .await
    .unwrap();
    run.append(orchester_protokoll::UiEventKind::MessageDelta {
        text: "Hello ".into(),
        final_chunk: false,
    })
    .await
    .unwrap();
    run.append(orchester_protokoll::UiEventKind::MessageDelta {
        text: "world".into(),
        final_chunk: true,
    })
    .await
    .unwrap();
    run.append(orchester_protokoll::UiEventKind::Reasoning {
        text: "PRIVATE".into(),
    })
    .await
    .unwrap();
    let rows = managed::snapshot(&context).await;
    assert_eq!(rows[0].entries.len(), 2);
    assert_eq!(rows[0].entries[1].text, "Hello world");
    assert_eq!(rows[0].summary.source, "managed");
}
