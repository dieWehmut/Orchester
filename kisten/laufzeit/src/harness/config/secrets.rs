use std::fmt;

use secrecy::{ExposeSecret, SecretString};

use super::{
    ConfigError, CredentialStore, SecretReference, UserConfig, PROTECTED_CREDENTIAL_MARKER,
};

const MAX_CONFIGURED_SECRETS: usize = 256;
const MAX_SECRET_BYTES: usize = 64 * 1024;

/// All configured credential values needed by secret-aware runtime sinks.
///
/// Values remain private secrecy wrappers; callers outside Laufzeit can only
/// inspect the number of resolved credentials.
#[derive(Clone)]
pub struct ConfiguredSecretSet {
    pub(crate) values: Vec<SecretString>,
}

impl ConfiguredSecretSet {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl fmt::Debug for ConfiguredSecretSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredSecretSet")
            .field("count", &self.values.len())
            .finish()
    }
}

impl UserConfig {
    /// Resolve every protected literal and external secret reference into one
    /// bounded, deduplicated set for transcript and tool-output sanitizers.
    pub fn resolve_configured_secrets<S: CredentialStore + ?Sized>(
        &self,
        store: &S,
    ) -> Result<ConfiguredSecretSet, ConfigError> {
        let mut values = Vec::new();

        if let Some(vault) = self.credential_vault.0.as_ref() {
            for secret in vault.values() {
                push_unique(&mut values, secret.clone())?;
            }
        }

        for (name, value) in self.env() {
            if value != PROTECTED_CREDENTIAL_MARKER
                && SecretReference::parse(value)
                    .map_err(|_| ConfigError::InvalidSecretReference { path: name.clone() })?
                    .is_some()
            {
                push_unique(&mut values, self.resolve_secret(name, store)?.into_secret())?;
            }
        }

        for (provider, config) in self.model_providers() {
            let Some(value) = config.api_key.as_deref() else {
                continue;
            };
            let reference =
                SecretReference::parse(value).map_err(|_| ConfigError::InvalidSecretReference {
                    path: format!("model_providers.{provider}.api_key"),
                })?;
            if value != PROTECTED_CREDENTIAL_MARKER && reference.is_some() {
                push_unique(
                    &mut values,
                    self.resolve_provider_secret(provider, store)?.into_secret(),
                )?;
            }
        }

        Ok(ConfiguredSecretSet { values })
    }
}

fn push_unique(values: &mut Vec<SecretString>, secret: SecretString) -> Result<(), ConfigError> {
    let exposed = secret.expose_secret();
    if exposed.is_empty() || exposed.len() > MAX_SECRET_BYTES {
        return Err(secret_set_error("configured credential length is invalid"));
    }
    if values
        .iter()
        .any(|existing| existing.expose_secret() == exposed)
    {
        return Ok(());
    }
    if values.len() >= MAX_CONFIGURED_SECRETS {
        return Err(secret_set_error("too many configured credentials"));
    }
    values.push(secret);
    Ok(())
}

fn secret_set_error(message: &'static str) -> ConfigError {
    ConfigError::Validation {
        path: "<credentials>".into(),
        message: message.into(),
    }
}
