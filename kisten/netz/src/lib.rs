#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod config;
mod lifecycle;
mod listener;
mod workspace;

pub use config::{ServerConfig, ServerConfigError, StaticAssets};
pub use lifecycle::{LifecycleError, ServerLifecycle, ServerState};
pub use listener::{bind_listener, ServerBindError};
pub use workspace::{select_workspace, WorkspaceSelectionError};
