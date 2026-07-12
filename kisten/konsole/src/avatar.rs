//! Static ANSI/ASCII portrait for the interactive startup screen.

use std::io::{self, Write};

pub const WIDTH: usize = 44;
pub const HEIGHT: usize = 24;

const RESET: &str = "\x1b[0m";

const ROWS: [&str; 24] = [
    "  ......                  .=* .   . ",
    "  ...    .... ---. .-*@%%+    .. .  ",
    "-+=- .. .=+*##+##@@@@##@%%%%#..    .",
    "=%%%%@@###*+****#####@%%@##@%# .. ..",
    " .@@%%@@%@#**+***########@%**%#      .",
    " =%@#@@@@###*#***********####@@@*=  ...",
    " *%%@@@*****+++++++*++*###@@%%@=-. .",
    " .=%@@@*+*++++======++=+*####@%@+  .",
    " @@###+++=+--++------=+=+**#*#@@=-  ..",
    "=%@#**==-=+--++=--=--===+**#@#@@+++-.",
    "... #@%*++==--+   -=------===*#####@%@@#=",
    " ..-#@%#+++--==   .     . -*#*++###**#@@%%%",
    " ++-*%++==+---=.  ..   .  -*+=-++- +###@@#@+",
    "--  #***=+- -+=+**-     . .  =  +##@@#- #+",
    " . +@##*=+---+ .--.     .   ++*#*=+%*+=*",
    "  *%+=#*=*+--   .         =*++*#+#%@ .--",
    ".#*- =#*+#*-= .           +*=**#%@*+  -",
    "-*.. -***#+=+*+=.         -**+=#*+-. ..--",
    "*    =+#+*@+-*=*###*+-     . ****###*= ...",
    "   . *+ *#+%#*#%%%%%%* .     #@@@%%%%%= . .",
    "-    *..**@%%%@@%%%*=   .+#@@@@###@@@*    ",
    "==-  =+%##@@%%%@@@@+  -*@%@@@@@@@@@@%* .",
    "    +@%%%#@%%@@%%%%*#%%@%%%%%%%%%@@@@@",
    "  -#%%@%%%@%%@@@@@@@@@@@@%@@@@@@@@#####@@+.",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Blush,
    Mid,
    Hair,
    Highlight,
}

impl Tone {
    fn ansi(self) -> &'static str {
        match self {
            Self::Blush => "\x1b[38;2;239;190;202m",
            Self::Mid => "\x1b[38;2;202;151;169m",
            Self::Hair => "\x1b[38;2;174;168;184m",
            Self::Highlight => "\x1b[38;2;226;219;230m",
        }
    }
}

fn tone(ch: char) -> Option<Tone> {
    match ch {
        '.' => Some(Tone::Blush),
        '-' | '+' | '=' => Some(Tone::Mid),
        '*' | '#' => Some(Tone::Hair),
        '%' | '@' => Some(Tone::Highlight),
        _ => None,
    }
}

pub fn render_row<W: Write>(out: &mut W, row: usize) -> io::Result<()> {
    render_row_width(out, row, WIDTH)
}

/// Render a row at a smaller terminal width by sampling the source columns.
/// The character palette remains unchanged, so scaling never turns the art
/// into a raster image or introduces a non-ASCII glyph.
pub fn render_row_width<W: Write>(
    out: &mut W,
    row: usize,
    requested_width: usize,
) -> io::Result<()> {
    let width = requested_width.min(WIDTH);
    if width == 0 {
        return Ok(());
    }
    let text = ROWS.get(row).copied().unwrap_or("");
    let source = text.chars().collect::<Vec<_>>();
    let source_width = source.len().max(1);
    let mut active = None;

    for column in 0..width {
        let source_column = column.saturating_mul(source_width) / width;
        let ch = source.get(source_column).copied().unwrap_or(' ');
        let next = tone(ch);
        if next != active {
            if active.is_some() {
                write!(out, "{RESET}")?;
            }
            if let Some(next) = next {
                write!(out, "{}", next.ansi())?;
            }
            active = next;
        }
        write!(out, "{ch}")?;
    }

    if active.is_some() {
        write!(out, "{RESET}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_is_bounded_ascii_text() {
        assert_eq!(ROWS.len(), HEIGHT);
        assert!(ROWS.iter().all(|row| row.is_ascii()));
        assert!(ROWS.iter().all(|row| row.chars().count() <= WIDTH));
        assert!(ROWS
            .iter()
            .flat_map(|row| row.chars())
            .all(|ch| ch == ' ' || ".-+=*#%@".contains(ch)));
    }

    #[test]
    fn rendered_row_is_true_colour_ascii_at_a_stable_width() {
        let mut out = Vec::new();
        render_row(&mut out, 8).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        assert!(rendered.contains("\x1b[38;2;"));
        assert_eq!(strip_ansi(&rendered).chars().count(), WIDTH);
        assert!(strip_ansi(&rendered).is_ascii());
        assert!(!rendered.contains('\u{2580}'));
    }

    #[test]
    fn scaled_row_keeps_a_stable_ascii_width() {
        let mut out = Vec::new();
        render_row_width(&mut out, 3, 24).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert_eq!(strip_ansi(&rendered).chars().count(), 24);
        assert!(rendered.contains("\x1b[38;2;"));
    }

    #[test]
    fn missing_rows_render_as_padding() {
        let mut out = Vec::new();
        render_row(&mut out, HEIGHT).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), " ".repeat(WIDTH));
    }

    fn strip_ansi(input: &str) -> String {
        let mut plain = String::new();
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' && matches!(chars.next(), Some('[')) {
                for end in chars.by_ref() {
                    if ('@'..='~').contains(&end) {
                        break;
                    }
                }
            } else {
                plain.push(ch);
            }
        }
        plain
    }
}
