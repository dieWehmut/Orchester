//! Normalize bracketed paste for the Windows console event reader, which
//! otherwise exposes its delimiters as ordinary key events.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use unicode_width::UnicodeWidthChar;

const START: &str = "\x1b[200~";
const END: &str = "\x1b[201~";
const ESCAPE_DELAY: Duration = Duration::from_millis(25);

#[derive(Default)]
pub(super) struct InputReader {
    pending: VecDeque<Event>,
    prefix: String,
    prefix_events: Vec<Event>,
    prefix_started: Option<Instant>,
    paste: Option<String>,
    enter: Option<(Event, Instant)>,
    burst: Option<(String, Instant)>,
    zero_width_pressed: Option<char>,
}

impl InputReader {
    pub(super) fn pop_pending(&mut self) -> Option<Event> {
        self.pending.pop_front()
    }

    pub(super) fn put_back(&mut self, event: Event) {
        self.pending.push_front(event);
    }

    pub(super) fn escape_timeout(&self) -> Option<Duration> {
        [
            self.prefix_started,
            self.enter.as_ref().map(|(_, start)| *start),
            self.burst.as_ref().map(|(_, start)| *start),
        ]
        .into_iter()
        .flatten()
        .map(|start| ESCAPE_DELAY.saturating_sub(start.elapsed()))
        .min()
    }

    pub(super) fn flush_escape(&mut self) {
        if self
            .prefix_started
            .is_some_and(|start| start.elapsed() >= ESCAPE_DELAY)
        {
            self.pending.extend(std::mem::take(&mut self.prefix_events));
            self.prefix.clear();
            self.prefix_started = None;
        }
        if self
            .enter
            .as_ref()
            .is_some_and(|(_, start)| start.elapsed() >= ESCAPE_DELAY)
        {
            if let Some((event, _)) = self.enter.take() {
                self.pending.push_back(event);
            }
        }
        if self
            .burst
            .as_ref()
            .is_some_and(|(_, start)| start.elapsed() >= ESCAPE_DELAY)
        {
            if let Some((text, _)) = self.burst.take() {
                self.pending.push_back(Event::Paste(text));
            }
        }
    }

    pub(super) fn feed(&mut self, mut event: Event) -> Option<Event> {
        let key = match &mut event {
            Event::Key(key) if key.kind == KeyEventKind::Release => {
                // ConHost delivers ZWJ and combining marks as release-only
                // records. Preserve those records, while ignoring the release
                // when the corresponding press was already delivered.
                match key.code {
                    KeyCode::Char(ch)
                        if cfg!(windows) && !ch.is_control() && ch.width() == Some(0) =>
                    {
                        if self.zero_width_pressed.take() == Some(ch) {
                            return None;
                        }
                        key.kind = KeyEventKind::Press;
                        *key
                    }
                    _ => return None,
                }
            }
            Event::Key(key) => {
                self.zero_width_pressed = match key.code {
                    KeyCode::Char(ch) if !ch.is_control() && ch.width() == Some(0) => Some(ch),
                    _ => None,
                };
                *key
            }
            _ => return Some(event),
        };
        let text = key_text(key);
        if let Some(paste) = &mut self.paste {
            paste.push_str(&text);
            if paste.ends_with(END) {
                paste.truncate(paste.len() - END.len());
                return self.paste.take().map(Event::Paste);
            }
            return None;
        }
        // ConHost may swallow the VT paste delimiters before ReadConsoleInput
        // sees them. Hold a plain Enter briefly: text arriving in the same
        // burst makes it a pasted newline, never a submit halfway through a
        // multiline paste. An isolated Enter still submits after 25 ms.
        if let Some((entered, _)) = self.enter.take() {
            // ConHost exposes the LF half of CRLF as Ctrl+Enter.
            if key.code == KeyCode::Enter && key.modifiers == KeyModifiers::CONTROL {
                self.enter = Some((entered, Instant::now()));
                return None;
            }
            if is_paste_key(key) {
                self.burst = Some((format!("\n{text}"), Instant::now()));
                return None;
            }
            self.pending.push_back(entered);
            self.pending.push_back(event);
            return self.pop_pending();
        }
        if let Some((burst, start)) = &mut self.burst {
            if is_paste_key(key) {
                if !(key.code == KeyCode::Enter
                    && key.modifiers == KeyModifiers::CONTROL
                    && burst.ends_with('\n'))
                {
                    burst.push_str(&text);
                }
                *start = Instant::now();
                return None;
            }
            let (text, _) = self.burst.take().expect("active paste burst");
            self.pending.push_back(Event::Paste(text));
            self.pending.push_back(event);
            return self.pop_pending();
        }
        if cfg!(windows)
            && self.prefix.is_empty()
            && key.code == KeyCode::Enter
            && key.modifiers.is_empty()
        {
            self.enter = Some((event, Instant::now()));
            return None;
        }
        if self.prefix.is_empty() && !text.starts_with('\x1b') {
            return Some(event);
        }
        if self.prefix.is_empty() {
            self.prefix_started = Some(Instant::now());
        }
        self.prefix.push_str(&text);
        self.prefix_events.push(event);
        if self.prefix == START {
            self.paste = Some(String::new());
            self.prefix.clear();
            self.prefix_events.clear();
            self.prefix_started = None;
        } else if !START.starts_with(&self.prefix) {
            self.pending.extend(std::mem::take(&mut self.prefix_events));
            self.prefix.clear();
            self.prefix_started = None;
        }
        self.pop_pending()
    }
}

