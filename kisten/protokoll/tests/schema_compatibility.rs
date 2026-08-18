use orchester_protokoll::{
    EventId, HarnessEvent, HarnessEventKind, RunId, StepId, UiEventEnvelope,
    HARNESS_SCHEMA_VERSION, LEGACY_EVENT_SCHEMA_VERSION, LEGACY_HARNESS_SCHEMA_VERSION,
    UI_SCHEMA_VERSION,
};

const HARNESS_V1_RUN_CREATED: &str = r#"{"schema_version":1,"event_id":"event-h1","run_id":"run-1","turn_id":null,"step_id":null,"call_id":null,"sequence":1,"occurred_at":"2026-08-19T00:00:00Z","kind":"run.created","payload":{}}"#;
const UI_V1_TURN_STARTED: &str = r#"{"schema_version":1,"event_id":"event-ui1","run_id":"run-1","sequence":1,"occurred_at":"2026-08-19T00:00:00Z","kind":{"type":"turn_started"}}"#;

#[test]
fn durable_harness_keeps_legacy_v1_readable() {
    let event: HarnessEvent = serde_json::from_str(HARNESS_V1_RUN_CREATED).unwrap();

    assert_eq!(event.schema_version, LEGACY_HARNESS_SCHEMA_VERSION);
    assert_eq!(event.kind, HarnessEventKind::RunCreated);
}

#[test]
fn current_harness_writers_emit_v2() {
    let event = HarnessEvent::new_for_test(
        EventId::from("event-h2"),
        RunId::from("run-1"),
        StepId::from("step-1"),
        1,
        HarnessEventKind::RunCreated,
    );

    assert_eq!(event.schema_version, HARNESS_SCHEMA_VERSION);
    assert_eq!(
        serde_json::to_value(event).unwrap()["schema_version"],
        HARNESS_SCHEMA_VERSION
    );
}

#[test]
fn browser_ui_accepts_only_its_v1_envelope() {
    let event: UiEventEnvelope = serde_json::from_str(UI_V1_TURN_STARTED).unwrap();
    assert_eq!(event.schema_version, UI_SCHEMA_VERSION);

    for unsupported in [LEGACY_EVENT_SCHEMA_VERSION, UI_SCHEMA_VERSION + 1] {
        let fixture = UI_V1_TURN_STARTED.replace(
            "\"schema_version\":1",
            &format!("\"schema_version\":{unsupported}"),
        );
        assert!(serde_json::from_str::<UiEventEnvelope>(&fixture).is_err());
    }
}
