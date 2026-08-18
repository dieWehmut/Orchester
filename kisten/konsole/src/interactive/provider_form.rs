//! Collecting one `model_providers` entry from a human, field by field.
//!
//! The form runs on the plain terminal rather than inside the chat frame, for
//! the same reason `/login` does: the API key is read through
//! [`prompt_secret`], which needs raw mode of its own.  Every other field is a
//! line of text with the current value offered as the default, so editing an
//! existing provider is a matter of pressing Enter past the parts that are
//! already right.
//!
//! Nothing here validates a field.  The harness resolves the whole draft with
//! the same gates a later start applies and names the field that failed, so a
//! second, weaker copy of those rules living in the terminal layer could only
//! disagree with the real one.

use std::io::{self, BufRead, Write};

use orchester_laufzeit::harness::service::{ProviderDraft, PROVIDER_WIRE_APIS};
use secrecy::SecretString;

use super::prompt_secret;
use crate::self_agent::safe_metadata;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Typed alone, empties a field that currently has a value.  Without it an
/// optional field could be filled but never cleared, because an empty line
/// already means "keep what is there".
const CLEAR: &str = "-";

/// A completed form: the entry to write, and the key to store under it.
///
/// `secret` is absent when the human left the key blank, which means "leave
/// whatever is in the credential store alone" — the common case when only a
/// base URL or a model needed changing.
pub struct ProviderForm {
    pub draft: ProviderDraft,
    pub secret: Option<SecretString>,
}

/// Ask for every field of a provider entry, starting from `defaults`.
///
/// `None` means the human cancelled, and no caller may treat that as an empty
/// entry.  Cancelling is deliberately reachable from the first field: a form
/// opened by mistake is escaped by pressing Enter on an empty provider key.
pub fn prompt_provider_form(defaults: &ProviderDraft) -> io::Result<Option<ProviderForm>> {
    let mut out = io::stdout();
    let editing = !defaults.provider.is_empty();
    write_header(&mut out, editing)?;

    let draft = {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        read_draft(&mut input, &mut out, defaults)?
    };
    let Some(draft) = draft else {
        return Ok(None);
    };

    // The key is optional on purpose. A stored key outlives the entry that
    // references it, so an edit that only moves a base URL must not force the
    // human to paste their key again.
    writeln!(
        out,
        "{DIM}Leave the key empty to keep whatever is already stored.{RESET}"
    )?;
    let secret = prompt_secret("  api key")?;
    Ok(Some(ProviderForm { draft, secret }))
}

fn write_header(out: &mut impl Write, editing: bool) -> io::Result<()> {
    writeln!(out)?;
    if editing {
        writeln!(out, "{BOLD}Edit provider{RESET}")?;
    } else {
        writeln!(out, "{BOLD}Add provider{RESET}")?;
    }
    writeln!(
        out,
        "{DIM}Enter keeps the value in brackets; `{CLEAR}` clears it; an empty provider key cancels.{RESET}"
    )
}

/// Read every text field of the draft, or `None` if the form was cancelled.
///
/// Separated from [`prompt_provider_form`] so the field-by-field behaviour can
/// be tested against a pipe instead of a terminal, which is also what makes the
/// cancel and clear rules assertable.
fn read_draft(
    input: &mut impl BufRead,
    out: &mut impl Write,
    defaults: &ProviderDraft,
) -> io::Result<Option<ProviderDraft>> {
    let Some(provider) = read_field(input, out, "provider key", &defaults.provider)? else {
        return Ok(None);
    };
    if provider.is_empty() {
        writeln!(out, "cancelled; nothing was written")?;
        return Ok(None);
    }
    let Some(name) = read_field(input, out, "display name", &defaults.name)? else {
        return Ok(None);
    };
    let Some(base_url) = read_field(input, out, "base url", &defaults.base_url)? else {
        return Ok(None);
    };
    let Some(wire_api) = read_wire(input, out, &defaults.wire_api)? else {
        return Ok(None);
    };
    let Some(model) = read_field(input, out, "model", &defaults.model)? else {
        return Ok(None);
    };
    let Some(activate) = read_flag(input, out, "make it the active provider", defaults.activate)?
    else {
        return Ok(None);
    };

    Ok(Some(ProviderDraft {
        provider,
        name,
        base_url,
        wire_api,
        model,
        activate,
    }))
}

