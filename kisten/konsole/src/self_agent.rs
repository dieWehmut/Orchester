//! Terminal rendering for the self-owned agent.
//!
//! The host itself — configuration loading, model selection, runtime caching —
//! lives in [`orchester_anwendung`], because a WebUI and a desktop shell need it
//! too and this crate is a `[[bin]]` nothing can link against. What stays here is
//! the half that is genuinely a terminal: functions that turn the host's data
//! into styled lines on a `Write`.

mod config;
mod credentials;
mod models;
mod permissions;
mod render;
mod resume;
mod status;

pub use config::render_config;
pub use credentials::{
    render_credential_cleared, render_credential_stored, render_credential_target,
};
pub use models::{render_model_selection, render_models, render_provider_written, safe_metadata};
pub use orchester_anwendung::{SelfAgentHost, SelfAgentHostError};
pub use permissions::render_permissions;
pub use render::{render_outcome, render_outcome_transcript};
pub use resume::render_resume;
pub use status::render_status;
