use orchester_protokoll::{EventId, RunId, UiEventEnvelope, UiEventKind, UI_SCHEMA_VERSION};

#[test]
fn ui_protocol_is_exported_from_the_crate_root() {
    let event = UiEventEnvelope {
        schema_version: UI_SCHEMA_VERSION,
        event_id: EventId::from("event-public-api"),
        run_id: RunId::from("run-public-api"),
        turn_id: None,
        call_id: None,
        sequence: 1,
        occurred_at: "2026-08-19T00:00:00Z".into(),
        kind: UiEventKind::TurnStarted,
    };

    assert_eq!(event.schema_version, UI_SCHEMA_VERSION);
    assert_eq!(event.sequence, 1);
}
