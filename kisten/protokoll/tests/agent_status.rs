use orchester_protokoll::{
    AgentActivityState, AgentAvailabilityState, AgentFleetSnapshotDto, AgentFleetStreamFrameDto,
    AgentRuntimeSummaryDto, AgentWindowCountSource, AGENT_STATUS_SCHEMA_VERSION,
};

fn agent(id: &str) -> AgentRuntimeSummaryDto {
    AgentRuntimeSummaryDto {
        agent_id: id.into(),
        provider: "codex".into(),
        display_name: "Codex".into(),
        icon_key: "codex".into(),
        availability: AgentAvailabilityState::Available,
        activity: AgentActivityState::Running,
        installed: true,
        configured: true,
        authenticated: true,
        active_windows: 2,
        active_sessions: 3,
        active_runs: 2,
        active_subagents: 1,
        window_count_source: AgentWindowCountSource::ManagedSessions,
        last_heartbeat_at: Some("2026-08-20T08:00:00Z".into()),
        last_error: None,
        capabilities: vec!["streaming".into(), "resume".into()],
        updated_at: "2026-08-20T08:00:01Z".into(),
    }
}

#[test]
fn fleet_stream_frames_have_strict_snapshot_and_heartbeat_shapes() {
    let snapshot = AgentFleetSnapshotDto {
        schema_version: AGENT_STATUS_SCHEMA_VERSION,
        sequence: 7,
        generated_at: "2026-08-20T08:00:02Z".into(),
        agents: vec![agent("codex-main")],
    };
    let frame = AgentFleetStreamFrameDto::Snapshot {
        snapshot: snapshot.clone(),
    };
    let value = serde_json::to_value(frame).expect("serialize snapshot frame");
    assert_eq!(value["type"], "snapshot");
    assert_eq!(value["snapshot"]["sequence"], 7);

    let heartbeat = serde_json::json!({
        "type": "heartbeat",
        "sequence": 7,
        "sent_at": "2026-08-20T08:00:03Z"
    });
    let decoded: AgentFleetStreamFrameDto =
        serde_json::from_value(heartbeat).expect("decode heartbeat");
    assert_eq!(
        decoded,
        AgentFleetStreamFrameDto::Heartbeat {
            sequence: 7,
            sent_at: "2026-08-20T08:00:03Z".into(),
        }
    );

    assert!(
        serde_json::from_value::<AgentFleetStreamFrameDto>(serde_json::json!({
            "type": "heartbeat",
            "sequence": 7,
            "sent_at": "2026-08-20T08:00:03Z",
            "provider_payload": true
        }))
        .is_err()
    );
}

#[test]
fn fleet_status_roundtrips_with_the_browser_field_names() {
    let snapshot = AgentFleetSnapshotDto {
        schema_version: AGENT_STATUS_SCHEMA_VERSION,
        sequence: 7,
        generated_at: "2026-08-20T08:00:02Z".into(),
        agents: vec![agent("codex-main")],
    };

    let value = serde_json::to_value(&snapshot).expect("serialize fleet snapshot");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["agents"][0]["agent_id"], "codex-main");
    assert_eq!(value["agents"][0]["activity"], "running");

    let decoded: AgentFleetSnapshotDto = serde_json::from_value(value).expect("decode snapshot");
    assert_eq!(decoded, snapshot);
}

#[test]
fn fleet_status_rejects_unknown_fields_duplicate_ids_and_invalid_sequence() {
    let snapshot = AgentFleetSnapshotDto {
        schema_version: AGENT_STATUS_SCHEMA_VERSION,
        sequence: 1,
        generated_at: "2026-08-20T08:00:02Z".into(),
        agents: vec![agent("codex-main")],
    };
    let mut value = serde_json::to_value(&snapshot).unwrap();
    value["unexpected"] = true.into();
    assert!(serde_json::from_value::<AgentFleetSnapshotDto>(value).is_err());

    let duplicate = AgentFleetSnapshotDto {
        agents: vec![agent("same"), agent("same")],
        ..snapshot.clone()
    };
    assert!(serde_json::to_value(&duplicate).is_err());
    assert!(
        serde_json::from_value::<AgentFleetSnapshotDto>(serde_json::json!({
            "schema_version": 1,
            "sequence": 1,
            "generated_at": "2026-08-20T08:00:02Z",
            "agents": [agent("same"), agent("same")]
        }))
        .is_err()
    );

    let mut invalid = serde_json::to_value(snapshot).unwrap();
    invalid["sequence"] = 0.into();
    assert!(serde_json::from_value::<AgentFleetSnapshotDto>(invalid).is_err());

    let mut invalid_agent = agent("invalid-icon");
    invalid_agent.icon_key = "../secret".into();
    assert!(serde_json::to_value(invalid_agent).is_err());
}

#[test]
fn fleet_status_redacts_absolute_error_paths_before_serializing() {
    let mut value = agent("codex-main");
    value.last_error = Some("failed path=C:\\Users\\dev\\transcript.json".into());
    let json = serde_json::to_string(&value).expect("serialize redacted agent");

    assert!(!json.contains("C:\\\\Users"));
    assert!(json.contains("[ROOT]"));
}
