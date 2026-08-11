//! Guided provider credential entry for `/login` and `/logout`.
//!
//! The secret itself only ever travels as a [`SecretString`] into a
//! [`CredentialStore`].  Nothing here writes, logs, or serializes plaintext:
//! callers receive a masked tail for confirmation and the config *reference*
//! that makes the stored value reachable.

use std::io;
use std::path::Path;

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use crate::harness::config::{SecretReference, UserConfig};
use crate::harness::credentials::{CredentialError, CredentialStore, KEYRING_SERVICE};
use crate::harness::private_fs::{create_private_dir_all, write_private_file};

/// Characters of the secret revealed for confirmation.  Four is enough for a
/// human to recognize a key they just pasted and far too few to reconstruct.
const VISIBLE_TAIL: usize = 4;

/// The shortest secret whose tail may be shown.  Below this a leaked tail is a
/// meaningful fraction of the whole value.
const MIN_MASKABLE_LEN: usize = 8;

#[derive(Debug, Error)]
pub enum CredentialEntryError {
    #[error("no model provider is active; name one explicitly")]
    NoActiveProvider,
    #[error("the entered API key is empty")]
    EmptySecret,
    #[error(transparent)]
    Store(#[from] CredentialError),
    #[error("could not write the user configuration: {0}")]
    Config(#[from] io::Error),
}

/// What `/login` is about to act on, resolved before the key is requested so
/// the prompt can name the provider and its endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialTarget {
    pub provider: String,
    pub base_url: Option<String>,
    /// The provider has a `model_providers` block.
    pub configured: bool,
    /// That block's `api_key` is a `${secret:…}`/`${env:…}` reference rather
    /// than a literal, so a stored credential is actually reachable.
    pub referenced: bool,
    /// The credential store already holds a value for this provider.
    pub present: bool,
    /// The reference the config needs in order to reach the stored value.
    pub reference: String,
}

/// The confirmation returned after a successful store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialUpdate {
    pub provider: String,
    pub masked: String,
    pub reference: String,
}

/// Describe the provider `/login` or `/logout` should act on.  `provider`
/// falls back to the config's active `model_provider`.
pub fn resolve_credential_target<S: CredentialStore + ?Sized>(
    config: &UserConfig,
    store: &S,
    provider: Option<&str>,
) -> Result<CredentialTarget, CredentialEntryError> {
    let provider = provider
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| config.model_provider.clone())
        .ok_or(CredentialEntryError::NoActiveProvider)?;

    let configured = config.model_providers().get(&provider);
    // A missing store entry is a normal pre-login state, but a backend that
    // cannot answer is not: surface it instead of reporting "absent".
    let present = store.present(&provider)?;

    Ok(CredentialTarget {
        reference: provider_reference(&provider),
        base_url: configured.and_then(|entry| entry.base_url.clone()),
        referenced: configured
            .and_then(|entry| entry.api_key.as_deref())
            .is_some_and(|value| matches!(SecretReference::parse(value), Ok(Some(_)))),
        configured: configured.is_some(),
        present,
        provider,
    })
}

/// Hand a freshly entered key to the credential store.  No provider request is
/// made, so the credential is stored but unverified.
pub fn store_provider_credential<S: CredentialStore + ?Sized>(
    store: &S,
    provider: &str,
    secret: SecretString,
) -> Result<CredentialUpdate, CredentialEntryError> {
    let masked = {
        let value = secret.expose_secret().trim();
        if value.is_empty() {
            return Err(CredentialEntryError::EmptySecret);
        }
        mask_tail(value)
    };
    store.set(provider, secret)?;
    Ok(CredentialUpdate {
        provider: provider.to_owned(),
        masked,
        reference: provider_reference(provider),
    })
}

/// Forget a stored provider key.  Returns whether one was actually present.
pub fn clear_provider_credential<S: CredentialStore + ?Sized>(
    store: &S,
    provider: &str,
) -> Result<bool, CredentialEntryError> {
    let present = store.present(provider)?;
    store.clear(provider)?;
    Ok(present)
}

/// What `/login` did (or could not do) to make the stored secret reachable
/// from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWiring {
    /// No user config existed, so one was created from a template that already
    /// routes this provider at the stored secret.
    Created,
    /// The config already reaches a stored secret through a reference.
    AlreadyReferenced,
    /// A config exists but names no reference for this provider.  It is
    /// human-owned and may carry comments, so it is never rewritten in place;
    /// the caller shows `snippet` instead.
    NeedsReference { snippet: String },
}

