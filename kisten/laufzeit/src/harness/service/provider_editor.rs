//! Writing one `model_providers` entry from an interactive form.
//!
//! This is the only path in the harness that changes the user's configuration,
//! and it is deliberately narrow: one provider entry, plus the two members that
//! make it the active choice. Everything else in `orchester.jsonc` — including
//! every comment the human wrote — is copied through by
//! [`ConfigLoader::edit_user_config`].
//!
//! No secret is ever written to the file. A key entered in the form goes to the
//! credential store and the file receives only the `${secret:…}` reference that
//! reaches it, so a configuration Orchester wrote can be shared or diffed
//! without leaking anything.

use std::path::PathBuf;

use secrecy::SecretString;
use thiserror::Error;

use super::credentials::{provider_reference, store_provider_credential, CredentialEntryError};
use crate::harness::config::{
    ConfigError, ConfigLoader, ConfigValue, ProviderConfig, UserConfig, ANTHROPIC_WIRE_API,
    RESPONSES_WIRE_API,
};
use crate::harness::credentials::CredentialStore;

/// The bound on every free-text field of a draft.
///
/// Generous for a URL or a model name and far below anything that would make a
/// configuration unreadable. The catalog applies the same bound when it projects
/// these values back for display.
const MAX_FIELD_BYTES: usize = 256;

/// A provider entry as an interactive form collected it.
///
/// Every field is raw text straight from the human, so nothing here is trusted:
/// [`write_self_agent_provider`] trims, bounds and resolves the whole entry
/// before anything is written. The API key is deliberately absent — it travels
/// separately as a [`SecretString`] and never becomes part of a struct that
/// could be logged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderDraft {
    /// The `model_providers` key, which is also what `/model provider` takes.
    pub provider: String,
    /// Display name. Blank falls back to the key.
    pub name: String,
    pub base_url: String,
    /// One of the two wire APIs the harness can build a transport for.
    pub wire_api: String,
    /// The model this provider should serve. Blank keeps the configured one.
    pub model: String,
    /// Also write this provider as the active one.
    pub activate: bool,
}

/// What writing a provider entry did, so the caller can confirm it without
/// re-reading the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEdit {
    pub provider: String,
    pub provider_name: String,
    pub model: String,
    pub wire_api: String,
    /// The provider is now the configured default, not only this session's.
    pub activated: bool,
    /// The indirection written in place of the key.
    pub reference: String,
    /// A key was handed to the credential store as part of this edit.
    pub credential_stored: bool,
    pub path: PathBuf,
    /// No configuration existed, so one was created around the entry.
    pub created: bool,
    /// Where the previous text was kept. Absent when there was none.
    pub backup: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum ProviderEditError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Credential(#[from] CredentialEntryError),
}

/// The wire APIs a form may offer, in the order a picker should list them.
///
/// Exported so the form cannot drift from the set resolution accepts: adding a
/// third wire is then one edit, not two that can disagree.
pub const PROVIDER_WIRE_APIS: [&str; 2] = [RESPONSES_WIRE_API, ANTHROPIC_WIRE_API];

/// Validate `draft` against `config`, store `secret` if one was entered, and
/// splice the provider entry into the user's configuration file.
///
/// The order matters. Validation happens first, so a rejected draft leaves
/// neither a key in the credential store nor a line in the file. The key is
/// stored next, so the reference the file is about to name already resolves by
/// the time anything can read it. A failure after that point leaves an
/// unreferenced key, which is inert and re-runnable — the opposite order would
/// leave a configuration pointing at nothing.
pub fn write_self_agent_provider<S: CredentialStore + ?Sized>(
    loader: &ConfigLoader,
    config: &UserConfig,
    store: &S,
    draft: &ProviderDraft,
    secret: Option<SecretString>,
) -> Result<ProviderEdit, ProviderEditError> {
    let draft = normalize(draft)?;
    let reference = provider_reference(&draft.provider);
    let entry = ProviderConfig {
        name: (!draft.name.is_empty()).then(|| draft.name.clone()),
        base_url: Some(draft.base_url.clone()),
        // Always a reference, never a value. A provider reached over the network
        // needs a key, and naming the indirection up front means a later
        // `/login` reaches this entry without the human editing the file again.
        api_key: Some(reference.clone()),
        wire_api: Some(draft.wire_api.clone()),
        requires_openai_auth: None,
    };
    // Resolving the candidate is the validation: it applies exactly the checks a
    // later load applies, and it names the field that failed.
    let resolved = config.resolve_provider_entry(
        &draft.provider,
        entry,
        (!draft.model.is_empty()).then_some(draft.model.as_str()),
    )?;

    let credential_stored = match secret {
        Some(secret) => {
            store_provider_credential(store, &draft.provider, secret)?;
            true
        }
        None => false,
    };

    let mut members = vec![(
        vec!["model_providers".to_owned(), draft.provider.clone()],
        ConfigValue::Object(entry_members(&draft, &reference)),
    )];
    if draft.activate {
        members.push((
            vec!["model_provider".to_owned()],
            ConfigValue::String(draft.provider.clone()),
        ));
        if !draft.model.is_empty() {
            members.push((
                vec!["model".to_owned()],
                ConfigValue::String(draft.model.clone()),
            ));
        }
    }
    let edit = loader.edit_user_config(&members)?;

    Ok(ProviderEdit {
        provider: resolved.provider,
        provider_name: resolved.provider_name,
        model: resolved.model,
        wire_api: resolved.wire_api,
        activated: draft.activate,
        reference,
        credential_stored,
        path: edit.path,
        created: edit.created,
        backup: edit.backup,
    })
}

