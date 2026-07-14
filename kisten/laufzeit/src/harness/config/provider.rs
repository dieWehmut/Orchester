//! Resolution of the active, non-secret model transport profile.

use std::fmt;

use super::{ConfigError, UserConfig};
use url::{Host, Url};

const RESPONSES_WIRE_API: &str = "responses";

/// The validated provider settings needed to construct a model transport.
///
/// Credential values and references are deliberately absent. A caller that
/// requires authentication resolves [`crate::harness::credentials::ProviderSecret`]
/// separately and only at the request boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct ResolvedModelProfile {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub base_url: String,
    pub wire_api: String,
    pub reasoning_effort: Option<String>,
    pub plan_mode_reasoning_effort: Option<String>,
    pub store: bool,
    pub service_tier: Option<String>,
    pub requires_auth: bool,
}

impl fmt::Debug for ResolvedModelProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedModelProfile")
            .field("provider", &self.provider)
            .field("provider_name", &self.provider_name)
            .field("model", &self.model)
            .field("base_url", &"[REDACTED]")
            .field("wire_api", &self.wire_api)
            .field("reasoning_effort", &self.reasoning_effort)
            .field(
                "plan_mode_reasoning_effort",
                &self.plan_mode_reasoning_effort,
            )
            .field("store", &self.store)
            .field("service_tier", &self.service_tier)
            .field("requires_auth", &self.requires_auth)
            .finish()
    }
}

impl UserConfig {
    /// Resolve the selected provider and model into a validated, non-secret
    /// transport profile.
    pub fn resolve_model_profile(&self) -> Result<ResolvedModelProfile, ConfigError> {
        let provider = required_value(
            self.model_provider.as_deref(),
            "model_provider",
            "active model provider is not configured",
        )?;
        if !provider
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        {
            return Err(validation(
                "model_provider",
                "provider identifier contains an invalid character",
            ));
        }
        let provider_config = self.model_providers().get(provider).ok_or_else(|| {
            validation(
                "model_provider",
                "active provider does not name a configured user profile",
            )
        })?;
        let model = required_value(
            self.model.as_deref(),
            "model",
            "active model is not configured",
        )?;
        let base_url_path = format!("model_providers.{provider}.base_url");
        let base_url = required_value(
            provider_config.base_url.as_deref(),
            &base_url_path,
            "provider base URL is not configured",
        )?;
        validate_base_url(base_url, &base_url_path)?;

        let wire_api_path = format!("model_providers.{provider}.wire_api");
        let wire_api = provider_config
            .wire_api
            .as_deref()
            .unwrap_or(RESPONSES_WIRE_API)
            .trim();
        if wire_api != RESPONSES_WIRE_API {
            return Err(validation(
                wire_api_path,
                "unsupported wire API; supported value is 'responses'",
            ));
        }

        let provider_name = provider_config
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(provider);

        Ok(ResolvedModelProfile {
            provider: provider.to_owned(),
            provider_name: provider_name.to_owned(),
            model: model.to_owned(),
            base_url: base_url.to_owned(),
            wire_api: wire_api.to_owned(),
            reasoning_effort: normalized_optional(self.model_reasoning_effort.as_deref()),
            plan_mode_reasoning_effort: normalized_optional(
                self.plan_mode_reasoning_effort.as_deref(),
            ),
            store: !self.disable_response_storage,
            service_tier: normalized_optional(self.service_tier.as_deref()),
            requires_auth: provider_config.requires_openai_auth,
        })
    }
}

fn required_value<'a>(
    value: Option<&'a str>,
    path: impl Into<String>,
    message: &'static str,
) -> Result<&'a str, ConfigError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| validation(path, message))
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_base_url(value: &str, path: &str) -> Result<(), ConfigError> {
    if value
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(validation(path, "provider base URL is not a valid URL"));
    }
    let parsed = Url::parse(value)
        .map_err(|_| validation(path, "provider base URL is not a valid URL"))?;
    let host = parsed
        .host()
        .ok_or_else(|| validation(path, "provider base URL must include a host"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(validation(
            path,
            "embedded credentials are not allowed in provider base URLs",
        ));
    }
    if parsed.query().is_some() {
        return Err(validation(
            path,
            "query parameters are not allowed in provider base URLs",
        ));
    }
    if parsed.fragment().is_some() {
        return Err(validation(
            path,
            "fragments are not allowed in provider base URLs",
        ));
    }

    if parsed.scheme() == "https" {
        return Ok(());
    }
    if parsed.scheme() == "http" && is_loopback_host(host) {
        return Ok(());
    }
    Err(validation(
        path,
        "provider base URL must use HTTPS; HTTP is allowed only for loopback hosts",
    ))
}

fn is_loopback_host(host: Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => domain
            .trim_end_matches('.')
            .eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

fn validation(path: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError::Validation {
        path: path.into(),
        message: message.into(),
    }
}
