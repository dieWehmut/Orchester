//! Wire-contract tests for the self-harness protocol.
//!
//! The harness stream is persisted and consumed by the CLI and later by the
//! WebUI, so its JSON shape must be stable and self-inverse.

use orchester_protokoll::{
    ActionId, AgentAction, ApprovalId, ApprovalRequest, EventId, HarnessEvent, HarnessEventKind,
    RunId, StepId, StopReason,
};

#[test]
fn action_recorded_uses_the_exact_top_level_fixture() {
    let event = HarnessEvent::new_for_test(
        EventId::from("evt-1"),
        RunId::from("run-1"),
        StepId::from("step-1"),
        1,
        HarnessEventKind::ActionRecorded {
            action_id: ActionId::from("act-1"),
            action_summary: "read_file path_bytes=10 start_line=None end_line=None".into(),
            action_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            origin_model_call_id: Some("model-call-1".into()),
        },
    );
    let expected = r#"{"schema_version":2,"event_id":"evt-1","run_id":"run-1","turn_id":null,"step_id":"step-1","call_id":null,"sequence":1,"occurred_at":"2026-07-10T00:00:00Z","kind":"action.recorded","payload":{"action_id":"act-1","action_summary":"read_file path_bytes=10 start_line=None end_line=None","action_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","origin_model_call_id":"model-call-1"}}"#;
    assert_eq!(serde_json::to_string(&event).unwrap(), expected);
    assert_eq!(
        serde_json::from_str::<HarnessEvent>(expected).unwrap(),
        event
    );
}

#[test]
fn action_recorded_legacy_fixture_without_origin_still_decodes() {
    let fixture = r#"{"kind":"action.recorded","payload":{"action_id":"act-legacy","action":{"tool":"read_file","path":"src/lib.rs","start_line":null,"end_line":null}}}"#;
    let kind: HarnessEventKind = serde_json::from_str(fixture).unwrap();
    assert!(matches!(
        &kind,
        HarnessEventKind::ActionRecorded {
            origin_model_call_id: None,
            ..
        }
    ));
    let encoded = serde_json::to_string(&kind).unwrap();
    assert!(!encoded.contains("src/lib.rs"));
    assert!(!encoded.contains("\"action\""));
    assert!(!format!("{kind:?}").contains("src/lib.rs"));
}

#[test]
fn action_recorded_rejects_noncanonical_summary_and_hash() {
    let control_summary = r#"{"kind":"action.recorded","payload":{"action_id":"act-1","action_summary":"read_file\u0000path_bytes=1","action_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","origin_model_call_id":"model-call-1"}}"#;
    let uppercase_hash = r#"{"kind":"action.recorded","payload":{"action_id":"act-1","action_summary":"read_file path_bytes=1","action_hash":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","origin_model_call_id":"model-call-1"}}"#;
    assert!(serde_json::from_str::<HarnessEventKind>(control_summary).is_err());
    assert!(serde_json::from_str::<HarnessEventKind>(uppercase_hash).is_err());
}

#[test]
fn tool_completed_fixture_has_dotted_kind_and_flattened_payload() {
    let fixture = r#"{"schema_version":1,"event_id":"evt-2","run_id":"run-1","turn_id":"turn-1","step_id":"step-1","call_id":"call-1","sequence":2,"occurred_at":"2026-07-10T00:00:01Z","kind":"tool.completed","payload":{"observation":{"observation_id":"obs-1","call_id":"call-1","kind":"read_file","summary":"ok","data":{"bytes":2}}}}"#;
    let event: HarnessEvent = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_string(&event).unwrap(), fixture);
}

#[test]
fn event_kind_alone_uses_the_same_dotted_contract_without_double_nesting() {
    let kind = HarnessEventKind::RunCreated;
    let fixture = r#"{"kind":"run.created","payload":{}}"#;
    assert_eq!(serde_json::to_string(&kind).unwrap(), fixture);
    assert_eq!(
        serde_json::from_str::<HarnessEventKind>(fixture).unwrap(),
        kind
    );
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
        action_summary: "run cargo test".into(),
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

#[test]
fn approval_summary_is_normalized_and_redacted_before_it_is_serialized() {
    let request = ApprovalRequest {
        approval_id: ApprovalId::from("approval-1"),
        run_id: RunId::from("run-1"),
        action_id: ActionId::from("action-1"),
        action_summary: "  run   OPENAI_API_KEY=sk-super-secret  ".into(),
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
    assert!(!json.contains("sk-super-secret"));
    assert!(json.contains("OPENAI_API_KEY=[REDACTED]"));
    let decoded: ApprovalRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.action_summary, "run OPENAI_API_KEY=[REDACTED]");
}

#[test]
fn generated_action_summary_does_not_copy_model_controlled_strings() {
    let action = AgentAction::RunCommand {
        program: "echo sk-model-controlled-secret".into(),
        args: vec!["--header".into(), "Authorization: Bearer hidden".into()],
        cwd: Some("workspace\u{1b}[31m".into()),
    };
    let summary = action.action_summary();
    assert!(!summary.contains("sk-model-controlled-secret"));
    assert!(!summary.contains("Authorization"));
    assert!(!summary.contains("workspace"));
    assert!(summary.contains("program_bytes="));
    assert!(summary.contains("args_count=2"));
}

#[test]
fn invalid_schema_and_sequence_are_rejected_at_the_wire_boundary() {
    let schema_fixture = r#"{"schema_version":3,"event_id":"evt-1","run_id":"run-1","turn_id":null,"step_id":null,"call_id":null,"sequence":1,"occurred_at":"2026-07-10T00:00:00Z","kind":"run.created","payload":{}}"#;
    let sequence_fixture = r#"{"schema_version":1,"event_id":"evt-1","run_id":"run-1","turn_id":null,"step_id":null,"call_id":null,"sequence":0,"occurred_at":"2026-07-10T00:00:00Z","kind":"run.created","payload":{}}"#;
    assert!(serde_json::from_str::<HarnessEvent>(schema_fixture).is_err());
    assert!(serde_json::from_str::<HarnessEvent>(sequence_fixture).is_err());
}

#[test]
fn empty_approval_summary_is_rejected() {
    let fixture = r#"{"approval_id":"approval-1","run_id":"run-1","action_id":"action-1","action_summary":"   ","action_hash":"hash","workspace_identity":"workspace","policy_snapshot_hash":"policy","config_snapshot_hash":"config","risk":"high","rule_id":"command.network","created_at":"2026-07-10T00:00:00Z","expires_at":"2026-07-11T00:00:00Z"}"#;
    assert!(serde_json::from_str::<ApprovalRequest>(fixture).is_err());
}

#[test]
fn approval_requested_must_bind_to_the_outer_run() {
    let fixture = r#"{"schema_version":1,"event_id":"evt-1","run_id":"run-outer","turn_id":null,"step_id":null,"call_id":null,"sequence":1,"occurred_at":"2026-07-10T00:00:00Z","kind":"approval.requested","payload":{"request":{"approval_id":"approval-1","run_id":"run-inner","action_id":"action-1","action_summary":"run cargo test","action_hash":"hash","workspace_identity":"workspace","policy_snapshot_hash":"policy","config_snapshot_hash":"config","risk":"high","rule_id":"command.network","created_at":"2026-07-10T00:00:00Z","expires_at":"2026-07-11T00:00:00Z"}}}"#;
    assert!(serde_json::from_str::<HarnessEvent>(fixture).is_err());
}
