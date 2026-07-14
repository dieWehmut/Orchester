//! Typed, layered JSONC configuration and secret-reference resolution.
//!
//! Configuration is parsed into a strict schema before it can influence the
//! harness.  User configuration may select credential references, while a
//! project file is scanned as an untrusted input and cannot introduce provider
//! credentials, URLs, or weaker security decisions.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use orchester_protokoll::PolicyDecision;
use secrecy::SecretString;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use thiserror::Error;
use zeroize::Zeroize;

use super::credentials::{CredentialError, CredentialStore, ProviderSecret};

mod protected_file;
mod provider;

pub use provider::ResolvedModelProfile;

/// Relative path of the per-user configuration file.
pub const USER_CONFIG: &str = ".orchester/orchester.jsonc";
/// Relative path of a project/workspace configuration file.
pub const PROJECT_CONFIG: &str = ".orchester/project.jsonc";

const PROTECTED_CREDENTIAL_MARKER: &str = "<redacted>";

/// Configuration loading and validation failures.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("user home directory is unavailable")]
    HomeDirectoryUnavailable,
    #[error("invalid JSONC configuration: {0}")]
    Parse(String),
    #[error("configuration field '{path}' is invalid: {message}")]
    Validation { path: String, message: String },
    #[error("project configuration field '{path}' is not allowed")]
    ForbiddenProjectField { path: String },
    #[error("project configuration would relax the user security policy")]
    SecurityRelaxation,
    #[error("plaintext secret is not allowed at configuration field '{path}'")]
    PlaintextSecret { path: String },
    #[error("invalid secret reference at configuration field '{path}'")]
    InvalidSecretReference { path: String },
    #[error("secret provider '{provider}' is unavailable")]
    SecretUnavailable { provider: String },
    #[error("insecure permissions on '{path}': expected {expected}, found {actual}")]
    InsecurePermissions {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("protected configuration file exceeds the 1 MiB limit")]
    ProtectedFileTooLarge,
    #[error("protected configuration file is not valid UTF-8")]
    ProtectedFileInvalidUtf8,
    #[error("protected configuration file I/O failed")]
    ProtectedFileIo,
    #[error("protected configuration file failed secure handle validation")]
    ProtectedFileSecurity,
    #[error(transparent)]
    Credential(#[from] CredentialError),
}

/// A reference used in an environment or provider field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretReference {
    /// Resolve a value from [`CredentialStore`] by provider name.
    Provider(String),
    /// Resolve another configured environment entry.  This is useful for the
    /// provider shape used by the existing Codex-compatible configuration.
    Environment(String),
}

/// Syntax failure for a reference.  It carries no source text so malformed
/// input cannot be echoed by an error formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid secret reference")]
pub struct SecretReferenceError;

impl SecretReference {
    /// Parse an exact `${secret:Provider}` or `${env:NAME}` expression.
    pub fn parse(value: &str) -> Result<Option<Self>, SecretReferenceError> {
        if !value.starts_with("${") {
            return Ok(None);
        }
        if !value.ends_with('}') {
            return Err(SecretReferenceError);
        }
        let body = &value[2..value.len() - 1];
        let (kind, name) = body.split_once(':').ok_or(SecretReferenceError)?;
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(SecretReferenceError);
        }
        match kind {
            "secret" => Ok(Some(Self::Provider(name.to_owned()))),
            "env" => Ok(Some(Self::Environment(name.to_owned()))),
            _ => Err(SecretReferenceError),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Provider(name) => format!("${{secret:{name}}}"),
            Self::Environment(name) => format!("${{env:{name}}}"),
        }
    }
}

/// Top-level user configuration.  Security-sensitive nested objects reject
/// unknown fields so typos cannot silently weaken governance.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default, serialize_with = "serialize_env")]
    env: BTreeMap<String, String>,
    #[serde(default)]
    pub model_provider: Option<String>,
    /// Backward-compatible top-level spelling used by Codex-style config.
    /// It is normalized into `governance.approval_reviewer` at load time.
    #[serde(default, alias = "approval_reviewer")]
    pub approvals_reviewer: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub review_model: Option<String>,
    #[serde(default)]
    pub model_reasoning_effort: Option<String>,
    #[serde(default)]
    pub plan_mode_reasoning_effort: Option<String>,
    #[serde(default)]
    pub disable_response_storage: bool,
    #[serde(default)]
    pub network_access: Option<String>,
    #[serde(default)]
    pub windows_wsl_setup_acknowledged: bool,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    model_providers: BTreeMap<String, ProviderConfig>,
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectTrustConfig>,
    #[serde(default)]
    pub governance: GovernanceConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub validators: Vec<ValidatorConfig>,
    #[serde(default)]
    pub tui: TuiConfig,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
    #[serde(default)]
    pub windows: Option<WindowsConfig>,
    #[serde(default)]
    pub notice: Option<NoticeConfig>,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginConfig>,
    /// Literal credentials extracted from a protected user file. The immutable
    /// vault is private and skipped by serde, so public field mutation cannot
    /// create a new privileged credential binding.
    #[serde(skip)]
    credential_vault: CredentialVault,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CredentialSlot {
    Environment(String),
    ProviderApiKey(String),
}

#[derive(Clone, Default)]
struct CredentialVault(Option<Arc<BTreeMap<CredentialSlot, SecretString>>>);

impl CredentialVault {
    fn from_values(values: BTreeMap<CredentialSlot, SecretString>) -> Self {
        if values.is_empty() {
            Self::default()
        } else {
            Self(Some(Arc::new(values)))
        }
    }

    fn get(&self, slot: &CredentialSlot) -> Option<SecretString> {
        self.0.as_ref()?.get(slot).cloned()
    }

