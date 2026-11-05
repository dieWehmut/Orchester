//! Bounded, read-only discovery from an already configured model provider.
use std::{collections::BTreeSet, time::Duration};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::harness::{
    config::{ConfigError, UserConfig, ANTHROPIC_WIRE_API},
    credentials::CredentialStore,
    provider::{CredentialHeader, HttpRequest, HttpTransport, HttpTransportError},
};

const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const MODEL_LIMIT: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredProviderModel {
    pub model: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Error)]
pub enum ModelDiscoveryError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Transport(#[from] HttpTransportError),
    #[error("provider did not return an available model directory")]
    Unavailable,
    #[error("provider returned an invalid or oversized model directory")]
    InvalidDirectory,
}

/// The caller supplies only a configured provider ID. Endpoint and credential
/// are resolved together from protected configuration, never from response data.
/// Discovery does not change the saved catalog; the editor previews any merge.
pub async fn discover_provider_models<S, T>(
    config: &UserConfig,
    credentials: &S,
    provider: &str,
    transport: &T,
    cancel: CancellationToken,
) -> Result<Vec<DiscoveredProviderModel>, ModelDiscoveryError>
where
    S: CredentialStore + ?Sized,
    T: HttpTransport + ?Sized,
{
    let selected = config.with_model_provider(provider)?;
    let profile = selected.resolve_model_profile()?;
    let authorization = if profile.requires_auth {
        Some(selected.resolve_provider_secret(provider, credentials)?)
    } else {
        None
    };
    let mut request = HttpRequest::get(models_endpoint(&profile.base_url)?, authorization)?
        .with_timeout(Duration::from_secs(15))?
        .with_response_limit(RESPONSE_LIMIT)?;
    if profile.wire_api == ANTHROPIC_WIRE_API {
        request = request
            .with_credential_header(CredentialHeader::ApiKey)
            .with_protocol_headers(vec![("anthropic-version", "2023-06-01")])?;
    }
    let response = transport.send(request, cancel).await?;
    if !(200..300).contains(&response.status()) {
        return Err(ModelDiscoveryError::Unavailable);
    }
    parse_directory(response.body())
}

fn models_endpoint(base_url: &str) -> Result<Url, HttpTransportError> {
    let mut endpoint = Url::parse(base_url).map_err(|_| HttpTransportError::InvalidRequest)?;
    // Match the inference adapter's handling of root, /v1, relay prefixes and
    // complete inference endpoints without accepting a provider-supplied URL.
    let base = endpoint.path().trim_end_matches('/');
    let path = if let Some(prefix) = base
        .strip_suffix("/responses")
        .or_else(|| base.strip_suffix("/messages"))
    {
        format!("{prefix}/models")
    } else if base.ends_with("/models") {
        base.to_owned()
    } else if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    };
    endpoint.set_path(&path);
    HttpRequest::get(endpoint.clone(), None)?;
    Ok(endpoint)
}

#[derive(Deserialize)]
struct Directory {
    data: Vec<DirectoryEntry>,
}

#[derive(Deserialize)]
struct DirectoryEntry {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

fn parse_directory(bytes: &[u8]) -> Result<Vec<DiscoveredProviderModel>, ModelDiscoveryError> {
    if bytes.len() > RESPONSE_LIMIT {
        return Err(ModelDiscoveryError::InvalidDirectory);
    }
    let directory: Directory =
        serde_json::from_slice(bytes).map_err(|_| ModelDiscoveryError::InvalidDirectory)?;
    if directory.data.len() > MODEL_LIMIT {
        return Err(ModelDiscoveryError::InvalidDirectory);
    }
    let mut ids = BTreeSet::new();
    let mut models = Vec::new();
    for entry in directory.data {
        let id = entry.id.trim();
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
            return Err(ModelDiscoveryError::InvalidDirectory);
        }
        let name = entry
            .display_name
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if name
            .as_ref()
            .is_some_and(|value| value.len() > 256 || value.chars().any(char::is_control))
        {
            return Err(ModelDiscoveryError::InvalidDirectory);
        }
        if ids.insert(id.to_owned()) {
            models.push(DiscoveredProviderModel {
                model: id.to_owned(),
                display_name: name,
            });
        }
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_stays_on_the_configured_origin_and_inference_prefix() {
        for (base, expected) in [
            ("https://relay.test", "https://relay.test/v1/models"),
            ("https://relay.test/v1/", "https://relay.test/v1/models"),
            (
                "https://relay.test/anthropic",
                "https://relay.test/anthropic/v1/models",
            ),
            (
                "https://relay.test/v1/messages",
                "https://relay.test/v1/models",
            ),
            (
                "https://relay.test/openai/v1/responses",
                "https://relay.test/openai/v1/models",
            ),
        ] {
            assert_eq!(models_endpoint(base).unwrap().as_str(), expected);
        }
        for bad in [
            "http://public.test/v1",
            "https://user:pass@relay.test",
            "https://relay.test?token=secret",
        ] {
            assert!(models_endpoint(bad).is_err());
        }
    }

    #[test]
    fn directory_deduplicates_ids_and_rejects_unbounded_or_malformed_data() {
        let models = parse_directory(br#"{"data":[{"id":"first","display_name":"First model"},{"id":"first"},{"id":"second","extra":"ignored"}]}"#).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].display_name.as_deref(), Some("First model"));
        for bad in [
            br#"{"data":[{"id":""}]}"#.as_slice(),
            br#"{"data":[{"id":"model\nname"}]}"#,
            br#"{"error":"not available"}"#,
        ] {
            assert!(parse_directory(bad).is_err());
        }
        assert!(parse_directory(&vec![b' '; RESPONSE_LIMIT + 1]).is_err());
        let excessive = serde_json::json!({"data": (0..=MODEL_LIMIT).map(|i| serde_json::json!({"id":format!("m{i}")})).collect::<Vec<_>>()});
        assert!(parse_directory(&serde_json::to_vec(&excessive).unwrap()).is_err());
    }
}
