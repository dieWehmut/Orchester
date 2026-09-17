use std::io::{self, Write};

use crossterm::cursor;
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
            DisableLineWrap,
            cursor::Hide
        ) {
            let _ = execute!(
                io::stdout(),
                cursor::Show,
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
            cursor::Show,
            EnableLineWrap,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Default)]
pub(super) struct FramePresenter {
    rows: Vec<Vec<u8>>,
}

impl FramePresenter {
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
    pub(super) fn place_caret<W: Write>(
        &self,
        out: &mut W,
        caret: Option<CaretPosition>,
    ) -> io::Result<()> {
        match caret {
            Some(caret) => execute!(out, cursor::MoveTo(caret.column, caret.row), cursor::Show)?,
            None => execute!(out, cursor::Hide)?,
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

        out.clear();
        presenter.place_caret(&mut out, None).unwrap();
        let update = String::from_utf8_lossy(&out).into_owned();
        assert!(
            update.contains("\x1b[?25l"),
            "an overlay or help view must hide the caret: {update:?}"
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
}