    fn binds_marker(&self, slot: &CredentialSlot, value: &str) -> bool {
        value == PROTECTED_CREDENTIAL_MARKER
            && self
                .0
                .as_ref()
                .is_some_and(|credentials| credentials.contains_key(slot))
    }
}

impl PartialEq for CredentialVault {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for CredentialVault {}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            schema: None,
            version: default_version(),
            env: BTreeMap::new(),
            model_provider: None,
            approvals_reviewer: None,
            model: None,
            review_model: None,
            model_reasoning_effort: None,
            plan_mode_reasoning_effort: None,
            disable_response_storage: false,
            network_access: None,
            windows_wsl_setup_acknowledged: false,
            service_tier: None,
            model_providers: BTreeMap::new(),
            projects: BTreeMap::new(),
            governance: GovernanceConfig::default(),
            limits: LimitsConfig::default(),
            validators: Vec::new(),
            tui: TuiConfig::default(),
            features: BTreeMap::new(),
            windows: None,
            notice: None,
            plugins: BTreeMap::new(),
            credential_vault: CredentialVault::default(),
        }
    }
}

impl fmt::Debug for UserConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_redacted_debug("UserConfig", self, formatter)
    }
}

impl UserConfig {
    /// Read the configured environment entries without exposing a mutable map.
    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    /// Read provider profiles without exposing a mutable map. Provider values
    /// remain validated configuration snapshots and cannot replace vault slots.
    pub fn model_providers(&self) -> &BTreeMap<String, ProviderConfig> {
        &self.model_providers
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.version != 1 {
            return Err(ConfigError::Validation {
                path: "version".into(),
                message: "unsupported configuration version".into(),
            });
        }
        for (name, value) in &self.env {
            validate_reference_syntax(value, name)?;
            if is_sensitive_name(name)
                && SecretReference::parse(value).ok().flatten().is_none()
                && !self
                    .credential_vault
                    .binds_marker(&CredentialSlot::Environment(name.clone()), value)
            {
                return Err(ConfigError::PlaintextSecret { path: name.clone() });
            }
        }
        for (provider, config) in &self.model_providers {
            if let Some(value) = config.api_key.as_deref() {
                let reference = SecretReference::parse(value).map_err(|_| {
                    ConfigError::InvalidSecretReference {
                        path: format!("model_providers.{provider}.api_key"),
                    }
                })?;
                if reference.is_none()
                    && !self
                        .credential_vault
                        .binds_marker(&CredentialSlot::ProviderApiKey(provider.clone()), value)
                {
                    return Err(ConfigError::PlaintextSecret {
                        path: format!("model_providers.{provider}.api_key"),
                    });
                }
            }
            if let Some(value) = config.base_url.as_deref() {
                validate_reference_syntax(value, &format!("model_providers.{provider}.base_url"))?;
            }
        }
        Ok(())
    }

    /// Resolve a named environment value through the credential store.
    pub fn resolve_secret<S: CredentialStore + ?Sized>(
        &self,
        name: &str,
        store: &S,
    ) -> Result<ProviderSecret, ConfigError> {
        let mut stack = Vec::new();
        let secret = self.resolve_secret_inner(name, store, &mut stack)?;
        Ok(ProviderSecret::new(secret))
    }

    fn resolve_secret_inner<S: CredentialStore + ?Sized>(
        &self,
        name: &str,
        store: &S,
        stack: &mut Vec<String>,
    ) -> Result<SecretString, ConfigError> {
        if stack.iter().any(|item| item == name) {
            return Err(ConfigError::Validation {
                path: name.to_owned(),
                message: "secret environment references contain a cycle".into(),
            });
        }
        let value = self
            .env
            .get(name)
            .ok_or_else(|| ConfigError::SecretUnavailable {
                provider: name.to_owned(),
            })?;
        let slot = CredentialSlot::Environment(name.to_owned());
        if value == PROTECTED_CREDENTIAL_MARKER {
            return self
                .credential_vault
                .get(&slot)
                .ok_or_else(|| ConfigError::PlaintextSecret {
                    path: name.to_owned(),
                });
        }
        let reference =
            SecretReference::parse(value).map_err(|_| ConfigError::InvalidSecretReference {
                path: name.to_owned(),
            })?;
        let reference = reference.ok_or_else(|| ConfigError::PlaintextSecret {
            path: name.to_owned(),
        })?;
        stack.push(name.to_owned());
        let result = match reference {
            SecretReference::Provider(provider) => store
                .get(&provider)?
                .ok_or(ConfigError::SecretUnavailable { provider }),
            SecretReference::Environment(next) => self.resolve_secret_inner(&next, store, stack),
        };
        stack.pop();
        result
    }

    /// Resolve a provider's configured API key reference.
    pub fn resolve_provider_secret<S: CredentialStore + ?Sized>(
        &self,
        provider: &str,
        store: &S,
    ) -> Result<ProviderSecret, ConfigError> {
        let config = self
            .model_providers
            .get(provider)
            .ok_or_else(|| ConfigError::Validation {
                path: format!("model_providers.{provider}"),
                message: "provider is not configured".into(),
            })?;
        let value = config
            .api_key
            .as_deref()
            .ok_or_else(|| ConfigError::SecretUnavailable {
                provider: provider.to_owned(),
            })?;
        let slot = CredentialSlot::ProviderApiKey(provider.to_owned());
        if value == PROTECTED_CREDENTIAL_MARKER {
            return self
                .credential_vault
                .get(&slot)
                .map(ProviderSecret::new)
                .ok_or_else(|| ConfigError::PlaintextSecret {
                    path: format!("model_providers.{provider}.api_key"),
                });
        }
        let reference =
            SecretReference::parse(value).map_err(|_| ConfigError::InvalidSecretReference {
                path: format!("model_providers.{provider}.api_key"),
            })?;
        match reference {
            Some(SecretReference::Provider(name)) => store
                .get(&name)?
                .map(ProviderSecret::new)
                .ok_or(ConfigError::SecretUnavailable { provider: name }),
            Some(SecretReference::Environment(name)) => self.resolve_secret(&name, store),
            None => Err(ConfigError::PlaintextSecret {
                path: format!("model_providers.{provider}.api_key"),
            }),
        }
    }

