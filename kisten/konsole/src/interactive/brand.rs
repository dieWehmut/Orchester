//! Portable terminal rendering of the user's supplied Orchester portrait.
use std::io::{self, Write};

const PIXELS: &[u8; 64 * 64 * 3] = include_bytes!("assets/orchester.rgb");

pub(super) fn render(out: &mut impl Write, width: usize, rows: usize) -> io::Result<()> {
    // Keep the portrait quiet and centered; tiny viewports prioritize controls.
    if width < 30 || !(8..=10_000).contains(&rows) {
        return Ok(());
    }
    let size = 32.min(width / 6).min(rows * 3 / 5).max(8) & !1;
    let top = (rows - size / 2) / 2;
    let left = " ".repeat((width - size) / 2);
    for _ in 0..top {
        writeln!(out)?;
    }
    for row in (0..size).step_by(2) {
        write!(out, "{left}")?;
        for col in 0..size {
            let pixel = |y| {
                let i = ((y * 64 / size) * 64 + col * 64 / size) * 3;
                &PIXELS[i..i + 3]
            };
            let a = pixel(row);
            let b = pixel(row + 1);
            write!(
                out,
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                a[0], a[1], a[2], b[0], b[1], b[2]
            )?;
        }
        writeln!(out, "\x1b[0m")?;
    }
    Ok(())
}
