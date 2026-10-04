//! An in-memory draft editor. History never writes prompts to the user's home.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::sanitize_terminal_text;

#[derive(Debug, Default)]
pub(crate) struct Composer {
    text: String,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    saved_draft: String,
    saved_cursor: usize,
}

impl From<String> for Composer {
    fn from(text: String) -> Self {
        Self {
            cursor: text.len(),
            text,
            ..Self::default()
        }
    }
}

impl From<&str> for Composer {
    fn from(text: &str) -> Self {
        Self::from(text.to_owned())
    }
}

impl Composer {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
    pub(crate) fn is_command_query(&self) -> bool {
        self.text.starts_with('/') && !self.text.contains('\n')
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_draft.clear();
    }

    pub(crate) fn remember_submission(&mut self) {
        if !self.text.trim().is_empty() && self.history.last() != Some(&self.text) {
            self.history.push(self.text.clone());
            if self.history.len() > 100 {
                self.history.remove(0);
            }
        }
    }

    pub(crate) fn insert(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let normalized: String = normalized
            .chars()
            .flat_map(|ch| {
                if ch.is_control() && ch != '\n' && ch != '\t' {
                    ch.escape_default().collect::<Vec<_>>()
                } else {
                    vec![ch]
                }
            })
            .collect();
        self.text.insert_str(self.cursor, &normalized);
        self.cursor += normalized.len();
        // Inserting a combining mark or ZWJ can merge across the insertion
        // point. Park on a grapheme boundary before the next editing action.
        self.snap_cursor();
        self.history_index = None;
    }

    fn snap_cursor(&mut self) {
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= self.cursor)
            .unwrap_or(self.text.len());
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(self.cursor, |g| self.cursor + g.len())
    }

    fn word_left(&self) -> usize {
        let mut end = self.cursor;
        let mut saw_word = false;
        for (i, grapheme) in self.text[..self.cursor].grapheme_indices(true).rev() {
            if grapheme.chars().all(char::is_whitespace) {
                if saw_word {
                    break;
                }
            } else {
                saw_word = true;
            }
            end = i;
        }
        end
    }

    fn word_right(&self) -> usize {
        let mut end = self.cursor;
        let mut saw_word = false;
        for (i, grapheme) in self.text[self.cursor..].grapheme_indices(true) {
            if grapheme.chars().all(char::is_whitespace) {
                if saw_word {
                    break;
                }
            } else {
                saw_word = true;
            }
            end = self.cursor + i + grapheme.len();
        }
        end
    }

    fn recall(&mut self, previous: bool) {
        if self.history.is_empty() {
            return;
        }
        let index = match (self.history_index, previous) {
            (None, true) => {
                self.saved_draft = self.text.clone();
                self.saved_cursor = self.cursor;
                self.history.len() - 1
            }
            (Some(index), true) => index.saturating_sub(1),
            (Some(index), false) if index + 1 < self.history.len() => index + 1,
            (Some(_), false) => {
                self.text = std::mem::take(&mut self.saved_draft);
                self.cursor = self.saved_cursor;
                self.history_index = None;
                return;
            }
            (None, false) => return,
        };
        self.text.clone_from(&self.history[index]);
        self.cursor = self.text.len();
        self.history_index = Some(index);
    }

    fn vertical(&mut self, previous: bool, width: usize) {
        let frame = ComposerFrame::new(&self.text, self.cursor, width);
        let row = frame.caret.row;
        if (previous && row == 0) || (!previous && self.history_index.is_some()) {
            self.recall(previous);
            return;
        }
        let target = if previous {
            row.saturating_sub(1)
        } else {
            row + 1
        };
        let target_column = frame.caret.column;
        if let Some((index, _)) = frame
            .positions
            .iter()
            .filter(|(_, caret)| caret.row == target)
            .min_by_key(|(_, caret)| caret.column.abs_diff(target_column))
        {
            self.cursor = *index;
        }
    }

    pub(crate) fn edit_key(&mut self, key: KeyEvent, width: usize) -> bool {
        let control = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Left => {
                self.cursor = if control {
                    self.word_left()
                } else {
                    self.previous()
                }
            }
            KeyCode::Right => {
                self.cursor = if control {
                    self.word_right()
                } else {
                    self.next()
                }
            }
            KeyCode::Home | KeyCode::Char('a') if key.code == KeyCode::Home || control => {
                self.cursor = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
            }
            KeyCode::End | KeyCode::Char('e') if key.code == KeyCode::End || control => {
                self.cursor += self.text[self.cursor..]
                    .find('\n')
                    .unwrap_or(self.text.len() - self.cursor);
            }
            KeyCode::Up => self.vertical(true, width),
            KeyCode::Down => self.vertical(false, width),
            KeyCode::Backspace => {
                let start = if control {
                    self.word_left()
                } else {
                    self.previous()
                };
                self.text.replace_range(start..self.cursor, "");
                self.cursor = start;
                self.history_index = None;
            }
            KeyCode::Delete => {
                let end = if control {
                    self.word_right()
                } else {
                    self.next()
                };
                self.text.replace_range(self.cursor..end, "");
                self.history_index = None;
            }
            KeyCode::Char('w') if control => {
                let start = self.word_left();
                self.text.replace_range(start..self.cursor, "");
                self.cursor = start;
                self.history_index = None;
            }
            KeyCode::Char('u') if control => self.clear(),
            KeyCode::Char('k') if control => {
                let end = self.cursor
                    + self.text[self.cursor..]
                        .find('\n')
                        .unwrap_or(self.text.len() - self.cursor);
                self.text.replace_range(self.cursor..end, "");
                self.history_index = None;
            }
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert("\n")
            }
            KeyCode::Char('j') if control => self.insert("\n"),
            KeyCode::Char(ch)
                if !control
                    && (!key.modifiers.contains(KeyModifiers::ALT)
                        || key.modifiers.contains(KeyModifiers::CONTROL)) =>
            {
                self.insert(&ch.to_string())
            }
            _ => return false,
        }
        self.snap_cursor();
        true
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DraftCaret {
    pub(crate) column: usize,
    pub(crate) row: usize,
}