/// Make the stored secret reachable from `config_path`, creating that file only
/// when it is absent.  An existing file is read but never written.
///
/// A created config is user-only from the moment it exists: the loader refuses
/// to read a configuration that anyone else could write, and this file may hold
/// a literal API key.
pub fn wire_provider_reference(
    config_path: &Path,
    target: &CredentialTarget,
) -> Result<ConfigWiring, CredentialEntryError> {
    if target.referenced {
        return Ok(ConfigWiring::AlreadyReferenced);
    }
    if config_path.exists() {
        return Ok(ConfigWiring::NeedsReference {
            snippet: reference_snippet(target),
        });
    }
    if let Some(parent) = config_path.parent() {
        create_private_dir_all(parent)?;
    }
    write_private_file(config_path, &initial_config(target))?;
    Ok(ConfigWiring::Created)
}

/// The `model_providers` block a human pastes into a config Orchester will not
/// touch.  Two spaces of indent match the template below.
fn reference_snippet(target: &CredentialTarget) -> String {
    let CredentialTarget {
        provider,
        reference,
        ..
    } = target;
    let base_url = target
        .base_url
        .as_ref()
        .map(|url| format!("\n      \"base_url\": {},", json_string(url)))
        .unwrap_or_default();
    format!(
        "\"model_provider\": {provider},\n\
         \"model_providers\": {{\n  \
           {provider}: {{{base_url}\n      \
             \"api_key\": {reference}\n  \
           }}\n\
         }}",
        provider = json_string(provider),
        reference = json_string(reference),
    )
}

/// The whole user config written on a first `/login`.  It is deliberately
/// commented: this file belongs to the human from here on.
fn initial_config(target: &CredentialTarget) -> String {
    let base_url = target
        .base_url
        .as_ref()
        .map(|url| format!("      \"base_url\": {},\n", json_string(url)))
        .unwrap_or_default();
    format!(
        "// Orchester user configuration.\n\
         // Created by `orchester login`.  Orchester never rewrites this file in\n\
         // place, so comments and formatting are yours to keep.\n\
         {{\n  \
           \"version\": 1,\n\n  \
           // The provider used when no model profile overrides it.\n  \
           \"model_provider\": {provider},\n\n  \
           \"model_providers\": {{\n    \
             {provider}: {{\n\
{base_url}      \
               // Resolved through the OS keyring (service \"{KEYRING_SERVICE}\").\n      \
               // Re-run `orchester login` to replace the stored key.\n      \
               \"api_key\": {reference}\n    \
             }}\n  \
           }}\n\
         }}\n",
        provider = json_string(&target.provider),
        reference = json_string(&target.reference),
    )
}

