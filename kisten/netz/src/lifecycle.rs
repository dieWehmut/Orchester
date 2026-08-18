use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tokio::sync::watch;

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

#[derive(Debug, Clone)]
pub struct ServerControl {
    lifecycle: Arc<ServerLifecycle>,
    shutdown: watch::Sender<bool>,
}

impl ServerControl {
    pub fn new() -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            lifecycle: Arc::new(ServerLifecycle::new()),
            shutdown,
        }
    }

    pub fn state(&self) -> ServerState {
        self.lifecycle.state()
    }

    pub fn start(&self) -> Result<(), LifecycleError> {
        self.lifecycle.transition(ServerState::Running)
    }

    pub fn request_shutdown(&self) -> Result<bool, LifecycleError> {
        match self.state() {
            ServerState::Running => match self.lifecycle.transition(ServerState::Stopping) {
                Ok(()) => Ok(!self.shutdown.send_replace(true)),
                Err(LifecycleError::InvalidTransition {
                    from: ServerState::Stopping | ServerState::Stopped,
                    ..
                }) => Ok(false),
                Err(error) => Err(error),
            },
            ServerState::Stopping | ServerState::Stopped => Ok(false),
            ServerState::Starting => Err(LifecycleError::InvalidTransition {
                from: ServerState::Starting,
                to: ServerState::Stopping,
            }),
        }
    }

    pub fn complete_shutdown(&self) -> Result<(), LifecycleError> {
        if self.state() == ServerState::Stopped {
            return Ok(());
        }
        self.lifecycle.transition(ServerState::Stopped)
    }

    pub fn subscribe_shutdown(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }
}

impl Default for ServerControl {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn wait_for_shutdown(mut receiver: watch::Receiver<bool>) {
    if *receiver.borrow() {
        return;
    }

    while receiver.changed().await.is_ok() {
        if *receiver.borrow() {
            return;
        }
    }
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
