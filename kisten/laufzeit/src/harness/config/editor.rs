//! Comment-preserving edits to the user configuration.
//!
//! Everything else in this crate treats `orchester.jsonc` as read-only, and for
//! good reason: it is the one file a human is expected to hand-edit, and a
//! serializer round trip would silently eat their comments. An interactive
//! provider form still has to be able to write, so it writes the only way that
//! keeps that promise — by splicing single members through
//! [`super::document`], leaving every other byte where it was.
//!
//! The file may hold a literal API key, so the text is carried in
//! [`Zeroizing`] from read to write, the previous version is preserved with the
//! same owner-only permissions as the original, and nothing is echoed back to
//! the caller.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use super::document::{quote, upsert_member, DocumentEditError};
use super::{
    path_entry_exists, protected_file, require_user_permissions, ConfigError, ConfigLoader,
};
use crate::harness::private_fs::{create_private_dir_all, write_private_file};

/// Suffix of the copy kept beside the configuration before it is rewritten.
///
/// One fixed name rather than a timestamp: a backup of this file can contain a
/// literal key, and a growing pile of those is a hazard, not a safety net.
const BACKUP_SUFFIX: &str = "bak";

/// Suffix of the file the new text is written to before it takes the real name.
///
/// [`write_private_file`] creates and refuses to truncate, which is what makes
/// tightening the new file's permissions safe. Replacing a configuration
/// therefore has to happen as a rename, and a rename needs somewhere to come
/// from.
const STAGING_SUFFIX: &str = "new";

/// A value an edit can splice into the configuration.
///
/// Deliberately small. Configuration edits write names, URLs and blocks of
/// those; anything richer belongs in the file the human is editing by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    String(String),
    Object(Vec<(String, ConfigValue)>),
}

impl ConfigValue {
    /// Render as JSON text whose inner lines start from `indent`, so a spliced
    /// block lines up with the block it lands in.
    fn render(&self, indent: &str) -> String {
        match self {
            Self::String(value) => quote(value),
            Self::Object(members) if members.is_empty() => "{}".into(),
            Self::Object(members) => {
                let inner = format!("{indent}  ");
                let body = members
                    .iter()
                    .map(|(key, value)| format!("{inner}{}: {}", quote(key), value.render(&inner)))
                    .collect::<Vec<_>>()
                    .join(",\n");
                format!("{{\n{body}\n{indent}}}")
            }
        }
    }
}

/// What an edit did, so the caller can tell the human where their old file went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEdit {
    pub path: PathBuf,
    /// No configuration existed, so one was created around the edit.
    pub created: bool,
    /// Where the previous text was kept. Absent when there was none.
    pub backup: Option<PathBuf>,
}

impl ConfigLoader {
    /// Replace or insert each named member of the user configuration.
    ///
    /// A path is a chain of member names, so `["model_providers", "relay"]`
    /// addresses one provider entry; intermediate blocks are created when they
    /// are missing. Every other member, and every comment, blank line and
    /// indent the human chose, is copied through unchanged.
    ///
    /// The previous text is kept beside the file with a `.bak` suffix before the
    /// new one is written, so a mistyped form is one rename away from undone.
    pub fn edit_user_config(
        &self,
        members: &[(Vec<String>, ConfigValue)],
    ) -> Result<ConfigEdit, ConfigError> {
        edit_config_file(&self.user_path, members)
    }
}

