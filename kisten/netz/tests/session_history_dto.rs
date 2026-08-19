use orchester_anwendung::{SessionHistoryDetail, SessionHistoryPage, SessionHistorySummary};
use orchester_netz::{
    session_detail_response, session_page_response, SessionOutcomeDto,
    SESSION_HISTORY_SCHEMA_VERSION, SESSION_PROMPT_MAX_CHARS, SESSION_RESULT_MAX_CHARS,
};
use orchester_protokoll::{Outcome, Usage};

fn summary() -> SessionHistorySummary {
    SessionHistorySummary {
        id: "s-0123456789abcdef0123456789abcdef".to_owned(),
        recorded_at_unix: 1_800_000_000,
        title: "inspect the workspace".to_owned(),
        agent: "codex".to_owned(),
        model: Some("gpt-5.6".to_owned()),
        outcome: Outcome::Success,
        resumable: false,
    }
}

#[test]
fn session_page_projection_matches_the_versioned_browser_contract() {
    let dto = session_page_response(&SessionHistoryPage {
        items: vec![summary()],
        next_cursor: Some("s-fedcba9876543210fedcba9876543210".to_owned()),
    });

    assert_eq!(dto.schema_version, SESSION_HISTORY_SCHEMA_VERSION);
    assert_eq!(dto.items.len(), 1);
    assert_eq!(dto.items[0].source, "delegate");
    assert_eq!(dto.items[0].outcome, SessionOutcomeDto::Success);
    assert!(!dto.items[0].resumable);

    let value = serde_json::to_value(dto).expect("serialize page");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["items"][0]["outcome"], "success");
    assert_eq!(value["next_cursor"], "s-fedcba9876543210fedcba9876543210");
}

#[test]
fn session_detail_projection_is_flat_path_free_and_bounded() {
    let prompt = format!(
        "C:\\private\\workspace\n{}",
        "p".repeat(SESSION_PROMPT_MAX_CHARS)
    );
    let final_text = "r".repeat(SESSION_RESULT_MAX_CHARS + 10);
    let dto = session_detail_response(&SessionHistoryDetail {
        summary: summary(),
        prompt,
        final_text,
        usage: Usage {
            input_tokens: 10,
            output_tokens: 20,
            cached_input_tokens: 3,
            reasoning_output_tokens: 4,
        },
    });

    assert_eq!(dto.prompt.chars().count(), SESSION_PROMPT_MAX_CHARS);
    assert_eq!(dto.final_text.chars().count(), SESSION_RESULT_MAX_CHARS);
    assert!(dto.prompt.ends_with("..."));
    assert!(dto.final_text.ends_with("..."));
    assert!(!format!("{dto:?}").contains("private"));

    let value = serde_json::to_value(dto).expect("serialize detail");
    assert!(value.get("summary").is_none());
    assert!(value.get("cwd").is_none());
    assert!(value.get("session_id").is_none());
    assert_eq!(value["usage"]["output_tokens"], 20);
}