pub(crate) struct ComposerFrame {
    pub(crate) rows: Vec<String>,
    pub(crate) caret: DraftCaret,
    positions: Vec<(usize, DraftCaret)>,
    pub(crate) clipped_above: bool,
    pub(crate) clipped_below: bool,
}

pub(crate) fn prompt_prefix(width: usize) -> &'static str {
    match width {
        0 | 1 => "",
        2 => ">",
        _ => "> ",
    }
}

impl ComposerFrame {
    pub(crate) fn new(text: &str, cursor: usize, width: usize) -> Self {
        let prefix = prompt_prefix(width).len();
        let budget = width.saturating_sub(prefix).max(1);
        let mut rows = Vec::new();
        let mut row = String::new();
        let mut cells = 0;
        let mut positions = Vec::new();
        for (index, grapheme) in text.grapheme_indices(true) {
            let visible = if grapheme == "\t" {
                "    ".to_string()
            } else {
                sanitize_terminal_text(grapheme)
            };
            let visible = if visible.width() > budget {
                "�".to_owned()
            } else {
                visible
            };
            let wrapped = (grapheme != "\n" && cells + visible.width() > budget) || cells == budget;
            if wrapped {
                rows.push(std::mem::take(&mut row));
                cells = 0;
            }
            positions.push((
                index,
                DraftCaret {
                    column: prefix + cells,
                    row: rows.len(),
                },
            ));
            if grapheme == "\n" {
                if !wrapped {
                    rows.push(std::mem::take(&mut row));
                }
                cells = 0;
            } else {
                cells += visible.width();
                row.push_str(&visible);
            }
        }
        if cells == budget {
            rows.push(std::mem::take(&mut row));
            cells = 0;
        }
        positions.push((
            text.len(),
            DraftCaret {
                column: prefix + cells,
                row: rows.len(),
            },
        ));
        rows.push(row);
        let caret = positions
            .iter()
            .rev()
            .find(|(index, _)| *index <= cursor)
            .map(|(_, caret)| *caret)
            .unwrap_or(DraftCaret {
                column: prefix,
                row: 0,
            });
        Self {
            rows,
            caret,
            positions,
            clipped_above: false,
            clipped_below: false,
        }
    }

