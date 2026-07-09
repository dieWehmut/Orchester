//! Built-in adapters shipped with Orchester.
//!
//! Two kinds live here:
//!
//! * [`MockAdapter`] — a scripted, subprocess-free adapter for testing the whole
//!   pipeline with no external CLIs.
//! * The three real agents (`claude`, `codex`, `opencode`) are **not** bespoke
//!   Rust types. Each is a [`ManifestAdapter`](orchester_vertrag::ManifestAdapter)
//!   built from a TOML manifest embedded at compile time. This is the "data by
//!   default" half of the hybrid model: adding an agent normally means shipping a
//!   manifest, not writing code. The manifests also live on disk under
//!   `manifeste/` so a user copy can override the built-in by name.
//!
//! The embedded copies here guarantee the three headline agents work even if the
//! `manifeste/` directory is missing (e.g. running the binary from elsewhere).

mod mock;

pub use mock::MockAdapter;

use orchester_vertrag::{AdapterError, ManifestAdapter};

/// Embedded manifest source for Claude Code.
pub const CLAUDE_MANIFEST: &str = include_str!("../../../manifeste/claude.toml");
/// Embedded manifest source for Codex CLI.
pub const CODEX_MANIFEST: &str = include_str!("../../../manifeste/codex.toml");
/// Embedded manifest source for OpenCode.
pub const OPENCODE_MANIFEST: &str = include_str!("../../../manifeste/opencode.toml");

/// The three built-in manifest sources, paired with their canonical name.
pub const BUILTIN_MANIFESTS: [(&str, &str); 3] = [
    ("claude", CLAUDE_MANIFEST),
    ("codex", CODEX_MANIFEST),
    ("opencode", OPENCODE_MANIFEST),
];

/// Build a [`ManifestAdapter`] from one of the embedded built-in manifests.
///
/// Returns `Ok(None)` for an unknown name so callers can fall through to disk
/// manifests or the mock adapter without treating "not built in" as an error.
pub fn builtin(name: &str) -> Result<Option<ManifestAdapter>, AdapterError> {
    let Some((_, toml)) = BUILTIN_MANIFESTS.iter().find(|(n, _)| *n == name) else {
        return Ok(None);
    };
    Ok(Some(ManifestAdapter::from_toml(toml)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_vertrag::{AdapterManifest, AgentAdapter};

    #[test]
    fn all_builtin_manifests_parse() {
        for (name, toml) in BUILTIN_MANIFESTS {
            let adapter = ManifestAdapter::from_toml(toml)
                .unwrap_or_else(|e| panic!("manifest {name} failed to parse: {e}"));
            assert_eq!(adapter.name(), name);
            assert!(adapter.capabilities().supports_resume);
        }
    }

    #[test]
    fn unknown_builtin_is_none() {
        assert!(builtin("does-not-exist").unwrap().is_none());
    }

    #[test]
    fn codex_resume_uses_subcommand() {
        // Regression guard for codex's irregular `exec resume <id>` shape:
        // the resume template must begin with the `resume` subcommand positionally.
        let manifest: AdapterManifest =
            toml::from_str(CODEX_MANIFEST).expect("codex manifest parses");
        let resume = manifest
            .resume_args
            .expect("codex declares resume_args");
        assert_eq!(resume[0], "exec");
        assert_eq!(resume[1], "resume");
        assert_eq!(resume[2], "{session_id}");
    }
}