/// Prompt for one field.  `None` is end of input, which cancels the form.
fn read_field(
    input: &mut impl BufRead,
    out: &mut impl Write,
    label: &str,
    default: &str,
) -> io::Result<Option<String>> {
    if default.is_empty() {
        write!(out, "  {label}: ")?;
    } else {
        // The default came out of the configuration file, so it is escaped like
        // every other value read from there: a control character in it could
        // otherwise rewrite the prompt around it.
        write!(out, "  {label} [{}]: ", safe_metadata(default))?;
    }
    out.flush()?;
    let Some(line) = read_line(input)? else {
        return Ok(None);
    };
    Ok(Some(match line.trim() {
        "" => default.to_owned(),
        CLEAR => String::new(),
        entered => entered.to_owned(),
    }))
}

/// Offer the wires the harness can actually build, by number or by name.
///
/// A rejected answer is asked again rather than silently defaulted: the wire
/// decides how every later request is framed, so guessing it would be the one
/// field where a typo is expensive.
fn read_wire(
    input: &mut impl BufRead,
    out: &mut impl Write,
    default: &str,
) -> io::Result<Option<String>> {
    let default = if PROVIDER_WIRE_APIS.contains(&default) {
        default
    } else {
        PROVIDER_WIRE_APIS[0]
    };
    writeln!(out, "  wire api:")?;
    for (index, wire) in PROVIDER_WIRE_APIS.iter().enumerate() {
        let current = if *wire == default { " (current)" } else { "" };
        writeln!(out, "    {}. {wire}{current}", index + 1)?;
    }
    loop {
        write!(out, "  choose 1-{} [{default}]: ", PROVIDER_WIRE_APIS.len())?;
        out.flush()?;
        let Some(line) = read_line(input)? else {
            return Ok(None);
        };
        let entered = line.trim();
        if entered.is_empty() {
            return Ok(Some(default.to_owned()));
        }
        // A human who just read the list types what they read, so a name is
        // accepted alongside its number.
        if let Some(wire) = PROVIDER_WIRE_APIS
            .iter()
            .find(|wire| wire.eq_ignore_ascii_case(entered))
        {
            return Ok(Some((*wire).to_owned()));
        }
        match entered.parse::<usize>() {
            Ok(index) if (1..=PROVIDER_WIRE_APIS.len()).contains(&index) => {
                return Ok(Some(PROVIDER_WIRE_APIS[index - 1].to_owned()));
            }
            _ => writeln!(out, "  not one of the offered wires")?,
        }
    }
}

fn read_flag(
    input: &mut impl BufRead,
    out: &mut impl Write,
    label: &str,
    default: bool,
) -> io::Result<Option<bool>> {
    loop {
        write!(out, "  {label} [{}]: ", if default { "Y/n" } else { "y/N" })?;
        out.flush()?;
        let Some(line) = read_line(input)? else {
            return Ok(None);
        };
        match line.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(Some(default)),
            "y" | "yes" => return Ok(Some(true)),
            "n" | "no" => return Ok(Some(false)),
            _ => writeln!(out, "  answer y or n")?,
        }
    }
}