    /// Build a JSON representation safe for `config show`.
    pub fn redacted(&self) -> RedactedConfig {
        let value =
            serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()));
        RedactedConfig {
            value: redact_value(value),
        }
    }

    pub fn redacted_json(&self) -> String {
        self.redacted().json()
    }

    /// Build the `config show` view with a present/absent status obtained from
    /// the injected credential backend.  The credential value itself never
    /// enters the serialized tree.
    pub fn redacted_with_credentials<S: CredentialStore + ?Sized>(
        &self,
        store: &S,
    ) -> Result<RedactedConfig, ConfigError> {
        let value = serde_json::to_value(self).map_err(|error| ConfigError::Validation {
            path: "<root>".into(),
            message: format!("cannot build redacted view: {error}"),
        })?;
        Ok(RedactedConfig {
            value: redact_value_with_credentials(value, self, store, &mut Vec::new())?,
        })
    }

    fn secret_reference_present<S: CredentialStore + ?Sized>(
        &self,
        reference: &str,
        store: &S,
    ) -> Result<bool, ConfigError> {
        match SecretReference::parse(reference).map_err(|_| {
            ConfigError::InvalidSecretReference {
                path: "<redacted-view>".into(),
            }
        })? {
            Some(SecretReference::Provider(provider)) => Ok(store.present(&provider)?),
            Some(SecretReference::Environment(name)) => match self.resolve_secret(&name, store) {
                Ok(_) => Ok(true),
                Err(ConfigError::SecretUnavailable { .. }) => Ok(false),
                Err(error) => Err(error),
            },
            None => Ok(false),
        }
    }
}

/// Project configuration.  It intentionally has no `env` or provider map:
/// projects may select existing profiles and tighten governance, but cannot
/// introduce a credential source or provider endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub review_model: Option<String>,
    #[serde(default)]
    pub model_reasoning_effort: Option<String>,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    pub governance: Option<GovernanceOverrides>,
    #[serde(default)]
    pub limits: Option<LimitsOverrides>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub tui: Option<TuiConfig>,
}

/// A named provider profile.  Literal `api_key` values are accepted only from
/// a protected user file and are always redacted when serialized or formatted.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default, serialize_with = "serialize_provider_api_key")]
    pub api_key: Option<String>,
    #[serde(default)]
    pub wire_api: Option<String>,
    #[serde(default)]
    pub requires_openai_auth: bool,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_redacted_debug("ProviderConfig", self, formatter)
    }
}

/// Security policy knobs.  The enum's ordering is intentional: a larger
/// value is more restrictive and therefore wins a layer merge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GovernanceConfig {
    #[serde(default = "default_approval_reviewer", alias = "approvals_reviewer")]
    pub approval_reviewer: String,
    #[serde(default = "default_ask")]
    pub tool_network: PolicyDecision,
    #[serde(default = "default_deny")]
    pub out_of_workspace: PolicyDecision,
    #[serde(default = "default_deny")]
    pub shell_interpreters: PolicyDecision,
    #[serde(default = "default_approval_ttl")]
    pub approval_ttl_seconds: u64,
}

/// Optional project/CLI governance values.  Keeping absence explicit avoids a
/// partially specified project object accidentally replacing a stricter user
/// value with the built-in default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct GovernanceOverrides {
    #[serde(default, alias = "approvals_reviewer")]
    pub approval_reviewer: Option<String>,
    #[serde(default)]
    pub tool_network: Option<PolicyDecision>,
    #[serde(default)]
    pub out_of_workspace: Option<PolicyDecision>,
    #[serde(default)]
    pub shell_interpreters: Option<PolicyDecision>,
    #[serde(default)]
    pub approval_ttl_seconds: Option<u64>,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            approval_reviewer: default_approval_reviewer(),
            tool_network: default_ask(),
            out_of_workspace: default_deny(),
            shell_interpreters: default_deny(),
            approval_ttl_seconds: default_approval_ttl(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    #[serde(default = "default_max_minutes")]
    pub max_minutes: u32,
    #[serde(default = "default_max_same_failure")]
    pub max_same_failure: u32,
    #[serde(default = "default_max_observation_bytes")]
    pub max_observation_bytes: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            max_minutes: default_max_minutes(),
            max_same_failure: default_max_same_failure(),
            max_observation_bytes: default_max_observation_bytes(),
        }
    }
}