fn is_paste_key(key: KeyEvent) -> bool {
    (key.code == KeyCode::Enter && key.modifiers == KeyModifiers::CONTROL)
        || (matches!(key.code, KeyCode::Char(_) | KeyCode::Enter | KeyCode::Tab)
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
}

fn key_text(key: KeyEvent) -> String {
    let text = match key.code {
        KeyCode::Esc => "\x1b".into(),
        KeyCode::Enter => "\n".into(),
        KeyCode::Tab => "\t".into(),
        KeyCode::Backspace => "\x08".into(),
        KeyCode::Char(ch) => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && ch.is_ascii_alphabetic()
            {
                ((ch.to_ascii_uppercase() as u8 - b'A' + 1) as char).to_string()
            } else {
                ch.to_string()
            }
        }
        KeyCode::Left => "\x1b[D".into(),
        KeyCode::Right => "\x1b[C".into(),
        KeyCode::Up => "\x1b[A".into(),
        KeyCode::Down => "\x1b[B".into(),
        _ => String::new(),
    };
    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL) {
        format!("\x1b{text}")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_paste_delimiters_produce_one_literal_multiline_event() {
        let mut reader = InputReader::default();
        let input = "\x1b[200~/quit\n第二行\x1b[201~";
        let mut result = Vec::new();
        for ch in input.chars() {
            let code = match ch {
                '\x1b' => KeyCode::Esc,
                '\n' => KeyCode::Enter,
                _ => KeyCode::Char(ch),
            };
            if let Some(event) = reader.feed(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))) {
                result.push(event);
            }
        }
        assert_eq!(result, vec![Event::Paste("/quit\n第二行".into())]);
    }

    #[test]
    fn an_unrelated_alt_key_or_split_escape_sequence_is_not_lost() {
        let mut reader = InputReader::default();
        let escape = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(reader.feed(escape.clone()).is_none());
        reader.prefix_started = Some(Instant::now() - ESCAPE_DELAY);
        reader.flush_escape();
        assert_eq!(reader.pop_pending(), Some(escape));
        let key = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT));
        assert_eq!(reader.feed(key.clone()), Some(key));
    }

    #[cfg(windows)]
    #[test]
    fn legacy_windows_crlf_bursts_do_not_emit_an_enter_mid_paste() {
        let mut reader = InputReader::default();
        let enter = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(reader.feed(enter.clone()).is_none());
        assert!(reader
            .feed(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::CONTROL
            )))
            .is_none());
        for ch in "second line".chars() {
            assert!(reader
                .feed(Event::Key(KeyEvent::new(
                    KeyCode::Char(ch),
                    KeyModifiers::NONE
                )))
                .is_none());
        }
        assert!(reader.feed(enter.clone()).is_none());
        assert!(reader
            .feed(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::CONTROL
            )))
            .is_none());
        reader.burst.as_mut().unwrap().1 = Instant::now() - ESCAPE_DELAY;
        reader.flush_escape();
        assert_eq!(
            reader.pop_pending(),
            Some(Event::Paste("\nsecond line\n".into()))
        );
        assert!(reader.pop_pending().is_none());
        assert!(reader.feed(enter.clone()).is_none());
        reader.enter.as_mut().unwrap().1 = Instant::now() - ESCAPE_DELAY;
        reader.flush_escape();
        assert_eq!(reader.pop_pending(), Some(enter));
    }

    #[cfg(windows)]
    #[test]
    fn windows_release_only_format_characters_are_preserved_once() {
        let mut reader = InputReader::default();
        for ch in ['\u{200d}', '\u{301}'] {
            let release = Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char(ch),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ));
            let press = Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            assert_eq!(reader.feed(release.clone()), Some(press.clone()));
            assert_eq!(reader.feed(press.clone()), Some(press));
            assert_eq!(reader.feed(release), None);
        }
    }
}
