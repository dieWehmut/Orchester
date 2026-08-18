#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod api_error;
mod bootstrap;
mod config;
mod fragment;
mod health;
mod lifecycle;
mod listener;
mod router;
mod session;
mod workspace;

pub use api_error::{api_error_response, ApiErrorBody, ApiErrorCode, ApiErrorResponse};
pub use bootstrap::{bootstrap_response, BootstrapDto, BootstrapWorkspaceDto, ServerContext};
pub use config::{ServerConfig, ServerConfigError, StaticAssets};
pub use fragment::{FragmentTokenStore, FragmentTokenStoreError};
pub use health::{health_handler, health_response, HealthDto};
pub use lifecycle::{
    wait_for_shutdown, LifecycleError, ServerControl, ServerLifecycle, ServerState,
};
pub use listener::{bind_listener, ServerBindError};
pub use router::app_router;
pub use session::{
    fragment_exchange_handler, session_bootstrap_handler, session_revoke_handler,
    FragmentTokenExchangeRequestDto, SessionBootstrap, SessionBootstrapDto, SessionStore,
    SessionStoreError, SESSION_COOKIE_NAME,
};
pub use workspace::{select_workspace, WorkspaceSelectionError};
