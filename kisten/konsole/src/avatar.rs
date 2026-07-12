//! Static terminal portrait used by the Orchester welcome screen.
//!
//! The portrait is intentionally checked-in character art rather than a runtime
//! image asset.  It is a compact, hand-tuned reduction of
//! `sample/picture/icon.png`: cat ears, red hair ribbons, and dark blue-black
//! hair frame a pale face with cyan eyes, pink cheeks, and a dark hoodie.
//! Keeping only ASCII glyphs makes the welcome screen safe on terminals that
//! do not support Unicode or image protocols.

use std::io::{self, Write};

/// Width of every row in [`AVATAR_ROWS`].
pub const AVATAR_WIDTH: usize = 36;

/// Number of rows in [`AVATAR_ROWS`].
pub const AVATAR_HEIGHT: usize = 20;

/// ASCII portrait rows.  Spaces are intentionally uncoloured so the portrait
/// can sit on either a dark or light terminal background.
pub const AVATAR_ROWS: &[&str] = &[
    r"      ,*&&&&*,        ,*&&&&*,      ",
    r"     /*&&&&&&*\______/*&&&&&&*\     ",
    r"    /#\/\~~~~~~~~~~~~~~~~~~/\/#\    ",
    r"   /####@@@############@@@####\     ",
    r"  /###@@@################@@@###\    ",
    r"  /##@@@######@@@@@@######@@@##\    ",
    r"  ##@@@####@@@######@@@####@@@##    ",
    r" ##@@@###@@  ........  @@###@@@##   ",
    r" ##@@###@  .  o    o  .  @###@@##   ",
    r" ##@@##@ .  oOO  OO o  . @##@@@##   ",
    r" ##@@## .      --      . ##@@@##    ",
    r" ##@@@  .   .-____-.   .  @@@@##    ",
    r"   ##@@ .  /  ~~~~~~  \  . @@##     ",
    r"  ##@@@ .(  ~~~~~~~~  ). .@@@##     ",
    r"   ##@@@ .\__________/ . @@@##      ",
    r"    ##@@@################@@@##      ",
    r"      ##@@##################        ",
    r"       ##@@##############@@         ",
    r"      /####################\        ",
    r"     /######################\       ",
];

// 256-colour ANSI palette — matched to the reference icon at sample/picture/icon.png.
// Hair: dark blue-black (visible on both light and dark terminals).
// Ribbons: bright red with a darker variant for depth.
// Skin / eyes / blush: kept at readable terminal contrast.
const RIBBON: &str = "\x1b[38;5;196m";
const RIBBON_DARK: &str = "\x1b[38;5;124m";
const HAIR: &str = "\x1b[38;5;60m";
const HAIR_DARK: &str = "\x1b[38;5;235m";
const HAIR_HIGHLIGHT: &str = "\x1b[38;5;103m";
const SKIN: &str = "\x1b[38;5;223m";
const SKIN_SHADE: &str = "\x1b[38;5;217m";
const IRIS: &str = "\x1b[38;5;51m";
const IRIS_DARK: &str = "\x1b[38;5;24m";
const BLUSH: &str = "\x1b[38;5;211m";
const RESET: &str = "\x1b[0m";

/// Return the semantic colour for one portrait glyph.
pub fn color_for(glyph: char) -> &'static str {
    match glyph {
        '*' => RIBBON,
        '&' => RIBBON_DARK,
        'o' => IRIS,
        'O' => IRIS_DARK,
        '~' => BLUSH,
        '.' => SKIN,
        '-' | '_' | '(' | ')' => SKIN_SHADE,
        '@' => HAIR_DARK,
        '#' | '/' | '\\' => HAIR,
        ',' => RIBBON,
        _ => HAIR_HIGHLIGHT,
    }
}

/// Write one row with compact ANSI colour transitions.
pub fn write_line<W: Write>(out: &mut W, line: &str) -> io::Result<()> {
    let mut active: Option<&'static str> = None;
    for glyph in line.chars() {
        if glyph == ' ' {
            if active.is_some() {
                write!(out, "{RESET}")?;
                active = None;
            }
            out.write_all(b" ")?;
            continue;
        }

        let color = color_for(glyph);
        if active != Some(color) {
            if active.is_some() {
                write!(out, "{RESET}")?;
            }
            write!(out, "{color}")?;
            active = Some(color);
        }
        write!(out, "{glyph}")?;
    }
    if active.is_some() {
        write!(out, "{RESET}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(input: &str) -> String {
        let mut plain = String::new();
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                if matches!(chars.next(), Some('[')) {
                    for end in chars.by_ref() {
                        if ('@'..='~').contains(&end) {
                            break;
                        }
                    }
                }
            } else {
                plain.push(ch);
            }
        }
        plain
    }

    #[test]
    fn rows_are_fixed_width_ascii() {
        assert_eq!(AVATAR_ROWS.len(), AVATAR_HEIGHT);
        for row in AVATAR_ROWS {
            assert_eq!(row.len(), AVATAR_WIDTH);
            assert!(row.is_ascii());
        }
    }

    #[test]
    fn portrait_contains_source_features() {
        let portrait = AVATAR_ROWS.join("\n");
        assert!(portrait.contains("/#\\"), "missing cat-ear outline");
        assert!(portrait.contains('o'), "missing iris glyph");
        assert!(portrait.contains('~'), "missing blush glyph");
        assert!(portrait.contains('&'), "missing red ribbon glyph");
        assert!(portrait.contains('*'), "missing bright ribbon glyph");
    }

    #[test]
    fn renderer_preserves_row_width_and_emits_colour() {
        let mut rendered = Vec::new();
        write_line(&mut rendered, AVATAR_ROWS[9]).unwrap();
        let rendered = String::from_utf8(rendered).unwrap();
        assert!(rendered.contains("\x1b["));
        assert_eq!(strip_ansi(&rendered), AVATAR_ROWS[9]);
    }

    #[test]
    fn spaces_are_not_wrapped_in_colour_sequences() {
        let mut rendered = Vec::new();
        write_line(&mut rendered, "  ##  ").unwrap();
        let rendered = String::from_utf8(rendered).unwrap();
        assert_eq!(strip_ansi(&rendered), "  ##  ");
        assert!(rendered.starts_with("  "));
        assert!(rendered.ends_with("  "));
    }
}
