#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod config;
mod health;
mod lifecycle;
mod listener;
mod workspace;

pub use config::{ServerConfig, ServerConfigError, StaticAssets};
pub use health::{app_router, health_handler, health_response, HealthDto};
pub use lifecycle::{
    wait_for_shutdown, LifecycleError, ServerControl, ServerLifecycle, ServerState,
};
pub use listener::{bind_listener, ServerBindError};
pub use workspace::{select_workspace, WorkspaceSelectionError};
