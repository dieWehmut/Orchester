use orchester_netz::{
    wait_for_shutdown, LifecycleError, ServerControl, ServerLifecycle, ServerState,
};

#[test]
fn lifecycle_starts_in_starting_state_and_accepts_ordered_transitions() {
    let lifecycle = ServerLifecycle::new();

    assert_eq!(lifecycle.state(), ServerState::Starting);
    lifecycle
        .transition(ServerState::Running)
        .expect("start server");
    lifecycle
        .transition(ServerState::Stopping)
        .expect("stop server");
    lifecycle
        .transition(ServerState::Stopped)
        .expect("finish shutdown");

    assert_eq!(lifecycle.state(), ServerState::Stopped);
}

#[test]
fn lifecycle_rejects_skipping_a_state_or_restarting_after_stop() {
    let lifecycle = ServerLifecycle::new();

    assert_eq!(
        lifecycle.transition(ServerState::Stopped),
        Err(LifecycleError::InvalidTransition {
            from: ServerState::Starting,
            to: ServerState::Stopped,
        })
    );
    lifecycle
        .transition(ServerState::Running)
        .expect("start server");
    lifecycle
        .transition(ServerState::Stopping)
        .expect("stop server");
    lifecycle
        .transition(ServerState::Stopped)
        .expect("finish shutdown");
    assert_eq!(
        lifecycle.transition(ServerState::Running),
        Err(LifecycleError::InvalidTransition {
            from: ServerState::Stopped,
            to: ServerState::Running,
        })
    );
}

#[tokio::test]
async fn control_broadcasts_one_shutdown_signal_and_completes_without_a_socket() {
    let control = ServerControl::new();
    control.start().expect("start server");
    let receiver = control.subscribe_shutdown();
    let waiter = tokio::spawn(wait_for_shutdown(receiver));

    assert!(control.request_shutdown().expect("request shutdown"));
    assert!(!control.request_shutdown().expect("repeat shutdown"));
    waiter.await.expect("shutdown waiter");

    control.complete_shutdown().expect("complete shutdown");
    assert_eq!(control.state(), ServerState::Stopped);
}

#[tokio::test]
async fn control_cannot_shutdown_before_starting() {
    let control = ServerControl::new();

    assert_eq!(
        control.request_shutdown(),
        Err(LifecycleError::InvalidTransition {
            from: ServerState::Starting,
            to: ServerState::Stopping,
        })
    );
}

#[tokio::test]
async fn concurrent_shutdown_requests_are_idempotent() {
    let control = ServerControl::new();
    control.start().expect("start server");
    let first = control.clone();
    let second = control.clone();

    let (first, second) = tokio::join!(
        tokio::spawn(async move { first.request_shutdown() }),
        tokio::spawn(async move { second.request_shutdown() }),
    );
    let results = [
        first.expect("first shutdown task"),
        second.expect("second shutdown task"),
    ];

    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(true)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(false)))
            .count(),
        1
    );
    assert!(results.iter().all(Result::is_ok));
}
