use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalError, ApprovalState, ApprovalStore, CapabilityToken,
};
use orchester_laufzeit::harness::audit::{AuditError, AuditInput, JsonlAuditSink};
use sha2::{Digest, Sha256};

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn audit_chain_is_contiguous_fsyncable_and_detects_tampering() {
    let path = temp_path("audit");
    let sink = JsonlAuditSink::open(&path).unwrap();
    sink.append(AuditInput::test(1, "run-1", "action-1", "run cargo test"))
        .unwrap();
    sink.append(AuditInput::test(
        2,
        "run-1",
        "action-2",
        "OPENAI_API_KEY=sk-not-for-audit",
    ))
    .unwrap();
    assert!(sink.verify().unwrap().is_valid());
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("sk-not-for-audit"));
    assert!(text.contains("summary_bytes="));

    let mut tampered = text;
    tampered = tampered.replacen("action-2", "action-tampered", 1);
    fs::write(&path, tampered).unwrap();
    assert!(sink.verify().is_err());
    assert!(sink
        .append(AuditInput::test(3, "run-1", "action-3", "after corruption"))
        .is_err());
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn independent_audit_sinks_share_one_locked_head_and_reject_unknown_fields() {
    let path = temp_path("multi-sink");
    let first = JsonlAuditSink::open(&path).unwrap();
    let second = JsonlAuditSink::open(&path).unwrap();
    assert_eq!(
        first
            .append(AuditInput::test(1, "run-1", "action-1", "one"))
            .unwrap(),
        1
    );
    assert_eq!(
        second
            .append(AuditInput::test(2, "run-1", "action-2", "two"))
            .unwrap(),
        2
    );
    assert_eq!(first.verify().unwrap().entries, 2);

    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replacen('{', "{\"unexpected\":true,", 1)).unwrap();
    assert!(second.verify().is_err());

    let parent = path.parent().unwrap().to_path_buf();
    drop(first);
    drop(second);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_rejects_identifiers_that_would_normalize_or_truncate() {
    let path = temp_path("identifier-integrity");
    let sink = JsonlAuditSink::open(&path).unwrap();
    let mut invalid = AuditInput::test(1, "run-1", "action-1", "invalid id");
    invalid.event_id = "event/1".into();
    assert!(sink.append(invalid).is_err());
    assert_eq!(
        sink.append(AuditInput::test(1, "run-1", "action-1", "valid"))
            .unwrap(),
        1
    );
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_records_action_projection_and_rejects_same_event_drift() {
    let path = temp_path("action-binding");
    let sink = JsonlAuditSink::open(&path).unwrap();
    let input = AuditInput::test(
        1,
        "run-action-binding",
        "action-binding",
        "OPENAI_API_KEY=sk-never-persist-this",
    );
    let expected_summary = "read_file path_bytes=12 start_line=None end_line=None".to_owned();
    let expected_hash = "0123456789abcdef".repeat(4);
    let mut input = input;
    input.action_summary = Some(expected_summary.clone());
    input.action_hash = Some(expected_hash.clone());
    sink.append(input.clone()).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("sk-never-persist-this"));
    let entry: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(entry["schema_version"], 2);
    assert_eq!(entry["action_id"], "action-binding");
    assert_eq!(entry["action_summary"], expected_summary);
    assert_eq!(entry["action_hash"], expected_hash);

    let mut drifted_id = input.clone();
    drifted_id.action_id = Some("different-action".into());
    assert!(matches!(
        sink.append(drifted_id),
        Err(AuditError::EventConflict)
    ));
    let mut drifted_summary = input.clone();
    drifted_summary.action_summary =
        Some("read_file path_bytes=99 start_line=None end_line=None".into());
    assert!(matches!(
        sink.append(drifted_summary),
        Err(AuditError::EventConflict)
    ));
    let mut drifted_hash = input;
    drifted_hash.action_hash = Some("fedcba9876543210".repeat(4));
    assert!(matches!(
        sink.append(drifted_hash),
        Err(AuditError::EventConflict)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_reads_v1_prefix_but_writes_only_v2_bindings() {
    let path = temp_path("v1-prefix");
    drop(JsonlAuditSink::open(&path).unwrap());
    let legacy = r#"{"schema_version":1,"sequence":1,"occurred_at":"2026-07-12T00:00:01Z","event_id":"event-1","actor":"local-user","run_id":"run-legacy","action_id":"action-legacy","approval_id":null,"policy_rule":"test.rule","decision":"allow","result_summary":"summary_bytes=6","prev_hash":"0000000000000000000000000000000000000000000000000000000000000000","entry_hash":"388a0a8e5a8cac77b6a8a6d5b93668765d61e70c5764a60ebc50f6a124cc736e"}"#;
    fs::write(&path, format!("{legacy}\n")).unwrap();

    let sink = JsonlAuditSink::open(&path).unwrap();
    assert_eq!(sink.verify().unwrap().entries, 1);
    assert!(matches!(
        sink.append(AuditInput::test(1, "run-legacy", "action-legacy", "legacy",)),
        Err(AuditError::EventConflict)
    ));
    assert_eq!(
        sink.append(AuditInput::test(2, "run-current", "action-current", "new"))
            .unwrap(),
        2
    );
    let entries = fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(entries[0]["schema_version"], 1);
    assert_eq!(entries[1]["schema_version"], 2);
    assert_eq!(
        entries[1]["prev_hash"],
        "388a0a8e5a8cac77b6a8a6d5b93668765d61e70c5764a60ebc50f6a124cc736e"
    );
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_rejects_v1_after_v2_even_when_the_downgrade_record_is_self_consistent() {
    let path = temp_path("v2-downgrade");
    let sink = JsonlAuditSink::open(&path).unwrap();
    sink.append(AuditInput::test(1, "run-current", "action-current", "new"))
        .unwrap();
    drop(sink);

    let current = fs::read_to_string(&path).unwrap();
    let current_entry: serde_json::Value = serde_json::from_str(current.trim()).unwrap();
    let current_head = current_entry["entry_hash"].as_str().unwrap();
    let unsigned_legacy = format!(
        r#"{{"schema_version":1,"sequence":2,"occurred_at":"2026-07-12T00:00:02Z","event_id":"event-legacy-after-v2","actor":"local-user","run_id":"run-legacy","action_id":"action-legacy","approval_id":null,"policy_rule":"test.rule","decision":"allow","result_summary":"summary_bytes=6","prev_hash":"{current_head}","entry_hash":""}}"#
    );
    let legacy_hash = Sha256::digest(unsigned_legacy.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let legacy = unsigned_legacy.replacen(
        r#""entry_hash":"""#,
        &format!(r#""entry_hash":"{legacy_hash}""#),
        1,
    );
    fs::write(&path, format!("{current}{legacy}\n")).unwrap();

    assert!(matches!(
        JsonlAuditSink::open(&path),
        Err(AuditError::Corrupt)
    ));
    let parent = path.parent().unwrap().to_path_buf();
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_rejects_noncanonical_action_projection() {
    let path = temp_path("invalid-action-binding");
    let sink = JsonlAuditSink::open(&path).unwrap();
    let mut invalid_summary = AuditInput::test(1, "run-1", "action-1", "summary");
    invalid_summary.action_summary = Some("run_command\nprogram_bytes=4".into());
    assert!(matches!(
        sink.append(invalid_summary),
        Err(AuditError::InvalidRecord)
    ));
    let mut invalid_hash = AuditInput::test(1, "run-1", "action-1", "summary");
    invalid_hash.action_hash = Some("A".repeat(64));
    assert!(matches!(
        sink.append(invalid_hash),
        Err(AuditError::InvalidRecord)
    ));
    for missing in ["id", "summary", "hash"] {
        let mut incomplete = AuditInput::test(1, "run-1", "action-1", "summary");
        match missing {
            "id" => incomplete.action_id = None,
            "summary" => incomplete.action_summary = None,
            "hash" => incomplete.action_hash = None,
            _ => unreachable!(),
        }
        assert!(matches!(
            sink.append(incomplete),
            Err(AuditError::InvalidRecord)
        ));
    }
    let mut no_action = AuditInput::test(1, "run-1", "action-1", "summary");
    no_action.action_id = None;
    no_action.action_summary = None;
    no_action.action_hash = None;
    assert_eq!(sink.append(no_action).unwrap(), 1);
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_rejects_self_consistent_v2_records_with_invalid_action_projection() {
    let path = temp_path("invalid-wire-action-binding");
    let sink = JsonlAuditSink::open(&path).unwrap();
    sink.append(AuditInput::test(1, "run-1", "action-1", "summary"))
        .unwrap();
    let original = fs::read_to_string(&path).unwrap();
    let entry: serde_json::Value = serde_json::from_str(original.trim()).unwrap();
    let summary = entry["action_summary"].as_str().unwrap();
    let incomplete_projection = resign_entry(&original.trim().replace(
        &format!(r#""action_summary":"{summary}""#),
        r#""action_summary":null"#,
    ));
    fs::write(&path, format!("{incomplete_projection}\n")).unwrap();
    assert!(matches!(sink.verify(), Err(AuditError::Corrupt)));

    let invalid_id = resign_entry(
        &original
            .trim()
            .replace(r#""action_id":"action-1""#, r#""action_id":"action/1""#),
    );
    fs::write(&path, format!("{invalid_id}\n")).unwrap();
    assert!(matches!(sink.verify(), Err(AuditError::Corrupt)));
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[test]
fn audit_rejects_self_consistent_v2_records_with_invalid_event_semantics() {
    let path = temp_path("invalid-wire-semantics");
    let sink = JsonlAuditSink::open(&path).unwrap();
    sink.append(AuditInput::test(1, "run-1", "action-1", "summary"))
        .unwrap();
    let original = fs::read_to_string(&path).unwrap();

    let valid_event_id = resign_entry(
        &original
            .trim()
            .replace(r#""event_id":"event-1""#, r#""event_id":"event-2""#),
    );
    fs::write(&path, format!("{valid_event_id}\n")).unwrap();
    assert!(
        sink.verify().is_ok(),
        "the resigned control record is valid"
    );

    let invalid_event_id = resign_entry(
        &original
            .trim()
            .replace(r#""event_id":"event-1""#, r#""event_id":"event/1""#),
    );
    fs::write(&path, format!("{invalid_event_id}\n")).unwrap();
    assert!(matches!(sink.verify(), Err(AuditError::Corrupt)));

    let raw_result = resign_entry(&original.trim().replace(
        r#""result_summary":"summary_bytes=7""#,
        r#""result_summary":"OPENAI_API_KEY=sk-must-not-persist""#,
    ));
    fs::write(&path, format!("{raw_result}\n")).unwrap();
    assert!(matches!(sink.verify(), Err(AuditError::Corrupt)));
    let parent = path.parent().unwrap().to_path_buf();
    drop(sink);
    fs::remove_file(path).ok();
    fs::remove_dir(parent).ok();
}

#[cfg(unix)]
#[test]
fn audit_open_never_chmods_an_existing_insecure_parent() {
    use std::os::unix::fs::PermissionsExt;

    let path = temp_path("permissions");
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent).unwrap();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(JsonlAuditSink::open(&path).is_err());
    let mode = fs::metadata(parent).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
    fs::remove_dir(parent).ok();
}

#[test]
fn approval_requires_exact_binding_and_is_single_use() {
    let store = ApprovalStore::with_fixed_time(1);
    let request = ApprovalStore::test_request("approval-1", "owner-1", 100);
    let id = store.request(request).unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");

    let capability = store.approve(&id, "owner-1", &binding).unwrap();
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Approved);
    store.consume(&capability, "owner-1", &binding).unwrap();
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Consumed);
    assert!(matches!(
        store.consume(&capability, "owner-1", &binding),
        Err(ApprovalError::InvalidState)
    ));
}

#[test]
fn approval_drift_and_expiry_fail_closed_without_tool_authorization() {
    let store = ApprovalStore::with_fixed_time(1);
    let request = ApprovalStore::test_request("approval-2", "owner-1", 20);
    let id = store.request(request).unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");
    let drifted = ApprovalBinding::test("different-action", "workspace", "policy", "config");

    assert!(matches!(
        store.approve(&id, "owner-1", &drifted),
        Err(ApprovalError::BindingMismatch)
    ));
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Invalidated);

    let (expired_store, clock) = ApprovalStore::with_test_clock(1);
    let second = expired_store
        .request(ApprovalStore::test_request("approval-3", "owner-1", 20))
        .unwrap();
    clock.set(20);
    assert!(matches!(
        expired_store.approve(&second, "owner-1", &binding),
        Err(ApprovalError::Expired)
    ));
    assert_eq!(
        expired_store.state(&second).unwrap(),
        ApprovalState::Expired
    );
}

#[test]
fn capability_debug_does_not_expose_nonce() {
    let store = ApprovalStore::with_fixed_time(1);
    let id = store
        .request(ApprovalStore::test_request("approval-4", "owner-1", 100))
        .unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");
    let token: CapabilityToken = store.approve(&id, "owner-1", &binding).unwrap();
    let debug = format!("{token:?}");
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("nonce"));
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "orchester-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
        .join("audit.jsonl")
}

fn resign_entry(line: &str) -> String {
    let entry: serde_json::Value = serde_json::from_str(line).unwrap();
    let unsigned = line.replacen(
        &format!(
            r#""entry_hash":"{}""#,
            entry["entry_hash"].as_str().unwrap()
        ),
        r#""entry_hash":"""#,
        1,
    );
    let hash = Sha256::digest(unsigned.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    unsigned.replacen(
        r#""entry_hash":"""#,
        &format!(r#""entry_hash":"{hash}""#),
        1,
    )
}
