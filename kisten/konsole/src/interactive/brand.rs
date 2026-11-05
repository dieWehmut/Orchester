//! Portable terminal rendering of the user's supplied Orchester portrait.
use std::io::{self, Write};

const SOURCE_SIZE: usize = 256;
const PIXELS: &[u8; SOURCE_SIZE * SOURCE_SIZE] = include_bytes!("assets/orchester.gray");
const ALPHA: &[u8; SOURCE_SIZE * SOURCE_SIZE] = include_bytes!("assets/orchester.alpha");

pub(super) fn render(out: &mut impl Write, width: usize, rows: usize) -> io::Result<()> {
    // Fill most of the body with the original image, leaving a small margin.
    if width < 30 || !(8..=10_000).contains(&rows) {
        return Ok(());
    }
    let size = 128
        .min(width * 2 / 3)
        .min(rows.saturating_sub(2) * 2)
        .max(8)
        & !1;
    let top = (rows - size / 2) / 2;
    let left = " ".repeat((width - size) / 2);
    for _ in 0..top {
        writeln!(out)?;
    }
    for row in (0..size).step_by(2) {
        write!(out, "{left}")?;
        for col in 0..size {
            let a = pixel(col, row, size);
            let b = pixel(col, row + 1, size);
            match (a, b) {
                (Some(a), Some(b)) => {
                    write!(out, "\x1b[38;2;{a};{a};{a}m\x1b[48;2;{b};{b};{b}m▀")?;
                }
                (Some(a), None) => write!(out, "\x1b[49m\x1b[38;2;{a};{a};{a}m▀")?,
                (None, Some(b)) => write!(out, "\x1b[49m\x1b[38;2;{b};{b};{b}m▄")?,
                (None, None) => write!(out, "\x1b[0m ")?,
            }
        }
        writeln!(out, "\x1b[0m")?;
    }
    Ok(())
}

fn pixel(x: usize, y: usize, size: usize) -> Option<u8> {
    // Weight by the extracted matte so the original backdrop never contributes
    // to the portrait. Transparent halves use the terminal's own background.
    let (left, right) = (x * SOURCE_SIZE / size, (x + 1) * SOURCE_SIZE / size);
    let (top, bottom) = (y * SOURCE_SIZE / size, (y + 1) * SOURCE_SIZE / size);
    let mut total = 0u32;
    let mut coverage = 0u32;
    for row in top..bottom {
        for col in left..right {
            let index = row * SOURCE_SIZE + col;
            let alpha = u32::from(ALPHA[index]);
            total += u32::from(PIXELS[index]) * alpha;
            coverage += alpha;
        }
    }
    let samples = ((right - left) * (bottom - top)) as u32;
    if coverage <= samples * 48 {
        return None;
    }
    Some((24 + (total / coverage) * 120 / 255) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matte_keeps_the_face_and_removes_the_backdrop_at_every_size() {
        for size in [8, 20, 62, 96, 128] {
            assert_eq!(pixel(0, 0, size), None);
            assert_eq!(pixel(size - 1, 0, size), None);
            assert_eq!(pixel(0, size / 2, size), None);
            assert!(pixel(size / 2, size / 2, size).is_some());
            assert!(pixel(size / 2, size - 1, size).is_some());
            for y in 0..size {
                for x in 0..size {
                    assert!(pixel(x, y, size).map_or(true, |gray| (24..=144).contains(&gray)));
                }
            }
        }
    }
}
