use std::io::{self, Write};
use std::path::Path;

use orchester_laufzeit::harness::credentials::KEYRING_SERVICE;
use orchester_laufzeit::harness::service::{ConfigWiring, CredentialTarget, CredentialUpdate};

use super::models::safe_metadata;
use super::render::safe_terminal_text;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Name what `/login` is about to act on, printed before the key is requested
/// so a mistyped provider is caught before anything is pasted.
pub fn render_credential_target(out: &mut impl Write, target: &CredentialTarget) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{BOLD}Login{RESET}: {}",
        safe_metadata(&target.provider)
    )?;
    writeln!(
        out,
        "  base url: {}",
        target
            .base_url
            .as_deref()
            .map(safe_metadata)
            .unwrap_or_else(|| "not configured".into())
    )?;
    // An existing key is worth naming: the human is about to overwrite it, and
    // "unverified" says the stored value was never proven to work.
    writeln!(
        out,
        "  keyring:  {}",
        if target.present {
            "present, unverified — a new key will replace it"
        } else {
            "no key stored"
        }
    )?;
    writeln!(out, "  config:   {}", config_state(target))?;
    writeln!(
        out,
        "{DIM}The key is stored exactly as typed; nothing is sent to the provider to check it.{RESET}"
    )?;
    writeln!(out)
}

/// Confirm a stored key: where it went, how the config reaches it, and the
/// masked tail that lets a human recognize what they pasted.
///
/// `config_path` is reported rather than a fixed location because
/// `ORCHESTER_HOME` moves the file; naming a constant path would send the
/// reader looking for a key that is not there.
pub fn render_credential_stored(
    out: &mut impl Write,
    update: &CredentialUpdate,
    wiring: &ConfigWiring,
    config_path: &Path,
) -> io::Result<()> {
    let provider = safe_metadata(&update.provider);
    let reference = safe_metadata(&update.reference);
    // The path reaches here from the environment, so it is escaped like every
    // other rendered value: a newline in it could otherwise forge the `ok` line.
    let config_path = safe_metadata(&config_path.display().to_string());
    writeln!(out)?;
    writeln!(out, "  stored   OS keyring (service: {KEYRING_SERVICE})")?;
    match wiring {
        ConfigWiring::Created => {
            writeln!(out, "  created  {config_path}")?;
            writeln!(
                out,
                "           model_providers.{provider}.api_key = {reference}"
            )?;
        }
        ConfigWiring::AlreadyReferenced => {
            writeln!(out, "  wired    {config_path}")?;
            writeln!(
                out,
                "           model_providers.{provider}.api_key = {reference}"
            )?;
        }
        // The config is human-owned and may carry comments, so it is never
        // rewritten; the human is handed the exact text to paste.
        ConfigWiring::NeedsReference { snippet } => {
            writeln!(out, "  pending  {config_path} — add this to reach the key:")?;
            for line in safe_terminal_text(snippet).lines() {
                writeln!(out, "           {line}")?;
            }
        }
    }
    writeln!(out)?;
    writeln!(out, "{BOLD}ok{RESET}  {provider}  {}", update.masked)?;
    writeln!(
        out,
        "{DIM}Stored but unverified: the first turn is what proves the key.{RESET}"
    )?;
    writeln!(out)
}

/// Report a `/logout`, distinguishing a key that was removed from one that was
/// never there.
pub fn render_credential_cleared(
    out: &mut impl Write,
    provider: &str,
    removed: bool,
) -> io::Result<()> {
    let provider = safe_metadata(provider);
    writeln!(out)?;
    if removed {
        writeln!(out, "{BOLD}Logout{RESET}: {provider}")?;
        writeln!(out, "  removed  OS keyring (service: {KEYRING_SERVICE})")?;
        // The reference stays valid and simply resolves to nothing, so the
        // human is not left wondering whether their config is now broken.
        writeln!(
            out,
            "{DIM}The configuration reference was left in place; /login stores a key under it again.{RESET}"
        )?;
    } else {
        writeln!(out, "{BOLD}Logout{RESET}: {provider} — no key was stored")?;
    }
    writeln!(out)
}

