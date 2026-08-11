//! Reading an API key from a human without leaving it on screen.
//!
//! On a terminal the key is echoed as mask characters; when stdin is piped the
//! line is read verbatim, because there is no terminal to echo to.  Either way
//! the value leaves this module only as a [`SecretString`].

use std::io::{self, BufRead, IsTerminal, Write};

use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;
use secrecy::SecretString;

/// Shown in place of each typed character.
const MASK: char = '*';

/// Ask for a secret on stdin.  `None` means the human cancelled or supplied
/// nothing, and no caller may treat that as an empty key.
pub fn prompt_secret(label: &str) -> io::Result<Option<SecretString>> {
    let mut out = io::stdout();
    write!(out, "{label}: ")?;
    out.flush()?;

    if io::stdin().is_terminal() {
        let entered = read_secret_from_terminal(&mut out);
        // The newline is owed whether or not the read succeeded, otherwise the
        // next line of output lands beside the mask characters.
        writeln!(out)?;
        return entered;
    }
    let stdin = io::stdin();
    let mut input = stdin.lock();
    read_secret_from_reader(&mut input)
}

/// Read one line as a secret.  Used when stdin is not a terminal, so the
/// caller is a pipe or a test rather than a person watching the screen.
fn read_secret_from_reader(input: &mut impl BufRead) -> io::Result<Option<SecretString>> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let entered = line.trim_end_matches(['\r', '\n']).trim();
    if entered.is_empty() {
        return Ok(None);
    }
    Ok(Some(SecretString::new(entered.to_owned().into_boxed_str())))
}

/// Read a secret keystroke by keystroke, echoing only mask characters.  Raw
/// mode is disabled again on every path so a cancelled entry cannot leave the
/// terminal unusable.
fn read_secret_from_terminal(out: &mut impl Write) -> io::Result<Option<SecretString>> {
    terminal::enable_raw_mode()?;
    let entered = read_masked(out);
    terminal::disable_raw_mode()?;
    let entered = entered?;
    if entered.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(SecretString::new(
        entered.trim().to_owned().into_boxed_str(),
    )))
}

fn read_masked(out: &mut impl Write) -> io::Result<String> {
    let mut entered = String::new();
    loop {
        let TerminalEvent::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(String::new());
        }
        match key.code {
            KeyCode::Enter => return Ok(entered),
            KeyCode::Esc => return Ok(String::new()),
            KeyCode::Backspace => {
                if entered.pop().is_some() {
                    // Move back over the mask, overwrite it, move back again.
                    write!(out, "\u{8} \u{8}")?;
                    out.flush()?;
                }
            }
            KeyCode::Char(character) => {
                entered.push(character);
                write!(out, "{MASK}")?;
                out.flush()?;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn read(source: &str) -> Option<String> {
        let mut input = source.as_bytes();
        read_secret_from_reader(&mut input)
            .expect("read secret")
            .map(|secret| secret.expose_secret().to_owned())
    }

    #[test]
    fn a_piped_key_is_read_without_its_line_ending() {
        assert_eq!(read("sk-live-abcd9f2c\n"), Some("sk-live-abcd9f2c".into()));
        // A Windows pipe carries CRLF, and a stray \r inside a key would be
        // stored and later sent as part of an Authorization header.
        assert_eq!(
            read("sk-live-abcd9f2c\r\n"),
            Some("sk-live-abcd9f2c".into())
        );
    }

    #[test]
    fn surrounding_whitespace_from_a_paste_is_discarded() {
        assert_eq!(
            read("  sk-live-abcd9f2c  \n"),
            Some("sk-live-abcd9f2c".into())
        );
    }

    #[test]
    fn an_empty_line_cancels_rather_than_storing_an_empty_key() {
        assert_eq!(read("\n"), None);
        assert_eq!(read("   \n"), None);
    }

    #[test]
    fn closed_input_cancels() {
        assert_eq!(read(""), None);
    }
}