/// Optional project/CLI budget overrides.  A merge accepts only values no
/// larger than the user budget.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LimitsOverrides {
    #[serde(default)]
    pub max_steps: Option<u32>,
    #[serde(default)]
    pub max_minutes: Option<u32>,
    #[serde(default)]
    pub max_same_failure: Option<u32>,
    #[serde(default)]
    pub max_observation_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidatorConfig {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TuiConfig {
    #[serde(default = "default_status_line")]
    pub status_line: Vec<String>,
    #[serde(default)]
    pub status_line_use_colors: bool,
    #[serde(default)]
    pub model_availability_nux: BTreeMap<String, u32>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            status_line: default_status_line(),
            status_line_use_colors: false,
            model_availability_nux: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ProjectTrustConfig {
    #[serde(default)]
    pub trust_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct WindowsConfig {
    #[serde(default)]
    pub sandbox: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct NoticeConfig {
    #[serde(default)]
    pub hide_full_access_warning: bool,
    #[serde(default)]
    pub hide_rate_limit_model_nudge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// A redacted, serializable view of user configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct RedactedConfig {
    value: Value,
}

impl RedactedConfig {
    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn json(&self) -> String {
        serde_json::to_string_pretty(&self.value).unwrap_or_else(|_| "{}".into())
    }
}

fn serialize_env<S>(env: &BTreeMap<String, String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = serializer.serialize_map(Some(env.len()))?;
    for (name, value) in env {
        map.serialize_entry(name, redact_sensitive_literal(value))?;
    }
    map.end()
}

fn serialize_provider_api_key<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(redact_sensitive_literal(value)),
        None => serializer.serialize_none(),
    }
}

fn redact_sensitive_literal(value: &str) -> &str {
    if SecretReference::parse(value).ok().flatten().is_some() {
        value
    } else {
        "<redacted>"
    }
}

fn fmt_redacted_debug<T: Serialize>(
    name: &str,
    value: &T,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let serialized =
        serde_json::to_value(value).unwrap_or_else(|_| Value::String("<redacted>".to_owned()));
    formatter
        .debug_tuple(name)
        .field(&redact_debug_value(serialized))
        .finish()
}

/// Filesystem permission finding returned by `config doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDiagnostic {
    pub path: PathBuf,
    pub secure: bool,
    pub expected: String,
    pub actual: Option<String>,
    pub message: String,
}

impl PermissionDiagnostic {
    pub fn is_ok(&self) -> bool {
        self.secure
    }
}

/// Loads inline JSONC in tests and user/project files in production.
#[derive(Debug, Clone)]
pub struct ConfigLoader {
    user_path: PathBuf,
    project_path: Option<PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Result<Self, ConfigError> {
        Self::from_home_dir(home_dir())
    }

    fn from_home_dir(home: Option<PathBuf>) -> Result<Self, ConfigError> {
        let home = home.ok_or(ConfigError::HomeDirectoryUnavailable)?;
        if home.as_os_str().is_empty() || !home.is_absolute() {
            return Err(ConfigError::HomeDirectoryUnavailable);
        }
        Ok(Self {
            user_path: home.join(USER_CONFIG),
            project_path: None,
        })
    }

    /// Constructor used by unit/integration tests; no filesystem lookup is
    /// performed by the inline loaders.
    pub fn test() -> Self {
        Self {
            user_path: PathBuf::new(),
            project_path: None,
        }
    }

    pub fn with_project_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.project_path = Some(path.into());
        self
    }

    /// Override the user configuration path. This is primarily useful for
    /// deterministic embedding and integration tests.
    pub fn with_user_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.user_path = path.into();
        self
    }

    pub fn user_path(&self) -> &Path {
        &self.user_path
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    pub fn load_user(&self, source: &str) -> Result<UserConfig, ConfigError> {
        self.load_user_value(parse_jsonc(source)?, CredentialVault::default())
    }

    fn load_user_value(
        &self,
        value: Value,
        credential_vault: CredentialVault,
    ) -> Result<UserConfig, ConfigError> {
        validate_user_value(&value, &credential_vault)?;
        let mut config: UserConfig = serde_json::from_value(value).map_err(|_| {
            ConfigError::Parse("configuration values do not match the expected schema".into())
        })?;
        config.credential_vault = credential_vault;
        if let Some(reviewer) = config.approvals_reviewer.take() {
            if reviewer.trim().is_empty() {
                return Err(ConfigError::Validation {
                    path: "approvals_reviewer".into(),
                    message: "reviewer cannot be empty".into(),
                });
            }
            config.governance.approval_reviewer = reviewer;
        }
        config.validate()?;
        Ok(config)
    }

    pub fn load_project(&self, source: &str) -> Result<ProjectConfig, ConfigError> {
        let value = parse_jsonc(source)?;
        if let Some(path) = find_forbidden_project_field(&value, "") {
            return Err(ConfigError::ForbiddenProjectField { path });
        }
        serde_json::from_value(value).map_err(|error| ConfigError::Parse(error.to_string()))
    }

    pub fn load_user_file(&self, path: impl AsRef<Path>) -> Result<UserConfig, ConfigError> {
        let path = path.as_ref();
        require_user_permissions(path)?;
        let mut source = protected_file::read_protected_file(path)?;
        let mut value = parse_jsonc(&source)?;
        let credential_vault = extract_protected_credentials(&mut value)?;
        source.zeroize();
        self.load_user_value(value, credential_vault)
    }

    pub fn load_project_file(&self, path: impl AsRef<Path>) -> Result<ProjectConfig, ConfigError> {
        self.load_project(&fs::read_to_string(path)?)
    }

    pub fn load_user_path(&self) -> Result<UserConfig, ConfigError> {
        if path_entry_exists(&self.user_path)? {
            self.load_user_file(&self.user_path)
        } else {
            Ok(UserConfig::default())
        }
    }

    pub fn load_project_path(&self) -> Result<Option<ProjectConfig>, ConfigError> {
        let Some(path) = self.project_path.as_ref() else {
            return Ok(None);
        };
        self.load_project_file_if_exists(path)
    }

    /// Load the user-owned configuration and merge an optional workspace
    /// layer. Model transport validation remains an explicit caller action.
    pub fn load_effective(&self, workspace: impl AsRef<Path>) -> Result<UserConfig, ConfigError> {
        let user = self.load_user_path()?;
        let project_path = self
            .project_path
            .clone()
            .unwrap_or_else(|| workspace.as_ref().join(PROJECT_CONFIG));
        let effective = if let Some(project) = self.load_project_file_if_exists(&project_path)? {
            self.merge_project(&user, &project)?
        } else {
            user
        };
        Ok(effective)
    }

    fn load_project_file_if_exists(
        &self,
        path: &Path,
    ) -> Result<Option<ProjectConfig>, ConfigError> {
        if path_entry_exists(path)? {
            self.load_project_file(path).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Apply a validated project configuration over a user configuration.
    /// Ordinary selections override user defaults, while governance, budgets,
    /// provider names, and validators are constrained by the user-owned set.
    pub fn merge_project(
        &self,
        user: &UserConfig,
        project: &ProjectConfig,
    ) -> Result<UserConfig, ConfigError> {
        let mut merged = user.clone();

        if let Some(provider) = project.model_provider.as_ref() {
            if !user.model_providers.contains_key(provider) {
                return Err(ConfigError::Validation {
                    path: "model_provider".into(),
                    message: "project selected an unknown user provider profile".into(),
                });
            }
            merged.model_provider = Some(provider.clone());
        }
        if let Some(model) = project.model.as_ref() {
            merged.model = Some(model.clone());
        }
        if let Some(model) = project.review_model.as_ref() {
            merged.review_model = Some(model.clone());
        }
        if let Some(effort) = project.model_reasoning_effort.as_ref() {
            merged.model_reasoning_effort = Some(effort.clone());
        }
        if let Some(tier) = project.service_tier.as_ref() {
            merged.service_tier = Some(tier.clone());
        }

        if let Some(overrides) = project.governance.as_ref() {
            if overrides.approval_reviewer.is_some() {
                return Err(ConfigError::ForbiddenProjectField {
                    path: "governance.approval_reviewer".into(),
                });
            }
            merged.governance.tool_network = merge_security(
                user.governance.tool_network,
                None,
                overrides.tool_network,
                None,
            )?;
            merged.governance.out_of_workspace = merge_security(
                user.governance.out_of_workspace,
                None,
                overrides.out_of_workspace,
                None,
            )?;
            merged.governance.shell_interpreters = merge_security(
                user.governance.shell_interpreters,
                None,
                overrides.shell_interpreters,
                None,
            )?;
            if let Some(ttl) = overrides.approval_ttl_seconds {
                if ttl > user.governance.approval_ttl_seconds {
                    return Err(ConfigError::SecurityRelaxation);
                }
                merged.governance.approval_ttl_seconds = ttl;
            }
        }

        if let Some(limits) = project.limits.as_ref() {
            merged.limits.max_steps = merge_limit(user.limits.max_steps, limits.max_steps)?;
            merged.limits.max_minutes = merge_limit(user.limits.max_minutes, limits.max_minutes)?;
            merged.limits.max_same_failure =
                merge_limit(user.limits.max_same_failure, limits.max_same_failure)?;
            merged.limits.max_observation_bytes = merge_limit(
                user.limits.max_observation_bytes,
                limits.max_observation_bytes,
            )?;
        }

        if !project.validators.is_empty() {
            let mut selected = Vec::with_capacity(project.validators.len());
            for id in &project.validators {
                let validator = user
                    .validators
                    .iter()
                    .find(|item| item.id == *id)
                    .ok_or_else(|| ConfigError::Validation {
                        path: format!("validators.{id}"),
                        message: "project selected an unknown user validator".into(),
                    })?;
                selected.push(validator.clone());
            }
            merged.validators = selected;
        }

        if let Some(tui) = project.tui.as_ref() {
            merged.tui = tui.clone();
        }
        Ok(merged)
    }

    /// Run platform-specific configuration/state permission checks.
    pub fn doctor(&self) -> Vec<PermissionDiagnostic> {
        let mut paths = Vec::new();
        if !self.user_path.as_os_str().is_empty() {
            if let Some(parent) = self.user_path.parent() {
                paths.push(parent.to_path_buf());
            }
            paths.push(self.user_path.clone());
        }
        paths.into_iter().flat_map(check_permissions).collect()
    }
}

/// Merge one security decision from each configuration layer.  Project and
/// CLI values may only move the user ceiling toward `Deny`.
pub fn merge_security(
    default: PolicyDecision,
    user: Option<PolicyDecision>,
    project: Option<PolicyDecision>,
    cli: Option<PolicyDecision>,
) -> Result<PolicyDecision, ConfigError> {
    let ceiling = user.unwrap_or(default);
    for candidate in [project, cli].into_iter().flatten() {
        if candidate < ceiling {
            return Err(ConfigError::SecurityRelaxation);
        }
    }
    Ok([project, cli]
        .into_iter()
        .flatten()
        .fold(ceiling, PolicyDecision::max))
}

fn merge_limit<T>(user: T, project: Option<T>) -> Result<T, ConfigError>
where
    T: Copy + Ord,
{
    match project {
        Some(candidate) if candidate > user => Err(ConfigError::SecurityRelaxation),
        Some(candidate) => Ok(candidate),
        None => Ok(user),
    }
}

/// Check a directory/file using the platform's user-only permission model.
pub fn check_permissions(path: impl AsRef<Path>) -> Vec<PermissionDiagnostic> {
    let path = path.as_ref().to_path_buf();
    if !path.exists() {
        return vec![PermissionDiagnostic {
            path,
            secure: false,
            expected: "present with user-only permissions".into(),
            actual: None,
            message: "path does not exist yet".into(),
        }];
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                return vec![PermissionDiagnostic {
                    path,
                    secure: false,
                    expected: "readable metadata".into(),
                    actual: None,
                    message: format!("cannot inspect permissions: {error}"),
                }]
            }
        };
        let mode = metadata.permissions().mode() & 0o777;
        let expected = if metadata.is_dir() { 0o700 } else { 0o600 };
        vec![PermissionDiagnostic {
            path,
            secure: mode == expected,
            expected: format!("{expected:03o}"),
            actual: Some(format!("{mode:03o}")),
            message: if mode == expected {
                "permissions are user-only".into()
            } else {
                "restrict the path to the current user before storing configuration or state".into()
            },
        }]
    }

    #[cfg(windows)]
    {
        check_windows_acl(path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        vec![PermissionDiagnostic {
            path,
            secure: false,
            expected: "platform ACL inspection".into(),
            actual: None,
            message: "permission inspection is unavailable on this platform".into(),
        }]
    }
}

/// Enforce user-only modes on Unix before loading a user configuration.  On
/// Windows the loader remains read-only and `doctor` reports ACL ownership and
/// grants, because automatically rewriting an ACL owned by another principal
/// would be unsafe.
pub fn require_user_permissions(path: impl AsRef<Path>) -> Result<(), ConfigError> {
    #[cfg(unix)]
    {
        let path = path.as_ref();
        let mut paths = Vec::new();
        if let Some(parent) = path.parent() {
            paths.push(parent);
        }
        paths.push(path);
        for candidate in paths {
            for finding in check_permissions(candidate) {
                if !finding.secure {
                    return Err(ConfigError::InsecurePermissions {
                        path: finding.path,
                        expected: finding.expected,
                        actual: finding.actual.unwrap_or_else(|| "unavailable".into()),
                    });
                }
            }
        }
    }
    #[cfg(windows)]
    {
        let path = path.as_ref();
        let mut paths = Vec::new();
        if let Some(parent) = path.parent() {
            paths.push(parent);
        }
        paths.push(path);
        for candidate in paths {
            for finding in check_permissions(candidate) {
                if !finding.secure {
                    return Err(ConfigError::InsecurePermissions {
                        path: finding.path,
                        expected: finding.expected,
                        actual: finding.actual.unwrap_or_else(|| "unavailable".into()),
                    });
                }
            }
        }
    }
    #[cfg(not(any(unix, windows)))]
    let _ = path;
    Ok(())
}

#[cfg(windows)]
fn check_windows_acl(path: PathBuf) -> Vec<PermissionDiagnostic> {
    fn system_tool(name: &str) -> Option<PathBuf> {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|root| root.join("System32").join(format!("{name}.exe")))
            .filter(|candidate| candidate.is_file())
    }

    use std::process::Command;
    let acl_output = system_tool("icacls")
        .map(|tool| Command::new(tool).arg(&path).output())
        .transpose()
        .ok()
        .flatten();
    let Some(acl_output) = acl_output else {
        return vec![PermissionDiagnostic {
            path,
            secure: false,
            expected: "owner=current user; no broad write grant".into(),
            actual: None,
            message: "icacls is unavailable; inspect the owner and grants manually".into(),
        }];
    };

    let acl = String::from_utf8_lossy(&acl_output.stdout);
    let broad_write = acl.lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        let broad_principal = lower.contains("everyone:")
            || lower.contains("builtin\\users:")
            || lower.contains("authenticated users:");
        let write_grant = ["(f)", "(m)", "(w)", "(wd)", "(ad)"]
            .into_iter()
            .any(|grant| lower.contains(grant));
        broad_principal && write_grant
    });

    let owner = system_tool("WindowsPowerShell\\v1.0\\powershell")
        .map(|tool| {
            Command::new(tool)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "(Get-Acl -LiteralPath $env:ORCHESTER_DOCTOR_PATH).Owner",
                ])
                .env("ORCHESTER_DOCTOR_PATH", &path)
                .output()
        })
        .transpose()
        .ok()
        .flatten()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|owner| !owner.is_empty());
    let identity = system_tool("whoami")
        .map(Command::new)
        .map(|mut command| command.output())
        .transpose()
        .ok()
        .flatten()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|identity| !identity.is_empty());
    let owner_matches = match (&owner, &identity) {
        (Some(owner), Some(identity)) => owner.eq_ignore_ascii_case(identity),
        _ => false,
    };

    // This is deliberately conservative: an ACL that cannot be confidently
    // recognized as user-only is reported as a doctor finding.
    let secure = acl_output.status.success() && owner_matches && !broad_write;
    let actual = format!(
        "owner={}; identity={}; broad_write={broad_write}",
        owner.as_deref().unwrap_or("unavailable"),
        identity.as_deref().unwrap_or("unavailable")
    );
    vec![PermissionDiagnostic {
        path,
        secure,
        expected: "owner=current user; no broad write grant".into(),
        actual: Some(actual),
        message: if secure {
            "ACL ownership and grants passed conservative checks".into()
        } else {
            "verify ownership and remove broad write grants with Get-Acl/icacls; Orchester will not rewrite another principal's ACL".into()
        },
    }]
}

