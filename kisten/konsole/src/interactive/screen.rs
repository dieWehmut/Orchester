use std::io::{self, Write};

use crossterm::cursor;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, ClearType, DisableLineWrap, EnableLineWrap,
    EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
};

use super::CaretPosition;

pub(super) struct TerminalSession;

impl TerminalSession {
    pub(super) fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            DisableLineWrap,
            cursor::Hide
        ) {
            let _ = execute!(
                io::stdout(),
                cursor::Show,
                DisableBracketedPaste,
                EnableLineWrap,
                LeaveAlternateScreen
            );
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            cursor::SetCursorStyle::DefaultUserShape,
            cursor::Show,
            DisableBracketedPaste,
            EnableLineWrap,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Default)]
pub(super) struct FramePresenter {
    rows: Vec<Vec<u8>>,
    caret_visible: bool,
    viewport: Option<(usize, usize)>,
}

impl FramePresenter {
    pub(super) fn invalidate(&mut self) {
        self.rows.clear();
    }

    pub(super) fn set_viewport(&mut self, width: usize, height: usize) {
        if self.viewport != Some((width, height)) {
            // ConHost reflows and scrolls existing rows during a resize. Old
            // row positions cannot be reused; clearing old rows beyond the
            // new height would also erase the last visible row repeatedly.
            self.invalidate();
            self.viewport = Some((width, height));
        }
    }

    pub(super) fn present<W: Write>(&mut self, out: &mut W, frame: &[u8]) -> io::Result<()> {
        let rows = frame_rows(frame);
        let row_count = self.rows.len().max(rows.len());
        if row_count > usize::from(u16::MAX) + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "terminal frame has too many rows",
            ));
        }

        execute!(out, BeginSynchronizedUpdate)?;
        let mut update_result = Ok(());
        for row in 0..row_count {
            if self.rows.get(row) == rows.get(row) {
                continue;
            }
            if let Err(error) = execute!(
                out,
                cursor::MoveTo(0, row as u16),
                terminal::Clear(ClearType::CurrentLine)
            ) {
                update_result = Err(error);
                break;
            }
            if let Some(content) = rows.get(row) {
                if let Err(error) = out.write_all(content) {
                    update_result = Err(error);
                    break;
                }
            }
        }
        let end_result = execute!(out, cursor::MoveTo(0, 0), EndSynchronizedUpdate);
        update_result.and(end_result)?;
        out.flush()?;
        self.rows = rows;
        Ok(())
    }

    /// Parks the terminal cursor on the composer's insertion point. The frame
    /// never draws a caret glyph: a real cursor is what blinks, moves with the
    /// text, and lands after wide characters.
    ///
    /// The shape is set to a blinking bar while the composer owns the input, so
    /// the composer reads as a text field rather than a block cursor sitting on
    /// a menu. Terminals that do not implement `DECSCUSR` ignore it, and the
    /// shape is only re-sent when it changes: this runs on every frame.
    pub(super) fn place_caret<W: Write>(
        &mut self,
        out: &mut W,
        caret: Option<CaretPosition>,
    ) -> io::Result<()> {
        match caret {
            Some(caret) => {
                execute!(out, cursor::MoveTo(caret.column, caret.row))?;
                if !self.caret_visible {
                    execute!(out, cursor::SetCursorStyle::BlinkingBar, cursor::Show)?;
                    self.caret_visible = true;
                }
            }
            None if self.caret_visible => {
                execute!(out, cursor::SetCursorStyle::DefaultUserShape, cursor::Hide)?;
                self.caret_visible = false;
            }
            None => {}
        }
        out.flush()
    }
}

fn frame_rows(frame: &[u8]) -> Vec<Vec<u8>> {
    let frame = frame.strip_suffix(b"\n").unwrap_or(frame);
    if frame.is_empty() {
        Vec::new()
    } else {
        frame
            .split(|byte| *byte == b'\n')
            .map(<[u8]>::to_vec)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_rows_are_not_repainted() {
        let mut presenter = FramePresenter::default();
        let mut out = Vec::new();
        presenter.present(&mut out, b"alpha\nbeta\n").unwrap();
        out.clear();

        presenter.present(&mut out, b"alpha\ngamma\n").unwrap();

        let update = String::from_utf8(out).unwrap();
        assert!(!update.contains("alpha"));
        assert!(!update.contains("beta"));
        assert!(update.contains("gamma"));
        assert!(!update.contains("\x1b[J"));
        assert!(!update.contains("\x1b[2J"));
    }

    #[test]
    fn the_caret_is_shown_on_the_composer_and_hidden_without_one() {
        let mut presenter = FramePresenter::default();
        let mut out = Vec::new();
        presenter.present(&mut out, b"alpha\nbeta\n").unwrap();
        out.clear();

        presenter
            .place_caret(&mut out, Some(CaretPosition { column: 7, row: 1 }))
            .unwrap();

        let update = String::from_utf8_lossy(&out).into_owned();
        assert!(
            update.contains("\x1b[2;8H"),
            "the cursor must move to the insertion point: {update:?}"
        );
        assert!(
            update.contains("\x1b[?25h"),
            "the cursor must be visible while typing: {update:?}"
        );
        assert!(
            update.contains("\x1b[5 q"),
            "the composer caret must be an insertion bar: {update:?}"
        );

        // This runs on every frame, so the shape is only re-sent when it
        // changes; the position still moves.
        out.clear();
        presenter
            .place_caret(&mut out, Some(CaretPosition { column: 9, row: 1 }))
            .unwrap();
        let update = String::from_utf8_lossy(&out).into_owned();
        assert!(
            !update.contains("\x1b[5 q"),
            "the caret shape is not re-sent per frame: {update:?}"
        );
        assert!(
            update.contains("\x1b[2;10H"),
            "the caret still follows the text: {update:?}"
        );

        out.clear();
        presenter.place_caret(&mut out, None).unwrap();
        let update = String::from_utf8_lossy(&out).into_owned();
        assert!(
            update.contains("\x1b[?25l"),
            "an overlay or help view must hide the caret: {update:?}"
        );
        assert!(
            update.contains("\x1b[0 q"),
            "hiding the caret must restore the default shape: {update:?}"
        );
    }

    #[test]
    fn shorter_frames_clear_each_stale_row() {
        let mut presenter = FramePresenter::default();
        let mut out = Vec::new();
        presenter
            .present(&mut out, b"stable\nstale-one\nstale-two\n")
            .unwrap();
        out.clear();

        presenter.present(&mut out, b"stable\n").unwrap();

        let update = String::from_utf8(out).unwrap();
        assert_eq!(update.matches("\x1b[2K").count(), 2);
        assert!(!update.contains("stable"));
        assert!(!update.contains("stale-one"));
        assert!(!update.contains("stale-two"));
    }

    #[test]
    fn resized_viewports_repaint_the_header_without_touching_rows_below_the_screen() {
        let mut presenter = FramePresenter::default();
        let mut out = Vec::new();
        presenter.set_viewport(79, 24);
        presenter
            .present(&mut out, b"header\none\ntwo\nthree\nfour\nfive\n")
            .unwrap();
        out.clear();

        presenter.set_viewport(39, 3);
        presenter
            .present(&mut out, b"header\nbody\nstatus\n")
            .unwrap();
        let update = String::from_utf8(out).unwrap();
        assert!(update.contains("header"));
        assert!(update.contains("status"));
        assert_eq!(update.matches("\x1b[2K").count(), 3);
        assert!(!update.contains("\x1b[4;1H"));
        assert!(!update.contains("\x1b[2J"));
    }
}
