use std::fmt;

use async_trait::async_trait;
use orchester_modell::{LanguageModel, ModelError, ModelRequest, ModelResponse};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::harness::config::{ConfigError, ResolvedModelProfile, UserConfig};
use crate::harness::credentials::{CredentialStore, ProviderSecret};
use crate::harness::provider::{HttpTransport, ReqwestHttpTransport};

use super::{ResponsesLanguageModel, ResponsesModelError, ResponsesRequestOptions};

/// Errors raised while binding effective configuration to a Responses model.
#[derive(Debug, Error)]
pub enum ResponsesModelBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("cannot initialize the Responses HTTP transport")]
    Transport,
    #[error(transparent)]
    Endpoint(#[from] ResponsesModelError),
}

/// A Responses model plus the non-secret profile that governs it.
pub struct ConfiguredResponsesModel<T> {
    profile: ResolvedModelProfile,
    inner: ResponsesLanguageModel<T>,
}

impl<T> ConfiguredResponsesModel<T> {
    pub fn profile(&self) -> &ResolvedModelProfile {
        &self.profile
    }
}

impl<T> fmt::Debug for ConfiguredResponsesModel<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredResponsesModel")
            .field("profile", &self.profile)
            .field("inner", &self.inner)
            .finish()
    }
}

#[async_trait]
impl<T: HttpTransport + 'static> LanguageModel for ConfiguredResponsesModel<T> {
    async fn complete(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelResponse, ModelError> {
        if request.model != self.profile.model || request.store != self.profile.store {
            return Err(ModelError::Protocol);
        }
        self.inner.complete(request, cancel).await
    }
}

/// Build the production Responses model using the bounded reqwest transport.
pub fn build_responses_model<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
) -> Result<ConfiguredResponsesModel<ReqwestHttpTransport>, ResponsesModelBuildError> {
    let resolved = resolve_responses_config(config, credentials)?;
    let transport = ReqwestHttpTransport::new().map_err(|_| ResponsesModelBuildError::Transport)?;
    build_configured_model(resolved, transport)
}

/// Build a profile-bound model with an injected transport for offline tests or
/// embedding environments that provide their own network policy.
pub fn build_responses_model_with_transport<S, T>(
    config: &UserConfig,
    credentials: &S,
    transport: T,
) -> Result<ConfiguredResponsesModel<T>, ResponsesModelBuildError>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport,
{
    let resolved = resolve_responses_config(config, credentials)?;
    build_configured_model(resolved, transport)
}

fn resolve_responses_config<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    credentials: &S,
) -> Result<ResolvedResponsesConfig, ResponsesModelBuildError> {
    let profile = config.resolve_model_profile()?;
    let authorization = if profile.requires_auth {
        Some(config.resolve_provider_secret(&profile.provider, credentials)?)
    } else {
        None
    };
    let options = ResponsesRequestOptions {
        reasoning_effort: profile.reasoning_effort.clone(),
        service_tier: profile.service_tier.clone(),
    };
    Ok(ResolvedResponsesConfig {
        profile,
        authorization,
        options,
    })
}

fn build_configured_model<T: HttpTransport>(
    resolved: ResolvedResponsesConfig,
    transport: T,
) -> Result<ConfiguredResponsesModel<T>, ResponsesModelBuildError> {
    let ResolvedResponsesConfig {
        profile,
        authorization,
        options,
    } = resolved;
    let inner = ResponsesLanguageModel::new(&profile.base_url, transport, authorization, options)?;
    Ok(ConfiguredResponsesModel { profile, inner })
}

struct ResolvedResponsesConfig {
    profile: ResolvedModelProfile,
    authorization: Option<ProviderSecret>,
    options: ResponsesRequestOptions,
}
