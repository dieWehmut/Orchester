//! Dispatch from the configured wire API to the adapter that speaks it.
//!
//! The self-agent service binds one concrete model type, so a second wire API
//! becomes an enum arm rather than a trait object. The profile guard and the
//! credential resolution then stay identical no matter which wire is selected.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use orchester_modell::{LanguageModel, ModelError, ModelEventSink, ModelRequest, ModelResponse};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::harness::config::{
    ConfigError, ResolvedModelProfile, UserConfig, ANTHROPIC_WIRE_API, RESPONSES_WIRE_API,
};
use crate::harness::credentials::{CredentialStore, ProviderSecret};

use super::anthropic::{
    AnthropicLanguageModel, AnthropicModelError, AnthropicRequestOptions, DEFAULT_MAX_OUTPUT_TOKENS,
};
use super::responses::{ResponsesLanguageModel, ResponsesModelError, ResponsesRequestOptions};
use super::{HttpTransport, ReqwestHttpTransport};

/// Errors raised while binding effective configuration to a wire adapter.
#[derive(Debug, Error)]
pub enum WireModelBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot initialize the model HTTP transport")]
    Transport,
    #[error("the configured wire API has no model adapter")]
    UnsupportedWireApi,
    #[error(transparent)]
    ResponsesEndpoint(#[from] ResponsesModelError),
    #[error(transparent)]
    AnthropicEndpoint(#[from] AnthropicModelError),
}

/// A wire adapter plus the non-secret profile that governs it.
pub struct ConfiguredWireModel<T> {
    profile: ResolvedModelProfile,
    wire: WireModel<T>,
}

enum WireModel<T> {
    Responses(ResponsesLanguageModel<T>),
    Anthropic(AnthropicLanguageModel<T>),
}

impl<T> ConfiguredWireModel<T> {
    pub fn profile(&self) -> &ResolvedModelProfile {
        &self.profile
    }

    /// A request that does not match the profile this model was built for is
    /// refused, so a stale request can never be sent under another provider's
    /// credential.
    fn accepts(&self, request: &ModelRequest) -> bool {
        request.model == self.profile.model && request.store == self.profile.store
    }
}

impl<T> fmt::Debug for ConfiguredWireModel<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredWireModel")
            .field("profile", &self.profile)
            .field("wire", &self.wire)
            .finish()
    }
}

impl<T> fmt::Debug for WireModel<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Responses(model) => model.fmt(formatter),
            Self::Anthropic(model) => model.fmt(formatter),
        }
    }
}

#[async_trait]
impl<T: HttpTransport + 'static> LanguageModel for ConfiguredWireModel<T> {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        if !self.accepts(&request) {
            return Err(ModelError::Protocol);
        }
        match &self.wire {
            WireModel::Responses(model) => model.complete(request, cancel).await,
            WireModel::Anthropic(model) => model.complete(request, cancel).await,
        }
    }

    async fn complete_with_events(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
        events: Option<Arc<dyn ModelEventSink>>,
    ) -> Result<ModelResponse, ModelError> {
        if !self.accepts(&request) {
            return Err(ModelError::Protocol);
        }
        match &self.wire {
            WireModel::Responses(model) => {
                model.complete_with_events(request, cancel, events).await
            }
            WireModel::Anthropic(model) => {
                model.complete_with_events(request, cancel, events).await
            }
        }
    }
}

/// Build the model for the configured wire API over the bounded reqwest
/// transport.
pub fn build_wire_model<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
) -> Result<ConfiguredWireModel<ReqwestHttpTransport>, WireModelBuildError> {
    let transport = ReqwestHttpTransport::new().map_err(|_| WireModelBuildError::Transport)?;
    build_wire_model_with_transport(config, credentials, transport)
}

/// Build a profile-bound model with an injected transport for offline tests or
/// embedding environments that provide their own network policy.
pub fn build_wire_model_with_transport<S, T>(
    config: &UserConfig,
    credentials: &S,
    transport: T,
) -> Result<ConfiguredWireModel<T>, WireModelBuildError>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport,
{
    let profile = config.resolve_model_profile()?;
    let authorization = if profile.requires_auth {
        Some(config.resolve_provider_secret(&profile.provider, credentials)?)
    } else {
        None
    };
    let wire = build_wire(&profile, transport, authorization)?;
    Ok(ConfiguredWireModel { profile, wire })
}

fn build_wire<T: HttpTransport>(
    profile: &ResolvedModelProfile,
    transport: T,
    authorization: Option<ProviderSecret>,
) -> Result<WireModel<T>, WireModelBuildError> {
    match profile.wire_api.as_str() {
        RESPONSES_WIRE_API => Ok(WireModel::Responses(ResponsesLanguageModel::new(
            &profile.base_url,
            transport,
            authorization,
            ResponsesRequestOptions {
                reasoning_effort: profile.reasoning_effort.clone(),
                service_tier: profile.service_tier.clone(),
            },
        )?)),
        // Messages has no reasoning-effort field the harness can carry yet: its
        // extended thinking blocks must be replayed on the assistant turn that
        // made a tool call, which the agent loop does not round-trip.
        ANTHROPIC_WIRE_API => Ok(WireModel::Anthropic(AnthropicLanguageModel::new(
            &profile.base_url,
            transport,
            authorization,
            AnthropicRequestOptions {
                max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
                service_tier: profile.service_tier.clone(),
            },
        )?)),
        _ => Err(WireModelBuildError::UnsupportedWireApi),
    }
}