fn edit_config_file(
    path: &Path,
    members: &[(Vec<String>, ConfigValue)],
) -> Result<ConfigEdit, ConfigError> {
    if members.is_empty() {
        return Err(uneditable("an edit named no configuration member"));
    }
    let existed = path_entry_exists(path)?;
    let source = if existed {
        // The same gates the loader applies: this file may hold a literal key,
        // and a version of it that only the editor will read is not safer.
        require_user_permissions(path)?;
        protected_file::read_protected_file(path)?
    } else {
        Zeroizing::new(initial_document())
    };

    let mut edited = Zeroizing::new(source.to_string());
    for (member, value) in members {
        let member: Vec<&str> = member.iter().map(String::as_str).collect();
        edited = Zeroizing::new(
            upsert_member(&edited, &member, |indent| value.render(indent))
                .map_err(document_error)?,
        );
    }

    let staging = suffixed(path, STAGING_SUFFIX);
    let backup = existed.then(|| backup_path(path));
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent)?;
    }
    // A staging file left behind by an interrupted edit holds a draft nobody
    // loaded, so it is ours to reclaim rather than a reason to refuse.
    remove_if_present(&staging)?;
    write_private_file(&staging, &edited)?;
    if let Some(backup) = backup.as_deref() {
        // The previous version is moved rather than copied: it keeps the
        // permissions it already had, and a file that may hold a literal key is
        // never duplicated into a second place we would have to protect.
        fs::rename(path, backup)?;
    }
    if let Err(error) = fs::rename(&staging, path) {
        // Put the human's file back before reporting. A failed edit that also
        // left them without a configuration would be the worse failure.
        if let Some(backup) = backup.as_deref() {
            let _ = fs::rename(backup, path);
        }
        let _ = fs::remove_file(&staging);
        return Err(error.into());
    }
    Ok(ConfigEdit {
        path: path.to_path_buf(),
        created: !existed,
        backup,
    })
}

fn remove_if_present(path: &Path) -> Result<(), ConfigError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// The file created when none exists. Only the frame an edit needs, annotated
/// so the human who inherits it knows what it is and what may happen to it.
fn initial_document() -> String {
    "// Orchester user configuration.\n\
     //\n\
     // Format is JSONC: comments and trailing commas are allowed.\n\
     // Created owner-only; Orchester refuses to load it if that privacy is lost.\n\
     //\n\
     // Orchester edits this file one member at a time and keeps the previous\n\
     // version beside it as `orchester.jsonc.bak`, so comments and formatting\n\
     // are yours to keep.\n\
     {\n  \
       \"version\": 1\n\
     }\n"
    .to_owned()
}

