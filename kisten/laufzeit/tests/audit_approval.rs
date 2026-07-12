use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_laufzeit::harness::approval::{
    ApprovalBinding, ApprovalError, ApprovalState, ApprovalStore, CapabilityToken,
};
use orchester_laufzeit::harness::audit::{AuditInput, JsonlAuditSink};

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