/// The members written for the entry itself, in the order a human reads them:
/// what it is called, where it is, how it is spoken to, and how it is
/// authenticated.
fn entry_members(draft: &ProviderDraft, reference: &str) -> Vec<(String, ConfigValue)> {
    let mut members = Vec::with_capacity(4);
    if !draft.name.is_empty() {
        members.push(("name".to_owned(), ConfigValue::String(draft.name.clone())));
    }
    members.push((
        "base_url".to_owned(),
        ConfigValue::String(draft.base_url.clone()),
    ));
    members.push((
        "wire_api".to_owned(),
        ConfigValue::String(draft.wire_api.clone()),
    ));
    members.push(("api_key".to_owned(), ConfigValue::String(reference.into())));
    members
}

/// Trim every field and bound the free-text ones.
///
/// The bounds are here rather than in the configuration layer because they are
/// about what a form may submit: resolution accepts any URL that parses, but a
/// field a human typed into a terminal has no business being longer than a line.
fn normalize(draft: &ProviderDraft) -> Result<ProviderDraft, ConfigError> {
    let provider = draft.provider.trim();
    let name = draft.name.trim();
    let base_url = draft.base_url.trim();
    let model = draft.model.trim();
    let wire_api = draft.wire_api.trim();

    for (path, value) in [
        ("model_providers", provider),
        ("model_providers.name", name),
        ("model_providers.base_url", base_url),
        ("model", model),
    ] {
        bounded(value, path)?;
    }
    if !matches!(wire_api, RESPONSES_WIRE_API | ANTHROPIC_WIRE_API) {
        return Err(ConfigError::Validation {
            path: "model_providers.wire_api".into(),
            message: "unsupported wire API; supported values are 'responses' and 'anthropic'"
                .into(),
        });
    }

    Ok(ProviderDraft {
        provider: provider.to_owned(),
        name: name.to_owned(),
        base_url: base_url.to_owned(),
        wire_api: wire_api.to_owned(),
        model: model.to_owned(),
        activate: draft.activate,
    })
}

