//! Wire-contract tests for the self-harness protocol.
//!
//! The harness stream is persisted and consumed by the CLI and later by the
//! WebUI, so its JSON shape must be stable and self-inverse.

use orchester_protokoll::{
    ActionId, AgentAction, ApprovalId, ApprovalRequest, EventId, HarnessEvent, HarnessEventKind,
    RunId, StepId, StopReason,
};

#[test]
fn action_recorded_round_trips_with_stable_envelope() {
    let event = HarnessEvent::new_for_test(
        EventId::from("evt-1"),
        RunId::from("run-1"),
        StepId::from("step-1"),
        1,
        HarnessEventKind::ActionRecorded {
            action_id: ActionId::from("act-1"),
            action: AgentAction::ReadFile {
                path: "src/lib.rs".into(),
                start_line: None,
                end_line: None,
            },
        },
    );
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"schema_version\":1"));
    assert!(json.contains("\"kind\":\"action_recorded\""));
    assert_eq!(serde_json::from_str::<HarnessEvent>(&json).unwrap(), event);
}

#[test]
fn stop_reasons_are_not_stringly_typed() {
    let json = serde_json::to_string(&StopReason::AwaitingApproval).unwrap();
    assert_eq!(json, "\"awaiting_approval\"");
}

#[test]
fn typed_ids_are_transparent_and_distinct_in_the_api() {
    let event_id = EventId::from("evt-1");
    let run_id = RunId::from("run-1");
    let json = serde_json::to_string(&(event_id.clone(), run_id.clone())).unwrap();
    assert_eq!(json, "[\"evt-1\",\"run-1\"]");
    let (event_back, run_back): (EventId, RunId) = serde_json::from_str(&json).unwrap();
    assert_eq!(event_back, event_id);
    assert_eq!(run_back, run_id);
}

#[test]
fn action_rejects_unknown_fields_before_execution() {
    let json = r#"{
        "tool": "read_file",
        "path": "src/lib.rs",
        "start_line": null,
        "end_line": null,
        "unexpected": true
    }"#;
    assert!(serde_json::from_str::<AgentAction>(json).is_err());
}

#[test]
fn approval_request_round_trips_with_run_binding() {
    let request = ApprovalRequest {
        approval_id: ApprovalId::from("approval-1"),
        run_id: RunId::from("run-1"),
        action_id: ActionId::from("action-1"),
        action_hash: "action-hash".into(),
        workspace_identity: "workspace".into(),
        policy_snapshot_hash: "policy-hash".into(),
        config_snapshot_hash: "config-hash".into(),
        risk: "high".into(),
        rule_id: "command.network".into(),
        created_at: "2026-07-10T00:00:00Z".into(),
        expires_at: "2026-07-11T00:00:00Z".into(),
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"run_id\":\"run-1\""));
    assert_eq!(
        serde_json::from_str::<ApprovalRequest>(&json).unwrap(),
        request
    );
}