fn parse_jsonc(source: &str) -> Result<Value, ConfigError> {
    json5::from_str(source).map_err(|error| match error {
        json5::Error::Message {
            location: Some(location),
            ..
        } => ConfigError::Parse(format!(
            "syntax error at line {}, column {}",
            location.line, location.column
        )),
        json5::Error::Message { .. } => ConfigError::Parse("invalid JSONC syntax".into()),
    })
}

#[derive(Clone)]
enum ConfigPathSegment {
    Key(String),
    Index(usize),
}

fn extract_protected_credentials(value: &mut Value) -> Result<CredentialVault, ConfigError> {
    let mut credentials = BTreeMap::new();
    let Value::Object(root) = value else {
        return Ok(CredentialVault::default());
    };

    if let Some(Value::Object(env)) = root.get_mut("env") {
        for (name, value) in env {
            let path = [
                ConfigPathSegment::Key("env".to_owned()),
                ConfigPathSegment::Key(name.clone()),
            ];
            extract_protected_literal(value, &path, &mut credentials)?;
        }
    }

    if let Some(Value::Object(providers)) = root.get_mut("model_providers") {
        for (provider, config) in providers {
            let Value::Object(config) = config else {
                continue;
            };
            let Some(api_key) = config.get_mut("api_key") else {
                continue;
            };
            let path = [
                ConfigPathSegment::Key("model_providers".to_owned()),
                ConfigPathSegment::Key(provider.clone()),
                ConfigPathSegment::Key("api_key".to_owned()),
            ];
            extract_protected_literal(api_key, &path, &mut credentials)?;
        }
    }

    Ok(CredentialVault::from_values(credentials))
}

