use orchester_netz::{LifecycleError, ServerLifecycle, ServerState};

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
