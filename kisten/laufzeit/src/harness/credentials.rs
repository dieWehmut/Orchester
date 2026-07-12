//! Credential storage for provider secrets.
//!
//! Secrets cross this module only as [`secrecy::SecretString`] values.  The
//! configuration layer stores references (for example `${secret:OpenAI}`),
//! while this module owns the one boundary at which a provider secret may be
//! resolved.  In particular, no implementation accepts a secret as a command
//! line argument or serializes a secret value.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// The stable service name used by the platform keyring.
pub const KEYRING_SERVICE: &str = "dev.orchester.cli";

/// Errors returned by credential backends.  Error text intentionally contains
/// only provider identifiers and operation names, never secret material.
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential provider name is empty")]
    EmptyProvider,
    #[error("credential provider name contains an invalid character")]
    InvalidProvider,
    #[error("{operation} failed in the secure credential store: {detail}")]
    Backend {
        operation: &'static str,
        detail: String,
    },
    #[error("secure credential storage is not available on this platform")]
    Unsupported,
    #[error("credential store lock is poisoned")]
    LockPoisoned,
}

/// A backend for provider secrets.
///
/// Implementations must not log, serialize, or otherwise retain a plaintext
/// copy beyond what the underlying secure store requires for the operation.
pub trait CredentialStore: Send + Sync {
    fn set(&self, provider: &str, secret: SecretString) -> Result<(), CredentialError>;
    fn get(&self, provider: &str) -> Result<Option<SecretString>, CredentialError>;
    fn clear(&self, provider: &str) -> Result<(), CredentialError>;

    /// Return whether a provider currently has a stored credential without
    /// exposing its value.
    fn present(&self, provider: &str) -> Result<bool, CredentialError> {
        Ok(self.get(provider)?.is_some())
    }
}

/// A secret wrapper handed to provider adapters after a successful lookup.
///
/// It deliberately has no `Serialize` implementation and its `Debug`/`Display`
/// output is redacted.  The explicit method makes the short-lived handoff to a
/// provider visible at call sites and avoids accidental formatting.
pub struct ProviderSecret(SecretString);

impl ProviderSecret {
    pub(crate) fn new(secret: SecretString) -> Self {
        Self(secret)
    }

    /// Borrow the secret for the duration of a provider request.
    pub fn expose_for_provider(&self) -> &str {
        self.0.expose_secret()
    }

    /// Consume the wrapper and return the secrecy-protected string.
    pub fn into_secret(self) -> SecretString {
        self.0
    }
}

impl AsRef<str> for ProviderSecret {
    fn as_ref(&self) -> &str {
        self.expose_for_provider()
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProviderSecret([REDACTED])")
    }
}

impl fmt::Display for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

fn validate_provider(provider: &str) -> Result<(), CredentialError> {
    if provider.trim().is_empty() {
        return Err(CredentialError::EmptyProvider);
    }
    // Keep the keyring account namespace portable and prevent control
    // characters from being passed to platform credential APIs.
    if !provider
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(CredentialError::InvalidProvider);
    }
    Ok(())
}

/// In-memory credential backend used by deterministic tests and dependency
/// injection.  It is intentionally not used as a production fallback.
#[derive(Clone, Default)]
pub struct InMemoryCredentialStore {
    values: Arc<RwLock<BTreeMap<String, SecretString>>>,
}

impl InMemoryCredentialStore {
    /// Build a test store with one provider value.  Production callers should
    /// use [`CredentialStore::set`] from a protected input path instead.
    pub fn with(provider: impl Into<String>, secret: impl Into<String>) -> Self {
        let store = Self::default();
        // The constructor is only a test convenience; invalid names result in
        // an empty store rather than a panic in test setup.
        let provider = provider.into();
        if validate_provider(&provider).is_ok() {
            let secret = SecretString::new(secret.into().into_boxed_str());
            if let Ok(mut values) = store.values.write() {
                values.insert(provider, secret);
            }
        }
        store
    }
}

impl fmt::Debug for InMemoryCredentialStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.values.read().map(|values| values.len()).unwrap_or(0);
        f.debug_struct("InMemoryCredentialStore")
            .field("entry_count", &count)
            .finish()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn set(&self, provider: &str, secret: SecretString) -> Result<(), CredentialError> {
        validate_provider(provider)?;
        let mut values = self
            .values
            .write()
            .map_err(|_| CredentialError::LockPoisoned)?;
        values.insert(provider.to_owned(), secret);
        Ok(())
    }

    fn get(&self, provider: &str) -> Result<Option<SecretString>, CredentialError> {
        validate_provider(provider)?;
        let values = self
            .values
            .read()
            .map_err(|_| CredentialError::LockPoisoned)?;
        Ok(values.get(provider).cloned())
    }

    fn clear(&self, provider: &str) -> Result<(), CredentialError> {
        validate_provider(provider)?;
        let mut values = self
            .values
            .write()
            .map_err(|_| CredentialError::LockPoisoned)?;
        values.remove(provider);
        Ok(())
    }
}

/// OS-backed credential backend.  The `keyring` crate selects Windows
/// Credential Manager, macOS Keychain, or the configured Linux kernel keyring
/// backend at compile time.
#[derive(Debug, Clone)]
pub struct KeyringCredentialStore {
    service: String,
}

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self {
            service: KEYRING_SERVICE.to_owned(),
        }
    }

    pub fn with_service(service: impl Into<String>) -> Result<Self, CredentialError> {
        let service = service.into();
        if service.trim().is_empty() {
            return Err(CredentialError::EmptyProvider);
        }
        Ok(Self { service })
    }

    fn entry(&self, provider: &str) -> Result<keyring::Entry, CredentialError> {
        validate_provider(provider)?;
        keyring::Entry::new(&self.service, provider).map_err(|_| secure_backend_error("open"))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn set(&self, provider: &str, secret: SecretString) -> Result<(), CredentialError> {
        let entry = self.entry(provider)?;
        entry
            .set_password(secret.expose_secret())
            .map_err(|_| secure_backend_error("set"))
    }

    fn get(&self, provider: &str) -> Result<Option<SecretString>, CredentialError> {
        let entry = self.entry(provider)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(SecretString::new(value.into_boxed_str()))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(secure_backend_error("get")),
        }
    }

    fn clear(&self, provider: &str) -> Result<(), CredentialError> {
        let entry = self.entry(provider)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(secure_backend_error("clear")),
        }
    }
}

fn secure_backend_error(operation: &'static str) -> CredentialError {
    // Do not forward platform error text: some credential providers include
    // account metadata (and a few third-party providers have historically
    // included secret-related details) in their Display implementation.
    CredentialError::Backend {
        operation,
        detail: "platform secure-store operation failed".into(),
    }
}

/// Resolve a value held by a credential backend into the provider-only wrapper
/// used by the model adapter boundary.
pub fn provider_secret<S: CredentialStore + ?Sized>(
    store: &S,
    provider: &str,
) -> Result<Option<ProviderSecret>, CredentialError> {
    Ok(store.get(provider)?.map(ProviderSecret::new))
}