fn bounded(value: &str, path: &str) -> Result<(), ConfigError> {
    if value.len() > MAX_FIELD_BYTES || value.chars().any(char::is_control) {
        return Err(ConfigError::Validation {
            path: path.into(),
            message: "field is too long or contains control characters".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::credentials::{CredentialStore, InMemoryCredentialStore};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-provider-editor-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create scratch directory");
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn draft() -> ProviderDraft {
        ProviderDraft {
            provider: "relay".into(),
            name: "Relay API".into(),
            base_url: "https://relay.test/v1".into(),
            wire_api: ANTHROPIC_WIRE_API.into(),
            model: "claude-opus-4-6".into(),
            activate: true,
        }
    }

    fn loader(path: &Path) -> ConfigLoader {
        ConfigLoader::test().with_user_path(path)
    }

    fn config(source: &str) -> UserConfig {
        ConfigLoader::test().load_user(source).expect("load config")
    }

    fn secret(value: &str) -> SecretString {
        SecretString::new(value.to_owned().into_boxed_str())
    }

    #[test]
    fn a_written_provider_loads_back_and_resolves_through_our_own_gates() {
        let root = TempDir::new();
        let path = root.join(".orchester").join("orchester.jsonc");
        let loader = loader(&path);
        let store = InMemoryCredentialStore::default();

        let edit = write_self_agent_provider(
            &loader,
            &config(r#"{"version": 1}"#),
            &store,
            &draft(),
            Some(secret("sk-relay-abcd9f2c")),
        )
        .expect("write the provider");

        assert!(edit.created);
        assert!(edit.activated);
        assert!(edit.credential_stored);
        assert_eq!(edit.reference, "${secret:relay}");
        assert_eq!(edit.provider_name, "Relay API");
        assert_eq!(edit.wire_api, ANTHROPIC_WIRE_API);
        assert!(store.present("relay").expect("stored key"));
        // Loading it back is the real assertion: a form that wrote an entry
        // Orchester then refuses would leave every later turn failing on a file
        // we wrote ourselves.
        let reloaded = loader.load_user_file(&path).expect("load written config");
        let resolved = reloaded
            .resolve_model_profile()
            .expect("the written provider resolves");
        assert_eq!(resolved.provider, "relay");
        assert_eq!(resolved.base_url, "https://relay.test/v1");
        assert_eq!(resolved.model, "claude-opus-4-6");
    }

    #[test]
    fn no_secret_ever_reaches_the_configuration_file() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let store = InMemoryCredentialStore::default();

        write_self_agent_provider(
            &loader(&path),
            &config(r#"{"version": 1}"#),
            &store,
            &draft(),
            Some(secret("sk-relay-abcd9f2c")),
        )
        .expect("write the provider");

        let written = fs::read_to_string(&path).expect("read written config");
        assert!(!written.contains("sk-relay-abcd9f2c"));
        assert!(written.contains("${secret:relay}"));
    }

    #[test]
    fn replacing_an_entry_keeps_the_rest_of_the_file_and_backs_it_up() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "// mine\n\
             {\n  \
               \"model_providers\": {\n    \
                 // straight to the vendor\n    \
                 \"direct\": { \"base_url\": \"https://api.anthropic.com\" },\n    \
                 \"relay\": { \"base_url\": \"https://old.test\" }\n  \
               }\n\
             }\n";
        fs::write(&path, original).expect("seed a configuration");
        let store = InMemoryCredentialStore::default();

        let edit =
            write_self_agent_provider(&loader(&path), &config(original), &store, &draft(), None)
                .expect("replace the provider");

        assert!(!edit.created);
        assert!(!edit.credential_stored);
        let written = fs::read_to_string(&path).expect("read written config");
        assert!(written.contains("// mine"));
        assert!(written.contains("// straight to the vendor"));
        assert!(written.contains("https://api.anthropic.com"));
        assert!(written.contains("https://relay.test/v1"));
        assert!(!written.contains("https://old.test"));
        let backup = edit.backup.expect("the previous version is preserved");
        assert_eq!(
            fs::read_to_string(&backup).expect("read the backup"),
            original
        );
    }

    #[test]
    fn a_draft_that_is_not_activated_leaves_the_active_choice_alone() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = r#"{
            "model_provider": "direct",
            "model": "claude-opus-4-6",
            "model_providers": { "direct": { "base_url": "https://api.anthropic.com" } }
        }"#;
        fs::write(&path, original).expect("seed a configuration");
        let store = InMemoryCredentialStore::default();
        let mut draft = draft();
        draft.activate = false;

        let edit =
            write_self_agent_provider(&loader(&path), &config(original), &store, &draft, None)
                .expect("add the provider");

        assert!(!edit.activated);
        let reloaded = loader(&path)
            .load_user_file(&path)
            .expect("load written config");
        assert_eq!(reloaded.model_provider.as_deref(), Some("direct"));
        // The entry still landed, so `/model provider relay` can switch to it.
        assert!(reloaded.model_providers().contains_key("relay"));
    }

    #[test]
    fn a_rejected_draft_writes_neither_a_file_nor_a_key() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let store = InMemoryCredentialStore::default();
        let mut draft = draft();
        draft.base_url = "http://relay.test/v1".into();

        let error = write_self_agent_provider(
            &loader(&path),
            &config(r#"{"version": 1}"#),
            &store,
            &draft,
            Some(secret("sk-relay-abcd9f2c")),
        )
        .expect_err("plain HTTP to a non-loopback host must be refused");

        // The field is named, so a form can put the cursor back on it.
        let ProviderEditError::Config(ConfigError::Validation { path: field, .. }) = error else {
            panic!("a bad base URL must be a field validation failure");
        };
        assert_eq!(field, "model_providers.relay.base_url");
        assert!(!path.exists());
        assert!(!store.present("relay").expect("no key was stored"));
    }

    #[test]
    fn an_unsupported_wire_is_refused_before_anything_is_touched() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let store = InMemoryCredentialStore::default();
        let mut draft = draft();
        draft.wire_api = "grpc".into();

        let error = write_self_agent_provider(
            &loader(&path),
            &config(r#"{"version": 1}"#),
            &store,
            &draft,
            None,
        )
        .expect_err("an unknown wire has no transport to build");

        let ProviderEditError::Config(ConfigError::Validation { path: field, .. }) = error else {
            panic!("an unknown wire must be a field validation failure");
        };
        assert_eq!(field, "model_providers.wire_api");
        assert!(!path.exists());
    }

    #[test]
    fn a_provider_key_that_could_forge_structure_is_refused() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let store = InMemoryCredentialStore::default();
        let mut draft = draft();
        draft.provider = "relay\": {}, \"evil".into();

        let error = write_self_agent_provider(
            &loader(&path),
            &config(r#"{"version": 1}"#),
            &store,
            &draft,
            None,
        )
        .expect_err("a key outside the identifier set must be refused");

        assert!(matches!(
            error,
            ProviderEditError::Config(ConfigError::Validation { .. })
        ));
        assert!(!path.exists());
    }

    #[test]
    fn the_offered_wires_are_exactly_the_ones_resolution_accepts() {
        for wire in PROVIDER_WIRE_APIS {
            let mut draft = draft();
            draft.wire_api = wire.into();
            assert!(
                normalize(&draft).is_ok(),
                "{wire} is offered but would be refused"
            );
        }
    }
}