fn backup_path(path: &Path) -> PathBuf {
    suffixed(path, BACKUP_SUFFIX)
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Both document failures name only our own path segments, so the message can
/// be surfaced as-is without echoing configuration content back.
fn document_error(error: DocumentEditError) -> ConfigError {
    uneditable(error.to_string())
}

fn uneditable(reason: impl Into<String>) -> ConfigError {
    ConfigError::Uneditable {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-config-editor-{}-{sequence}",
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

    fn entry() -> ConfigValue {
        ConfigValue::Object(vec![
            (
                "base_url".into(),
                ConfigValue::String("https://relay.test/v1".into()),
            ),
            ("wire_api".into(), ConfigValue::String("anthropic".into())),
            (
                "api_key".into(),
                ConfigValue::String("${secret:relay}".into()),
            ),
        ])
    }

    fn provider_edit() -> Vec<(Vec<String>, ConfigValue)> {
        vec![(vec!["model_providers".into(), "relay".into()], entry())]
    }

    fn loader(path: &Path) -> ConfigLoader {
        ConfigLoader::test().with_user_path(path)
    }

    #[test]
    fn a_missing_configuration_is_created_and_loads_back_through_our_own_gates() {
        let root = TempDir::new();
        let path = root.join(".orchester").join("orchester.jsonc");
        let loader = loader(&path);

        let edit = loader
            .edit_user_config(&provider_edit())
            .expect("create a configuration");

        assert!(edit.created);
        assert!(edit.backup.is_none());
        // Reading it back through the loader is the real assertion: an edit that
        // produces a file Orchester then refuses to load leaves every later
        // command failing on a path we wrote ourselves.
        let config = loader.load_user_file(&path).expect("load the written file");
        let provider = config
            .model_providers()
            .get("relay")
            .expect("the spliced provider");
        assert_eq!(provider.base_url.as_deref(), Some("https://relay.test/v1"));
        assert_eq!(provider.wire_api.as_deref(), Some("anthropic"));
    }

    #[test]
    fn an_existing_configuration_keeps_its_comments_and_every_other_member() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "// mine, and I want it back exactly like this\n\
             {\n  \
               // the model every provider here serves\n  \
               \"model\": \"claude-opus-4-6\",\n  \
               \"model_providers\": {\n    \
                 // straight to the vendor\n    \
                 \"direct\": { \"base_url\": \"https://api.anthropic.com\" }\n  \
               }\n\
             }\n";
        fs::write(&path, original).expect("seed a configuration");

        loader(&path)
            .edit_user_config(&provider_edit())
            .expect("splice a provider");

        let written = fs::read_to_string(&path).expect("read the edited file");
        assert!(written.contains("// mine, and I want it back exactly like this"));
        assert!(written.contains("// the model every provider here serves"));
        assert!(written.contains("// straight to the vendor"));
        assert!(written.contains("https://api.anthropic.com"));
        assert!(written.contains("https://relay.test/v1"));
    }

    #[test]
    fn the_previous_text_is_kept_beside_the_file_before_it_is_replaced() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "{ \"version\": 1 }\n";
        fs::write(&path, original).expect("seed a configuration");

        let edit = loader(&path)
            .edit_user_config(&provider_edit())
            .expect("splice a provider");

        let backup = edit.backup.expect("an existing file must be preserved");
        assert_eq!(
            backup.file_name().and_then(|name| name.to_str()),
            Some("orchester.jsonc.bak")
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("read the backup"),
            original
        );
    }

    #[test]
    fn several_members_are_written_in_one_pass() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let loader = loader(&path);
        let members = vec![
            (vec!["model_providers".into(), "relay".into()], entry()),
            (
                vec!["model_provider".into()],
                ConfigValue::String("relay".into()),
            ),
            (
                vec!["model".into()],
                ConfigValue::String("claude-opus-4-6".into()),
            ),
        ];

        loader
            .edit_user_config(&members)
            .expect("write provider, selection and model together");

        let config = loader.load_user_file(&path).expect("load the written file");
        assert_eq!(config.model_provider.as_deref(), Some("relay"));
        assert_eq!(config.model.as_deref(), Some("claude-opus-4-6"));
        // All three landed, so a form can add a provider and start using it
        // without a second write that could half-fail.
        let profile = config
            .resolve_model_profile()
            .expect("the written provider resolves");
        assert_eq!(profile.wire_api, "anthropic");
    }

    #[test]
    fn a_configuration_that_cannot_be_spliced_is_left_untouched() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        let original = "[1, 2]\n";
        fs::write(&path, original).expect("seed a configuration");

        let error = loader(&path)
            .edit_user_config(&provider_edit())
            .expect_err("an array has no member to splice");

        assert!(matches!(error, ConfigError::Uneditable { .. }));
        assert_eq!(
            fs::read_to_string(&path).expect("read the file"),
            original,
            "a refused edit must not have written anything"
        );
        assert!(!backup_path(&path).exists());
    }

    #[test]
    fn an_edit_that_names_no_member_is_refused() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");

        let error = loader(&path)
            .edit_user_config(&[])
            .expect_err("an empty edit has nothing to write");

        assert!(matches!(error, ConfigError::Uneditable { .. }));
        assert!(!path.exists());
    }

    #[test]
    fn a_rendered_block_carries_the_indentation_of_the_block_it_lands_in() {
        let root = TempDir::new();
        let path = root.join("orchester.jsonc");
        fs::write(
            &path,
            "{\n  \"model_providers\": {\n    \"direct\": { \"base_url\": \"https://direct.test\" }\n  }\n}\n",
        )
        .expect("seed a configuration");

        loader(&path)
            .edit_user_config(&provider_edit())
            .expect("splice a provider");

        let written = fs::read_to_string(&path).expect("read the edited file");
        assert!(
            written.contains("\n    \"relay\": {\n      \"base_url\""),
            "the entry should line up with its neighbour:\n{written}"
        );
    }
}
