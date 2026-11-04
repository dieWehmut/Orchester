//! Portable terminal rendering of the user's supplied Orchester portrait.
use std::io::{self, Write};

const SOURCE_SIZE: usize = 128;
const PIXELS: &[u8; SOURCE_SIZE * SOURCE_SIZE] = include_bytes!("assets/orchester.gray");

pub(super) fn render(out: &mut impl Write, width: usize, rows: usize) -> io::Result<()> {
    // Use about half the body height while leaving room around the full image.
    if width < 30 || !(8..=10_000).contains(&rows) {
        return Ok(());
    }
    let size = 64.min(width / 3).min(rows).max(8) & !1;
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
            write!(out, "\x1b[38;2;{a};{a};{a}m\x1b[48;2;{b};{b};{b}m▀")?;
        }
        writeln!(out, "\x1b[0m")?;
    }
    Ok(())
}

fn pixel(x: usize, y: usize, size: usize) -> u8 {
    // Average each source region instead of dropping pixels during reduction.
    // This retains the original composition and smooth tonal detail at any size.
    let (left, right) = (x * SOURCE_SIZE / size, (x + 1) * SOURCE_SIZE / size);
    let (top, bottom) = (y * SOURCE_SIZE / size, (y + 1) * SOURCE_SIZE / size);
    let mut total = 0u32;
    for row in top..bottom {
        for col in left..right {
            total += u32::from(PIXELS[row * SOURCE_SIZE + col]);
        }
    }
    (total / ((right - left) * (bottom - top)) as u32) as u8
}
