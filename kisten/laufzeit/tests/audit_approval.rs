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
    fs::remove_file(path).ok();
}

#[test]
fn approval_requires_exact_binding_and_is_single_use() {
    let store = ApprovalStore::new();
    let request = ApprovalStore::test_request("approval-1", "owner-1", 100);
    let id = store.request(request).unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");

    let capability = store.approve(&id, "owner-1", 10, &binding).unwrap();
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Approved);
    store.consume(&capability, "owner-1", 11, &binding).unwrap();
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Consumed);
    assert!(matches!(
        store.consume(&capability, "owner-1", 12, &binding),
        Err(ApprovalError::InvalidState)
    ));
}

#[test]
fn approval_drift_and_expiry_fail_closed_without_tool_authorization() {
    let store = ApprovalStore::new();
    let request = ApprovalStore::test_request("approval-2", "owner-1", 20);
    let id = store.request(request).unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");
    let drifted = ApprovalBinding::test("different-action", "workspace", "policy", "config");

    assert!(matches!(
        store.approve(&id, "owner-1", 10, &drifted),
        Err(ApprovalError::BindingMismatch)
    ));
    assert_eq!(store.state(&id).unwrap(), ApprovalState::Invalidated);

    let second = store
        .request(ApprovalStore::test_request("approval-3", "owner-1", 20))
        .unwrap();
    assert!(matches!(
        store.approve(&second, "owner-1", 20, &binding),
        Err(ApprovalError::Expired)
    ));
    assert_eq!(store.state(&second).unwrap(), ApprovalState::Expired);
}

#[test]
fn capability_debug_does_not_expose_nonce() {
    let store = ApprovalStore::new();
    let id = store
        .request(ApprovalStore::test_request("approval-4", "owner-1", 100))
        .unwrap();
    let binding = ApprovalBinding::test("action-hash", "workspace", "policy", "config");
    let token: CapabilityToken = store.approve(&id, "owner-1", 1, &binding).unwrap();
    let debug = format!("{token:?}");
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("nonce"));
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "orchester-{label}-{}-{}.jsonl",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
