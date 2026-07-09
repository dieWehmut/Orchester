//! Serde round-trip tests: the protocol *is* the wire format, so its JSONL shape
//! must be stable and self-inverse (`T -> json -> T`).

use orchester_protokoll::{
    ChangeKind, Event, Outcome, RunResult, SessionState, Task, TaskKind, TodoItem, ToolStatus,
    Usage,
};

fn roundtrip_event(e: &Event) -> Event {
    let json = serde_json::to_string(e).expect("serialize");
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn event_tag_is_snake_case_type_field() {
    let json = serde_json::to_value(Event::TurnStarted).unwrap();
    assert_eq!(json["type"], "turn_started");

    let json = serde_json::to_value(Event::SessionStarted {
        session_id: "abc".into(),
    })
    .unwrap();
    assert_eq!(json["type"], "session_started");
    assert_eq!(json["session_id"], "abc");
}

#[test]
fn all_event_variants_roundtrip() {
    let events = vec![
        Event::SessionStarted {
            session_id: "sess-1".into(),
        },
        Event::TurnStarted,
        Event::Message {
            text: "hello".into(),
        },
        Event::Reasoning {
            text: "thinking".into(),
        },
        Event::ToolCall {
            name: "bash".into(),
            status: ToolStatus::InProgress,
            detail: Some("ls -la".into()),
        },
        Event::ToolCall {
            name: "bash".into(),
            status: ToolStatus::Completed,
            detail: None,
        },
        Event::FileChange {
            path: "src/main.rs".into(),
            kind: ChangeKind::Update,
        },
        Event::TodoList {
            items: vec![
                TodoItem {
                    text: "step 1".into(),
                    completed: true,
                },
                TodoItem {
                    text: "step 2".into(),
                    completed: false,
                },
            ],
        },
        Event::Usage(Usage {
            input_tokens: 10,
            output_tokens: 20,
            cached_input_tokens: 5,
            reasoning_output_tokens: 3,
        }),
        Event::TurnCompleted,
        Event::Result {
            text: "done".into(),
        },
        Event::Error {
            message: "boom".into(),
        },
    ];
    for e in &events {
        assert_eq!(&roundtrip_event(e), e, "roundtrip failed for {e:?}");
    }
}

#[test]
fn usage_event_flattens_fields_alongside_tag() {
    // Newtype variant carrying a struct flattens fields next to `type`.
    let json = serde_json::to_value(Event::Usage(Usage {
        input_tokens: 100,
        output_tokens: 200,
        cached_input_tokens: 0,
        reasoning_output_tokens: 0,
    }))
    .unwrap();
    assert_eq!(json["type"], "usage");
    assert_eq!(json["input_tokens"], 100);
    assert_eq!(json["output_tokens"], 200);
}

#[test]
fn task_roundtrip_and_builders() {
    let task = Task::new("fix the bug", "/tmp/proj")
        .with_resume("sess-9")
        .with_model("claude-opus-4-6");
    let json = serde_json::to_string(&task).unwrap();
    let back: Task = serde_json::from_str(&json).unwrap();
    assert_eq!(task, back);
    assert_eq!(back.resume.as_deref(), Some("sess-9"));
    assert_eq!(back.model.as_deref(), Some("claude-opus-4-6"));
}

#[test]
fn task_omits_none_fields() {
    let task = Task::new("hi", "/tmp");
    let json = serde_json::to_value(&task).unwrap();
    assert!(json.get("resume").is_none());
    assert!(json.get("model").is_none());
}

#[test]
fn run_result_roundtrip() {
    let r = RunResult {
        session_id: Some("s".into()),
        final_text: "final".into(),
        usage: Usage::default(),
        outcome: Outcome::Success,
    };
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(serde_json::from_str::<RunResult>(&json).unwrap(), r);
}

#[test]
fn usage_accumulates() {
    let mut u = Usage::default();
    u.add(&Usage {
        input_tokens: 1,
        output_tokens: 2,
        cached_input_tokens: 3,
        reasoning_output_tokens: 4,
    });
    u.add(&Usage {
        input_tokens: 10,
        output_tokens: 20,
        cached_input_tokens: 30,
        reasoning_output_tokens: 40,
    });
    assert_eq!(u.input_tokens, 11);
    assert_eq!(u.output_tokens, 22);
    assert_eq!(u.cached_input_tokens, 33);
    assert_eq!(u.reasoning_output_tokens, 44);
}

#[test]
fn capability_and_session_state_roundtrip() {
    let cap = Capability_sample();
    let json = serde_json::to_string(&cap).unwrap();
    assert_eq!(serde_json::from_str::<orchester_protokoll::Capability>(&json).unwrap(), cap);

    for s in [
        SessionState::Starting,
        SessionState::Running,
        SessionState::Completed,
        SessionState::Failed,
        SessionState::Cancelled,
    ] {
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<SessionState>(&j).unwrap(), s);
    }
}

#[allow(non_snake_case)]
fn Capability_sample() -> orchester_protokoll::Capability {
    orchester_protokoll::Capability {
        name: "mock".into(),
        kinds: vec![TaskKind::Code, TaskKind::Chat, TaskKind::Custom("x".into())],
        supports_resume: true,
        streaming: true,
    }
}