    pub(crate) fn visible(mut self, max_rows: usize) -> Self {
        let max_rows = max_rows.max(1);
        let start = self
            .caret
            .row
            .saturating_sub(max_rows - 1)
            .min(self.rows.len().saturating_sub(max_rows));
        self.clipped_above = start > 0;
        self.clipped_below = start + max_rows < self.rows.len();
        self.rows = self.rows.into_iter().skip(start).take(max_rows).collect();
        self.caret.row = self.caret.row.saturating_sub(start);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(composer: &mut Composer, code: KeyCode, modifiers: KeyModifiers) {
        assert!(composer.edit_key(KeyEvent::new(code, modifiers), 40));
    }

    #[test]
    fn editing_in_the_middle_preserves_chinese_emoji_and_combining_marks() {
        let mut input = Composer::from("中文👩‍💻e\u{301}");
        key(&mut input, KeyCode::Left, KeyModifiers::NONE);
        key(&mut input, KeyCode::Backspace, KeyModifiers::NONE);
        input.insert("测试");
        assert_eq!(input.text(), "中文测试e\u{301}");
        key(&mut input, KeyCode::Delete, KeyModifiers::NONE);
        assert_eq!(input.text(), "中文测试");
        key(&mut input, KeyCode::Home, KeyModifiers::NONE);
        input.insert("先");
        assert_eq!(input.text(), "先中文测试");
    }

    #[test]
    fn history_returns_to_the_unsent_draft_and_is_only_in_memory() {
        let mut input = Composer::from("previous task");
        input.remember_submission();
        input.clear();
        input.insert("unfinished draft");
        key(&mut input, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(input.text(), "previous task");
        key(&mut input, KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(input.text(), "unfinished draft");
    }

    #[test]
    fn multiline_pastes_keep_their_text_and_control_sequences_are_literal() {
        let mut input = Composer::default();
        input.insert("first\r\n第二行\rthird\x1b[2J");
        assert_eq!(input.text(), "first\n第二行\nthird\\u{1b}[2J");
        assert!(!input.is_command_query());
        let frame = ComposerFrame::new(input.text(), input.cursor(), 40);
        assert_eq!(frame.rows.len(), 3);
        assert_eq!(frame.caret.row, 2);
    }

    #[test]
    fn wrapped_drafts_follow_the_caret_at_both_ends_in_small_viewports() {
        let input = "检查当前修改👩‍💻e\u{301} and explain everything";
        for width in [2, 9, 40] {
            for cursor in [0, input.len()] {
                let frame = ComposerFrame::new(input, cursor, width).visible(2);
                assert!(frame.rows.len() <= 2);
                assert!(frame.caret.row < frame.rows.len());
                assert!(frame.caret.column < width);
                assert!(frame
                    .rows
                    .iter()
                    .all(|row| row.width() + prompt_prefix(width).len() <= width));
            }
        }
    }

    #[test]
    fn combining_insertions_and_word_edits_leave_a_valid_grapheme_boundary() {
        let mut input = Composer::from("a 中文 tail");
        key(&mut input, KeyCode::Left, KeyModifiers::CONTROL);
        key(&mut input, KeyCode::Backspace, KeyModifiers::CONTROL);
        assert_eq!(input.text(), "a tail");
        key(&mut input, KeyCode::Home, KeyModifiers::NONE);
        key(&mut input, KeyCode::Right, KeyModifiers::NONE);
        input.insert("\u{301}");
        key(&mut input, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(input.text(), " tail");
    }
}