fn extract_protected_literal(
    value: &mut Value,
    path: &[ConfigPathSegment],
    credentials: &mut BTreeMap<CredentialSlot, SecretString>,
) -> Result<(), ConfigError> {
    let Value::String(value) = value else {
        return Ok(());
    };
    let reference =
        SecretReference::parse(value).map_err(|_| ConfigError::InvalidSecretReference {
            path: format_config_path(path),
        })?;
    if reference.is_some() {
        return Ok(());
    }
    if let Some(slot) = credential_slot_for_path(path) {
        let literal = std::mem::replace(value, PROTECTED_CREDENTIAL_MARKER.to_owned());
        credentials.insert(slot, SecretString::new(literal.into_boxed_str()));
    }
    Ok(())
}

fn validate_user_value(value: &Value, vault: &CredentialVault) -> Result<(), ConfigError> {
    validate_plaintext_strings(value, &mut Vec::new(), vault)
}

fn validate_plaintext_strings(
    value: &Value,
    path: &mut Vec<ConfigPathSegment>,
    vault: &CredentialVault,
) -> Result<(), ConfigError> {
    match value {
        Value::Object(values) => {
            for (key, child) in values {
                path.push(ConfigPathSegment::Key(key.clone()));
                if is_sensitive_name(key) {
                    if let Value::String(text) = child {
                        let parsed = SecretReference::parse(text).map_err(|_| {
                            ConfigError::InvalidSecretReference {
                                path: format_config_path(path),
                            }
                        })?;
                        if parsed.is_none() && !vault_binds_path(vault, path, text) {
                            return Err(ConfigError::PlaintextSecret {
                                path: format_config_path(path),
                            });
                        }
                    }
                }
                validate_plaintext_strings(child, path, vault)?;
                path.pop();
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                path.push(ConfigPathSegment::Index(index));
                validate_plaintext_strings(child, path, vault)?;
                path.pop();
            }
        }
        Value::String(text) if looks_like_secret(text) && !vault_binds_path(vault, path, text) => {
            return Err(ConfigError::PlaintextSecret {
                path: format_config_path(path),
            });
        }
        _ => {}
    }
    Ok(())
}

