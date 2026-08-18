#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod config;
mod listener;
mod workspace;

pub use config::{ServerConfig, ServerConfigError, StaticAssets};
pub use listener::{bind_listener, ServerBindError};
pub use workspace::{select_workspace, WorkspaceSelectionError};
