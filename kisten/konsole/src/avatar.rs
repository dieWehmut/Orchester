//! Embedded ANSI portrait for the interactive startup screen.

use std::io::{self, Write};
use std::sync::OnceLock;

pub const WIDTH: usize = 44;
pub const HEIGHT: usize = 20;

const RESET: &str = "\x1b[0m";
const LOGO_SOURCE: &[u8] = include_bytes!("../../../sample/logo/logo.txt");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    foreground: Option<Rgb>,
    background: Option<Rgb>,
    reverse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    character: char,
    style: Style,
}

#[derive(Debug)]
struct Logo {
    rows: Vec<Vec<Cell>>,
    width: usize,
}

static LOGO: OnceLock<Logo> = OnceLock::new();

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

    if row >= HEIGHT {
        return write!(out, "{}", " ".repeat(width));
    }

    let logo = LOGO.get_or_init(parse_logo);
    let source_row = sample_index(row, HEIGHT, logo.rows.len());
    let source = logo.rows.get(source_row);
    let source_width = logo.width.max(1);
    let mut active = Style::default();

    for column in 0..width {
        let source_column = sample_index(column, width, source_width);
        let cell = source
            .and_then(|source| source.get(source_column))
            .copied()
            .unwrap_or(Cell {
                character: ' ',
                style: Style::default(),
            });
        if cell.style != active {
            write!(out, "{RESET}")?;
            write_style(out, cell.style)?;
            active = cell.style;
        }
        write!(out, "{}", cell.character)?;
    }

    if active != Style::default() {
        write!(out, "{RESET}")?;
    }
    Ok(())
}

fn sample_index(index: usize, output_len: usize, source_len: usize) -> usize {
    if output_len <= 1 || source_len <= 1 {
        return 0;
    }
    index.saturating_mul(source_len - 1) / (output_len - 1)
}

fn parse_logo() -> Logo {
    let source = decode_source();
    let mut rows = vec![Vec::new()];
    let mut style = Style::default();
    let mut chars = source.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '\u{feff}' | '\r' => {}
            '\n' => {
                trim_plain_padding(rows.last_mut().expect("logo row exists"));
                rows.push(Vec::new());
                style = Style::default();
            }
            '\x1b' if matches!(chars.peek(), Some('[')) => {
                chars.next();
                let mut parameters = String::new();
                let mut command = None;
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        command = Some(next);
                        break;
                    }
                    parameters.push(next);
                }
                if command == Some('m') {
                    apply_sgr(&mut style, &parameters);
                }
            }
            // The source was saved after a code-page conversion of U+2580.
            '\u{923b}' if matches!(chars.peek(), Some('\u{20ac}')) => {
                chars.next();
                rows.last_mut().expect("logo row exists").push(Cell {
                    character: '\u{2580}',
                    style,
                });
            }
            value if !value.is_control() => {
                rows.last_mut().expect("logo row exists").push(Cell {
                    character: value,
                    style,
                });
            }
            _ => {}
        }
    }

    while rows.last().is_some_and(Vec::is_empty) {
        rows.pop();
    }
    // `chafa --size 66x30` is the source geometry.  Keep its right margin
    // while downsampling so the portrait does not become horizontally wide.
    let width = rows.iter().map(Vec::len).max().unwrap_or(66).max(66);
    Logo { rows, width }
}

fn decode_source() -> String {
    if LOGO_SOURCE.starts_with(&[0xff, 0xfe]) {
        let units = LOGO_SOURCE[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(LOGO_SOURCE).into_owned()
}

fn trim_plain_padding(row: &mut Vec<Cell>) {
    while row.last().is_some_and(|cell| {
        cell.character == ' ' && cell.style == Style::default()
    }) {
        row.pop();
    }
}

fn apply_sgr(style: &mut Style, parameters: &str) {
    let values = if parameters.is_empty() {
        vec![0]
    } else {
        parameters
            .split(';')
            .filter_map(|value| value.parse::<u16>().ok())
            .collect::<Vec<_>>()
    };
    let mut index = 0;
    while index < values.len() {
        match values[index] {
            0 => *style = Style::default(),
            7 => style.reverse = true,
            27 => style.reverse = false,
            39 => style.foreground = None,
            49 => style.background = None,
            channel @ (38 | 48)
                if values.get(index + 1) == Some(&2) && index + 4 < values.len() =>
            {
                let colour = Rgb(
                    values[index + 2].min(255) as u8,
                    values[index + 3].min(255) as u8,
                    values[index + 4].min(255) as u8,
                );
                if channel == 38 {
                    style.foreground = Some(colour);
                } else {
                    style.background = Some(colour);
                }
                index += 4;
            }
            _ => {}
        }
        index += 1;
    }
}

fn write_style<W: Write>(out: &mut W, style: Style) -> io::Result<()> {
    if style.reverse {
        write!(out, "\x1b[7m")?;
    }
    if let Some(Rgb(red, green, blue)) = style.foreground {
        write!(out, "\x1b[38;2;{red};{green};{blue}m")?;
    }
    if let Some(Rgb(red, green, blue)) = style.background {
        write!(out, "\x1b[48;2;{red};{green};{blue}m")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_source_matches_chafa_geometry_and_decodes_half_blocks() {
        let logo = LOGO.get_or_init(parse_logo);
        assert_eq!(logo.rows.len(), 30);
        assert_eq!(logo.width, 66);
        assert!(logo
            .rows
            .iter()
            .flat_map(|row| row.iter())
            .any(|cell| cell.character == '\u{2580}'));
        assert!(logo
            .rows
            .iter()
            .flat_map(|row| row.iter())
            .all(|cell| cell.character == ' ' || cell.character == '\u{2580}'));
    }

    #[test]
    fn rendered_row_uses_the_embedded_logo_asset_at_a_stable_width() {
        let mut out = Vec::new();
        render_row(&mut out, 8).unwrap();
        let rendered = String::from_utf8(out).unwrap();

        assert!(rendered.contains("\x1b[38;2;"));
        assert!(rendered.contains("\x1b[48;2;"));
        assert_eq!(strip_ansi(&rendered).chars().count(), WIDTH);
        assert!(strip_ansi(&rendered).contains('\u{2580}'));
        assert!(!rendered.contains("\u{923b}\u{20ac}"));
    }

    #[test]
    fn scaled_row_keeps_a_stable_logo_width() {
        let mut out = Vec::new();
        render_row_width(&mut out, 3, 24).unwrap();
        let rendered = String::from_utf8(out).unwrap();
        assert_eq!(strip_ansi(&rendered).chars().count(), 24);
        assert!(rendered.contains("\x1b[38;2;"));
        assert!(strip_ansi(&rendered).contains('\u{2580}'));
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
