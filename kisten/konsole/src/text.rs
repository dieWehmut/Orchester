//! Display-cell wrapping shared by the transcript and composer.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) fn wrap_line(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        let grapheme_cells = grapheme.width();
        if cells > 0 && cells + grapheme_cells > width {
            rows.push(std::mem::take(&mut row));
            cells = 0;
        }
        // A one-column viewport cannot display a wide grapheme. Keep the
        // frame bounded and show a replacement rather than spilling a row.
        if grapheme_cells > width {
            row.push('\u{fffd}');
            cells += 1;
        } else {
            row.push_str(grapheme);
            cells += grapheme_cells;
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_responses_preserve_text_and_wide_graphemes() {
        let text = "检查当前修改 e\u{301} 👩‍💻 and explain the entire response";
        for width in [2, 7, 19, 80] {
            let rows = wrap_line(text, width);
            assert_eq!(rows.concat(), text);
            assert!(rows.iter().all(|row| row.width() <= width));
            assert!(!rows.iter().any(|row| row.starts_with('\u{301}')));
            assert!(rows.iter().any(|row| row.contains("👩‍💻")));
        }
    }

    #[test]
    fn a_one_column_viewport_and_empty_text_are_bounded() {
        assert_eq!(wrap_line("中文", 1), vec!["�", "�"]);
        assert_eq!(wrap_line("", 20), vec![""]);
    }
}