/// Read one line.  `None` is end of input, never an empty line: the two mean
/// different things to every caller here.
fn read_line(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ProviderDraft {
        ProviderDraft {
            provider: "relay".into(),
            name: "Relay API".into(),
            base_url: "https://relay.test/v1".into(),
            wire_api: "anthropic".into(),
            model: "claude-opus-4-6".into(),
            activate: true,
        }
    }

    fn run(source: &str, defaults: &ProviderDraft) -> (Option<ProviderDraft>, String) {
        let mut input = source.as_bytes();
        let mut output = Vec::new();
        let draft = read_draft(&mut input, &mut output, defaults).expect("read draft");
        (draft, String::from_utf8(output).expect("UTF-8"))
    }

    #[test]
    fn pressing_enter_past_every_field_keeps_the_entry_as_it_is() {
        let (draft, rendered) = run("\n\n\n\n\n\n", &defaults());

        assert_eq!(draft, Some(defaults()));
        // Every current value has to be visible for Enter to be an informed
        // answer rather than a guess.
        assert!(rendered.contains("[relay]"));
        assert!(rendered.contains("[https://relay.test/v1]"));
        assert!(rendered.contains("anthropic (current)"));
        assert!(rendered.contains("[Y/n]"));
    }

    #[test]
    fn every_field_can_be_replaced() {
        let (draft, _) = run(
            "direct\nDirect\nhttps://api.test/v1\n1\ngpt-5\nn\n",
            &defaults(),
        );

        assert_eq!(
            draft,
            Some(ProviderDraft {
                provider: "direct".into(),
                name: "Direct".into(),
                base_url: "https://api.test/v1".into(),
                wire_api: PROVIDER_WIRE_APIS[0].into(),
                model: "gpt-5".into(),
                activate: false,
            })
        );
    }

    #[test]
    fn a_wire_can_be_chosen_by_name_as_well_as_by_number() {
        let (by_name, _) = run("relay\n\n\nANTHROPIC\n\n\n", &ProviderDraft::default());
        let (by_number, _) = run("relay\n\n\n2\n\n\n", &ProviderDraft::default());

        assert_eq!(
            by_name.map(|draft| draft.wire_api),
            Some("anthropic".into())
        );
        assert_eq!(
            by_number.map(|draft| draft.wire_api),
            Some(PROVIDER_WIRE_APIS[1].into())
        );
    }

    #[test]
    fn an_unknown_wire_is_asked_again_rather_than_guessed() {
        let (draft, rendered) = run(
            "relay\n\n\ngrpc\nresponses\n\n\n",
            &ProviderDraft::default(),
        );

        assert!(rendered.contains("not one of the offered wires"));
        assert_eq!(
            draft.map(|draft| draft.wire_api),
            Some("responses".into()),
            "the retry answer must be the one that lands"
        );
    }

    #[test]
    fn an_unreadable_yes_or_no_is_asked_again() {
        let defaults = ProviderDraft {
            activate: false,
            ..defaults()
        };
        let (draft, rendered) = run("\n\n\n\n\nmaybe\ny\n", &defaults);

        assert!(rendered.contains("answer y or n"));
        assert_eq!(draft.map(|draft| draft.activate), Some(true));
    }

    #[test]
    fn an_optional_field_can_be_cleared_but_an_empty_line_cannot_clear_it() {
        let (cleared, _) = run("\n-\n\n\n-\n\n", &defaults());
        let (kept, _) = run("\n\n\n\n\n\n", &defaults());

        let cleared = cleared.expect("draft");
        assert_eq!(cleared.name, "");
        assert_eq!(cleared.model, "");
        // The same keystroke must not mean two things: Enter is "keep".
        assert_eq!(kept.expect("draft").name, "Relay API");
    }

    #[test]
    fn an_empty_provider_key_cancels_instead_of_writing_a_nameless_entry() {
        let (draft, rendered) = run("\n", &ProviderDraft::default());

        assert_eq!(draft, None);
        assert!(rendered.contains("cancelled"));
    }

    #[test]
    fn closed_input_cancels_from_any_field() {
        for source in [
            "",
            "relay\n",
            "relay\nRelay\n",
            "relay\nRelay\nhttps://x.test\n",
        ] {
            assert_eq!(
                run(source, &ProviderDraft::default()).0,
                None,
                "input ending mid-form must cancel: {source:?}"
            );
        }
    }

    /// The defaults are echoed from the configuration file, so a control
    /// character in one of them would otherwise be written straight to the
    /// terminal that is drawing the prompt.
    #[test]
    fn a_default_carrying_terminal_controls_is_escaped_before_it_is_offered() {
        let defaults = ProviderDraft {
            base_url: "https://relay.test\u{1b}[31m".into(),
            ..ProviderDraft::default()
        };
        let (_, rendered) = run("relay\n\n\n\n\n\n", &defaults);

        assert!(rendered.contains("\\u{1b}[31m"));
        assert!(!rendered.contains("\u{1b}[31m"));
    }
}