fn credential_slot_for_path(path: &[ConfigPathSegment]) -> Option<CredentialSlot> {
    match path {
        [ConfigPathSegment::Key(root), ConfigPathSegment::Key(name)] if root == "env" => {
            Some(CredentialSlot::Environment(name.clone()))
        }
        [ConfigPathSegment::Key(root), ConfigPathSegment::Key(provider), ConfigPathSegment::Key(field)]
            if root == "model_providers" && field == "api_key" =>
        {
            Some(CredentialSlot::ProviderApiKey(provider.clone()))
        }
        _ => None,
    }
}

fn vault_binds_path(vault: &CredentialVault, path: &[ConfigPathSegment], value: &str) -> bool {
    credential_slot_for_path(path).is_some_and(|slot| vault.binds_marker(&slot, value))
}

fn format_config_path(path: &[ConfigPathSegment]) -> String {
    if path.is_empty() {
        return "<root>".to_owned();
    }
    let mut rendered = String::new();
    for segment in path {
        match segment {
            ConfigPathSegment::Key(key) => {
                if !rendered.is_empty() {
                    rendered.push('.');
                }
                rendered.push_str(key);
            }
            ConfigPathSegment::Index(index) => rendered.push_str(&format!("[{index}]")),
        }
    }
    rendered
}