/// Quote a value as JSON so a provider name carrying `"` or a control
/// character cannot break out of the string and forge config structure.
fn json_string(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

fn provider_reference(provider: &str) -> String {
    SecretReference::Provider(provider.to_owned()).as_str()
}

fn mask_tail(value: &str) -> String {
    // Count characters, not bytes: a multi-byte key must not be sliced apart.
    let characters: Vec<char> = value.chars().collect();
    if characters.len() < MIN_MASKABLE_LEN {
        return "…".into();
    }
    let tail: String = characters[characters.len() - VISIBLE_TAIL..]
        .iter()
        .collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::config::ConfigLoader;
    use crate::harness::credentials::{CredentialStore, InMemoryCredentialStore};
    use secrecy::SecretString;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    /// A scratch directory that removes itself, so a failing assertion cannot
    /// leave a half-written config behind for the next run to read.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-credential-entry-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create scratch directory");
            Self(path)
        }

        fn join(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn config(source: &str) -> crate::harness::config::UserConfig {
        ConfigLoader::test().load_user(source).expect("load config")
    }

    fn secret(value: &str) -> SecretString {
        SecretString::new(value.to_owned().into_boxed_str())
    }

    fn target(referenced: bool) -> CredentialTarget {
        CredentialTarget {
            provider: "OpenAI".into(),
            base_url: Some("https://agentrouter.org".into()),
            configured: referenced,
            referenced,
            present: true,
            reference: "${secret:OpenAI}".into(),
        }
    }

    #[test]
    fn storing_a_secret_reaches_the_store_and_reports_a_masked_tail() {
        let store = InMemoryCredentialStore::default();

        let update = store_provider_credential(&store, "OpenAI", secret("sk-live-abcd9f2c"))
            .expect("store credential");

        assert_eq!(update.provider, "OpenAI");
        assert_eq!(update.reference, "${secret:OpenAI}");
        assert_eq!(update.masked, "…9f2c");
        assert!(store.present("OpenAI").expect("present"));
    }

    #[test]
    fn a_short_secret_is_masked_completely() {
        let store = InMemoryCredentialStore::default();

        let update =
            store_provider_credential(&store, "OpenAI", secret("abc")).expect("store credential");

        assert_eq!(update.masked, "…");
        assert!(!update.masked.contains("abc"));
    }

    #[test]
    fn an_empty_secret_is_rejected_before_it_reaches_the_store() {
        let store = InMemoryCredentialStore::default();

        let error = store_provider_credential(&store, "OpenAI", secret("   "))
            .expect_err("empty secret must be refused");

        assert!(matches!(error, CredentialEntryError::EmptySecret));
        assert!(!store.present("OpenAI").expect("present"));
    }

    #[test]
    fn clearing_reports_whether_a_secret_was_actually_removed() {
        let store = InMemoryCredentialStore::with("OpenAI", "sk-live-abcd9f2c");

        assert!(clear_provider_credential(&store, "OpenAI").expect("clear"));
        assert!(!clear_provider_credential(&store, "OpenAI").expect("clear again"));
    }

    #[test]
    fn the_target_defaults_to_the_active_provider_and_reports_its_wiring() {
        let config = config(
            r#"{
                "model_provider": "OpenAI",
                "model_providers": {
                    "OpenAI": {
                        "base_url": "https://agentrouter.org",
                        "api_key": "${secret:OpenAI}"
                    }
                }
            }"#,
        );
        let store = InMemoryCredentialStore::default();

        let target = resolve_credential_target(&config, &store, None).expect("resolve target");

        assert_eq!(target.provider, "OpenAI");
        assert_eq!(target.base_url.as_deref(), Some("https://agentrouter.org"));
        assert!(target.configured);
        assert!(target.referenced);
        // /login has not run yet, so the reference still points at nothing.
        assert!(!target.present);
    }

    #[test]
    fn a_named_provider_overrides_the_active_one_and_may_be_unconfigured() {
        let config = config(r#"{"model_provider": "OpenAI"}"#);
        let store = InMemoryCredentialStore::default();

        let target =
            resolve_credential_target(&config, &store, Some("Anthropic")).expect("resolve target");

        assert_eq!(target.provider, "Anthropic");
        assert!(!target.configured);
        assert!(!target.referenced);
        assert_eq!(target.reference, "${secret:Anthropic}");
    }

    #[test]
    fn a_provider_without_an_api_key_is_configured_but_unreferenced() {
        // The load layer already refuses a plaintext key in an unprotected
        // file, so "configured but not keyring-backed" is the state /login
        // actually has to repair.
        let config = config(
            r#"{
                "model_provider": "OpenAI",
                "model_providers": { "OpenAI": { "base_url": "https://agentrouter.org" } }
            }"#,
        );
        let store = InMemoryCredentialStore::default();

        let target = resolve_credential_target(&config, &store, None).expect("resolve target");

        assert!(target.configured);
        assert!(!target.referenced);
    }

    #[test]
    fn an_environment_indirection_counts_as_a_reference() {
        let config = config(
            r#"{
                "model_provider": "OpenAI",
                "env": { "ROUTER_KEY": "${secret:OpenAI}" },
                "model_providers": { "OpenAI": { "api_key": "${env:ROUTER_KEY}" } }
            }"#,
        );
        let store = InMemoryCredentialStore::default();

        let target = resolve_credential_target(&config, &store, None).expect("resolve target");

        assert!(target.referenced);
    }

    #[test]
    fn a_config_without_an_active_provider_needs_an_explicit_name() {
        let config = config(r#"{"version": 1}"#);
        let store = InMemoryCredentialStore::default();

        let error = resolve_credential_target(&config, &store, None)
            .expect_err("no provider can be resolved");

        assert!(matches!(error, CredentialEntryError::NoActiveProvider));
    }

    #[test]
    fn a_missing_config_is_created_already_referencing_the_stored_secret() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");

        let wiring = wire_provider_reference(&path, &target(false)).expect("wire reference");

        assert_eq!(wiring, ConfigWiring::Created);
        // The round trip is the real assertion: the template must survive the
        // JSONC loader and come back as a reachable reference, not just as
        // text that happens to contain the right characters.
        let written = fs::read_to_string(&path).expect("read created config");
        let reloaded = config(&written);
        let store = InMemoryCredentialStore::default();
        let target = resolve_credential_target(&reloaded, &store, None).expect("resolve target");

        assert_eq!(target.provider, "OpenAI");
        assert!(target.configured);
        assert!(target.referenced);
    }

    #[test]
    fn a_created_config_keeps_the_provider_endpoint_it_was_given() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");

        wire_provider_reference(&path, &target(false)).expect("wire reference");

        let written = fs::read_to_string(&path).expect("read created config");
        let reloaded = config(&written);
        let store = InMemoryCredentialStore::default();
        let target = resolve_credential_target(&reloaded, &store, None).expect("resolve target");

        assert_eq!(target.base_url.as_deref(), Some("https://agentrouter.org"));
    }

    #[test]
    fn a_created_config_is_private_enough_for_the_loader_that_must_read_it_back() {
        // The file may hold a literal API key, so the loader refuses to read it
        // unless it is user-only.  A `/login` that writes a config Orchester
        // cannot then load leaves every later command failing on a path we
        // created ourselves.
        let root = TempDir::new();
        // Creating under a permissive directory is the whole point: a real home
        // or drive root hands down broad grants, and privacy has to be
        // established rather than inherited.
        grant_broad_access(&root.0);
        let path = root.join(".orchester").join("orchester.jsonc");

        wire_provider_reference(&path, &target(false)).expect("wire reference");

        // Going through the loader exercises both gates the file must pass: the
        // reported permission check and the stricter handle validation that
        // guards the read itself.
        let loaded = ConfigLoader::test()
            .load_user_file(&path)
            .expect("a config we just created must satisfy our own privacy gates");

        assert_eq!(loaded.model_provider.as_deref(), Some("OpenAI"));
    }

    /// Widen a directory as far as the platform allows, so anything created
    /// beneath it inherits a grant the privacy gate rejects.
    #[cfg(windows)]
    fn grant_broad_access(path: &std::path::Path) {
        let tool = std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32")
            .join("icacls.exe");
        // S-1-5-11 is Authenticated Users; (OI)(CI) makes the grant inheritable
        // by the files and directories `/login` is about to create.
        let output = std::process::Command::new(tool)
            .arg(path)
            .args(["/grant", "*S-1-5-11:(OI)(CI)(M)"])
            .output()
            .expect("run icacls");
        assert!(output.status.success(), "seed a permissive directory");
    }

    #[cfg(unix)]
    fn grant_broad_access(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o777)).expect("widen directory");
    }

    #[test]
    fn an_already_referenced_config_is_left_exactly_as_the_human_wrote_it() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "// hand written\n{ \"model_provider\": \"OpenAI\" }\n";
        fs::write(&path, original).expect("seed config");

        let wiring = wire_provider_reference(&path, &target(true)).expect("wire reference");

        assert_eq!(wiring, ConfigWiring::AlreadyReferenced);
        assert_eq!(fs::read_to_string(&path).expect("read config"), original);
    }

    #[test]
    fn an_unreferenced_config_is_never_rewritten_and_yields_a_pasteable_snippet() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "// hand written, with comments worth keeping\n{ \"version\": 1 }\n";
        fs::write(&path, original).expect("seed config");

        let wiring = wire_provider_reference(&path, &target(false)).expect("wire reference");

        let ConfigWiring::NeedsReference { snippet } = wiring else {
            panic!("an existing config without a reference must yield a snippet");
        };
        assert!(snippet.contains("\"OpenAI\""));
        assert!(snippet.contains("${secret:OpenAI}"));
        // A human-owned file with comments is never edited in place.
        assert_eq!(fs::read_to_string(&path).expect("read config"), original);
    }
}