fn config_state(target: &CredentialTarget) -> String {
    if target.referenced {
        format!("api_key = {}", safe_metadata(&target.reference))
    } else if target.configured {
        "provider configured without an api_key".into()
    } else {
        "provider not in the configuration yet".into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn target(present: bool, base_url: Option<&str>) -> CredentialTarget {
        CredentialTarget {
            provider: "OpenAI".into(),
            base_url: base_url.map(str::to_owned),
            configured: true,
            referenced: true,
            present,
            reference: "${secret:OpenAI}".into(),
        }
    }

    fn update() -> CredentialUpdate {
        CredentialUpdate {
            provider: "OpenAI".into(),
            masked: "…9f2c".into(),
            reference: "${secret:OpenAI}".into(),
        }
    }

    fn rendered(render: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> String {
        let mut output = Vec::new();
        render(&mut output).expect("render");
        String::from_utf8(output).expect("UTF-8")
    }

    #[test]
    fn the_prompt_header_names_the_provider_and_its_endpoint() {
        let rendered = rendered(|out| {
            render_credential_target(out, &target(false, Some("https://agentrouter.org")))
        });

        assert!(rendered.contains("OpenAI"));
        assert!(rendered.contains("https://agentrouter.org"));
        assert!(rendered.contains("no key stored"));
    }

    #[test]
    fn an_existing_key_is_announced_as_unverified_so_a_replacement_is_a_choice() {
        let rendered =
            rendered(|out| render_credential_target(out, &target(true, Some("https://x.test"))));

        assert!(rendered.contains("present, unverified"));
        assert!(rendered.contains("replace"));
    }

    #[test]
    fn a_stored_key_reports_the_keyring_service_and_only_a_masked_tail() {
        let rendered = rendered(|out| {
            render_credential_stored(
                out,
                &update(),
                &ConfigWiring::AlreadyReferenced,
                Path::new("/home/example/.orchester/orchester.jsonc"),
            )
        });

        assert!(rendered.contains(KEYRING_SERVICE));
        assert!(rendered.contains("…9f2c"));
        assert!(rendered.contains("${secret:OpenAI}"));
        // The store is written without a provider request, so the confirmation
        // must not read as proof that the key works.
        assert!(rendered.contains("unverified"));
    }

    #[test]
    fn a_created_config_is_reported_at_the_path_it_was_written_to() {
        let rendered = rendered(|out| {
            render_credential_stored(
                out,
                &update(),
                &ConfigWiring::Created,
                Path::new("/tmp/elsewhere/orchester.jsonc"),
            )
        });

        assert!(rendered.contains("created"));
        assert!(rendered.contains("${secret:OpenAI}"));
        // `ORCHESTER_HOME` can move the file, so naming a fixed location would
        // send the reader looking for a key that is not there.
        assert!(rendered.contains("/tmp/elsewhere/orchester.jsonc"));
    }

    /// The path comes from `ORCHESTER_HOME`, so it is attacker-influenced in
    /// exactly the way every other rendered value is.  The renderer emits its
    /// own escapes for the `ok` line, so what must be absent is specifically an
    /// escape the renderer never writes.
    #[test]
    fn a_home_holding_control_characters_cannot_forge_confirmation_lines() {
        let rendered = rendered(|out| {
            render_credential_stored(
                out,
                &update(),
                &ConfigWiring::Created,
                Path::new("/tmp/\u{1b}[31mred\nok  forged/orchester.jsonc"),
            )
        });

        assert!(rendered.contains("\\u{1b}[31m"));
        assert!(!rendered.contains("\u{1b}[31m"));
        assert!(!rendered.contains("\nok  forged"));
    }

    #[test]
    fn an_unwired_config_is_told_exactly_what_to_add() {
        let wiring = ConfigWiring::NeedsReference {
            snippet: "\"model_providers\": { \"OpenAI\": { \"api_key\": \"${secret:OpenAI}\" } }"
                .into(),
        };
        let rendered = rendered(|out| {
            render_credential_stored(
                out,
                &update(),
                &wiring,
                Path::new("/home/x/orchester.jsonc"),
            )
        });

        assert!(rendered.contains("add this"));
        assert!(rendered.contains("\"api_key\": \"${secret:OpenAI}\""));
        // Nothing was written, so the confirmation must not claim otherwise.
        assert!(!rendered.contains("created"));
    }

    #[test]
    fn clearing_distinguishes_a_removed_key_from_one_that_was_never_there() {
        let removed = rendered(|out| render_credential_cleared(out, "OpenAI", true));
        let absent = rendered(|out| render_credential_cleared(out, "OpenAI", false));

        assert!(removed.contains("removed"));
        assert!(absent.contains("no key was stored"));
        assert!(!absent.contains("removed"));
    }

    #[test]
    fn rendering_escapes_terminal_controls_carried_by_config_metadata() {
        let rendered = rendered(|out| {
            render_credential_target(out, &target(false, Some("https://x.test\x1b[31m")))
        });

        assert!(rendered.contains("https://x.test\\u{1b}[31m"));
        assert!(!rendered.contains("https://x.test\x1b[31m"));
    }
}
