#![forbid(unsafe_code)]

//! Loopback HTTP and WebSocket service for Orchester frontends.

mod agent_catalog;
mod api_error;
mod bootstrap;
mod config;
mod fragment;
mod health;
mod lifecycle;
mod listener;
mod model_catalog;
mod router;
mod session;
mod session_history;
mod workspace;

pub use agent_catalog::{
    agent_catalog_response, AgentAvailabilityDto, AgentCatalogDto, AgentSummaryDto,
    AGENT_CATALOG_SCHEMA_VERSION,
};
pub use api_error::{api_error_response, ApiErrorBody, ApiErrorCode, ApiErrorResponse};
pub use bootstrap::{bootstrap_response, BootstrapDto, BootstrapWorkspaceDto, ServerContext};
pub use config::{ServerConfig, ServerConfigError, StaticAssets};
pub use fragment::{FragmentTokenStore, FragmentTokenStoreError};
pub use health::{health_handler, health_response, HealthDto};
pub use lifecycle::{
    wait_for_shutdown, LifecycleError, ServerControl, ServerLifecycle, ServerState,
};
pub use listener::{bind_listener, ServerBindError};
pub use model_catalog::{
    model_catalog_response, ActiveModelDto, ModelCatalogDto, ModelChoiceDto, ModelProfileDto,
    ProviderChoiceDto, ProviderChoiceStateDto, MODEL_CATALOG_SCHEMA_VERSION,
};
pub use router::app_router;
pub use session::{
    fragment_exchange_handler, session_bootstrap_handler, session_revoke_handler,
    FragmentTokenExchangeRequestDto, SessionBootstrap, SessionBootstrapDto, SessionStore,
    SessionStoreError, SESSION_COOKIE_NAME,
};
pub use session_history::{
    session_detail_response, session_page_response, SessionDetailDto, SessionOutcomeDto,
    SessionPageDto, SessionSummaryDto, SESSION_HISTORY_SCHEMA_VERSION, SESSION_LIST_DEFAULT_LIMIT,
    SESSION_PROMPT_MAX_CHARS, SESSION_RESULT_MAX_CHARS,
};
pub use workspace::{select_workspace, WorkspaceSelectionError};
