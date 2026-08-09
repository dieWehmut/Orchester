//! Shared projection of a configuration failure into display-safe metadata.
//!
//! Read-only self-agent projections report an unusable model configuration
//! instead of refusing to answer, so several of them need the same conversion
//! from [`ConfigError`] to bounded text. Keeping one implementation means the
//! guarantee that nothing configured is echoed cannot drift between them.

use crate::harness::config::ConfigError;

const MAX_METADATA_CHARS: usize = 200;

/// Reduce a resolution failure to a field path and a short reason.
///
/// The resolvers behind these projections only report
/// [`ConfigError::Validation`], whose members are a field path and a static
/// message, so nothing configured is carried out. Any other variant would
/// indicate a load-time fault, so it is summarized without its payload.
pub(super) fn unresolved_metadata(error: ConfigError) -> (String, String) {
    let (path, message) = match error {
        ConfigError::Validation { path, message } => (path, message),
        _ => (
            "model".to_owned(),
            "active model configuration is unavailable".to_owned(),
        ),
    };
    (bounded_metadata(&path), bounded_metadata(&message))
}

fn bounded_metadata(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_METADATA_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_failures_keep_their_field_path_and_reason() {
        let (path, message) = unresolved_metadata(ConfigError::Validation {
            path: "model_provider".into(),
            message: "active model provider is not configured".into(),
        });

        assert_eq!(path, "model_provider");
        assert_eq!(message, "active model provider is not configured");
    }

    #[test]
    fn metadata_is_stripped_of_control_characters_and_bounded() {
        let (path, message) = unresolved_metadata(ConfigError::Validation {
            path: "model\x1b[31m".into(),
            message: "x".repeat(MAX_METADATA_CHARS + 50),
        });

        assert_eq!(path, "model[31m");
        assert_eq!(message.chars().count(), MAX_METADATA_CHARS);
    }
}
