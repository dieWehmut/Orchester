use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ServerState {
    Starting = 0,
    Running = 1,
    Stopping = 2,
    Stopped = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidTransition { from: ServerState, to: ServerState },
}

#[derive(Debug)]
pub struct ServerLifecycle {
    state: AtomicU8,
}

impl ServerLifecycle {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(ServerState::Starting as u8),
        }
    }

    pub fn state(&self) -> ServerState {
        ServerState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub fn transition(&self, to: ServerState) -> Result<(), LifecycleError> {
        let from = self.state();
        if !from.can_transition_to(to) {
            return Err(LifecycleError::InvalidTransition { from, to });
        }

        self.state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|actual| LifecycleError::InvalidTransition {
                from: ServerState::from_u8(actual),
                to,
            })
    }
}

impl Default for ServerLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    fn can_transition_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Starting, Self::Running)
                | (Self::Running, Self::Stopping)
                | (Self::Stopping, Self::Stopped)
        )
    }

    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Starting,
            1 => Self::Running,
            2 => Self::Stopping,
            3 => Self::Stopped,
            _ => unreachable!("server lifecycle stored an invalid state"),
        }
    }
}