fn validate_reference_syntax(value: &str, path: &str) -> Result<(), ConfigError> {
    if value.starts_with("${") && SecretReference::parse(value).is_err() {
        return Err(ConfigError::InvalidSecretReference {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn find_forbidden_project_field(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(values) => {
            for (key, child) in values {
                let child_path = join_path(path, key);
                if is_forbidden_project_key(key) {
                    return Some(child_path);
                }
                if let Some(found) = find_forbidden_project_field(child, &child_path) {
                    return Some(found);
                }
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                if let Some(found) =
                    find_forbidden_project_field(child, &format!("{path}[{index}]"))
                {
                    return Some(found);
                }
            }
        }
        Value::String(text) if looks_like_secret(text) || text.starts_with("${secret:") => {
            return Some(if path.is_empty() {
                "<root>".into()
            } else {
                path.into()
            });
        }
        _ => {}
    }
    None
}

fn is_sensitive_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.contains("apikey")
        || normalized == "key"
        || normalized.ends_with("key")
        || normalized.ends_with("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("credential")
        || normalized.contains("authorization")
        || normalized.contains("auth")
}

fn is_forbidden_project_key(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    is_sensitive_name(name)
        || matches!(
            normalized.as_str(),
            "baseurl"
                | "apiurl"
                | "endpoint"
                | "proxyurl"
                | "providerurl"
                | "trustlevel"
                | "approvalreviewer"
                | "approvalsreviewer"
                | "policypath"
                | "credentialsource"
        )
}

fn looks_like_secret(value: &str) -> bool {
    let prefixes = [
        "sk-",
        "sk_",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "Bearer ",
        "-----BEGIN PRIVATE KEY-----",
    ];
    prefixes
        .iter()
        .any(|prefix| value.starts_with(prefix) && value.len() > prefix.len() + 4)
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(name, child)| {
                    let redacted = if is_sensitive_name(&name) {
                        match child {
                            Value::String(text)
                                if SecretReference::parse(&text).ok().flatten().is_some() =>
                            {
                                Value::String(text)
                            }
                            _ => Value::String("<redacted>".into()),
                        }
                    } else {
                        redact_value(child)
                    };
                    (name, redacted)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        Value::String(text) if looks_like_secret(&text) => Value::String("<redacted>".into()),
        other => other,
    }
}

fn redact_debug_value(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(name, child)| {
                    let normalized = name
                        .chars()
                        .filter(|ch| ch.is_ascii_alphanumeric())
                        .collect::<String>()
                        .to_ascii_lowercase();
                    let child = if is_sensitive_name(&name) || normalized == "baseurl" {
                        match child {
                            Value::String(_) => Value::String("[REDACTED]".to_owned()),
                            other => redact_debug_value(other),
                        }
                    } else {
                        redact_debug_value(child)
                    };
                    (name, child)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_debug_value).collect()),
        Value::String(text) if looks_like_secret(&text) => Value::String("[REDACTED]".to_owned()),
        other => other,
    }
}

fn redact_value_with_credentials<S: CredentialStore + ?Sized>(
    value: Value,
    config: &UserConfig,
    store: &S,
    path: &mut Vec<ConfigPathSegment>,
) -> Result<Value, ConfigError> {
    match value {
        Value::Object(values) => {
            let mut redacted = serde_json::Map::new();
            for (name, child) in values {
                path.push(ConfigPathSegment::Key(name.clone()));
                let child = if credential_slot_for_path(path).is_some() {
                    redact_credential_value(child, config, store, path)?
                } else if is_sensitive_name(&name) && child.is_string() {
                    match child {
                        Value::String(reference)
                            if SecretReference::parse(&reference).ok().flatten().is_some() =>
                        {
                            serde_json::json!({
                                "source": reference,
                                "present": config.secret_reference_present(&reference, store)?,
                            })
                        }
                        _ => serde_json::json!({
                            "source": "<redacted>",
                            "present": false,
                        }),
                    }
                } else {
                    redact_value_with_credentials(child, config, store, path)?
                };
                path.pop();
                redacted.insert(name, child);
            }
            Ok(Value::Object(redacted))
        }
        Value::Array(values) => {
            let mut redacted = Vec::with_capacity(values.len());
            for (index, child) in values.into_iter().enumerate() {
                path.push(ConfigPathSegment::Index(index));
                redacted.push(redact_value_with_credentials(child, config, store, path)?);
                path.pop();
            }
            Ok(Value::Array(redacted))
        }
        Value::String(text) if looks_like_secret(&text) => Ok(Value::String("<redacted>".into())),
        other => Ok(other),
    }
}

fn redact_credential_value<S: CredentialStore + ?Sized>(
    value: Value,
    config: &UserConfig,
    store: &S,
    path: &[ConfigPathSegment],
) -> Result<Value, ConfigError> {
    match value {
        Value::String(marker) if vault_binds_path(&config.credential_vault, path, &marker) => {
            Ok(serde_json::json!({
                "source": "protected-user-file",
                "present": true,
            }))
        }
        Value::String(reference) if SecretReference::parse(&reference).ok().flatten().is_some() => {
            Ok(serde_json::json!({
                "source": reference,
                "present": config.secret_reference_present(&reference, store)?,
            }))
        }
        _ => Ok(serde_json::json!({
            "source": "<redacted>",
            "present": false,
        })),
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}.{child}")
    }
}

fn path_entry_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn default_version() -> u32 {
    1
}

fn default_approval_reviewer() -> String {
    "user".into()
}

fn default_ask() -> PolicyDecision {
    PolicyDecision::Ask
}

fn default_deny() -> PolicyDecision {
    PolicyDecision::Deny
}

fn default_approval_ttl() -> u64 {
    86_400
}

fn default_max_steps() -> u32 {
    80
}

fn default_max_minutes() -> u32 {
    30
}

fn default_max_same_failure() -> u32 {
    3
}

fn default_max_observation_bytes() -> usize {
    65_536
}

fn default_status_line() -> Vec<String> {
    [
        "current-dir",
        "model",
        "reasoning",
        "permissions",
        "task-progress",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod config_loader_tests {
    use super::*;

    #[test]
    fn home_directory_resolution_fails_closed() {
        for home in [None, Some(PathBuf::new()), Some(PathBuf::from("relative"))] {
            assert!(matches!(
                ConfigLoader::from_home_dir(home),
                Err(ConfigError::HomeDirectoryUnavailable)
            ));
        }
    }

    #[test]
    fn home_directory_resolution_builds_the_user_path() {
        #[cfg(windows)]
        let home = PathBuf::from(r"C:\Users\example");
        #[cfg(not(windows))]
        let home = PathBuf::from("/home/example");

        let loader = ConfigLoader::from_home_dir(Some(home.clone())).unwrap();

        assert_eq!(loader.user_path(), home.join(USER_CONFIG));
        assert_eq!(loader.project_path(), None);
    }
}
