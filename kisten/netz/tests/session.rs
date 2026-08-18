use std::time::Duration;

use orchester_netz::{SessionStore, SessionStoreError};

#[test]
fn session_store_issues_distinct_tokens_and_validates_csrf() {
    let store = SessionStore::new(Duration::from_secs(300));
    let first = store.issue().expect("first session");
    let second = store.issue().expect("second session");

    assert_ne!(first.session_cookie, second.session_cookie);
    assert_ne!(first.csrf_token, second.csrf_token);
    assert!(store.validate(&first.session_cookie, &first.csrf_token));
    assert!(!store.validate(&first.session_cookie, &second.csrf_token));
}

#[test]
fn session_store_rejects_expired_and_revoked_sessions() {
    let store = SessionStore::new(Duration::ZERO);
    let issued = store.issue().expect("expired session");

    assert!(!store.validate(&issued.session_cookie, &issued.csrf_token));

    let store = SessionStore::new(Duration::from_secs(300));
    let issued = store.issue().expect("revocable session");
    assert!(store.revoke(&issued.session_cookie));
    assert!(!store.revoke(&issued.session_cookie));
    assert_eq!(
        store.validate_result(&issued.session_cookie, &issued.csrf_token),
        Err(SessionStoreError::UnknownSession)
    );
}

#[test]
fn session_bootstrap_debug_output_redacts_raw_tokens() {
    let store = SessionStore::new(Duration::from_secs(300));
    let issued = store.issue().expect("session");
    let debug = format!("{issued:?}");

    assert!(!debug.contains(&issued.session_cookie));
    assert!(!debug.contains(&issued.csrf_token));
    assert!(debug.contains("REDACTED"));
}
