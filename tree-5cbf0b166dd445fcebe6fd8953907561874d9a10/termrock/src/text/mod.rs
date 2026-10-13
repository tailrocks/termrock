//! Product-neutral terminal text measurement, sanitization, and windows.

pub use crate::ansi_text::{strip_bytes, styled_spans};
use unicode_width::UnicodeWidthStr;

/// True for any C0 / C1 control byte or DEL (`0x7f`).
#[must_use]
pub fn is_terminal_control_char(c: char) -> bool {
    let code = c as u32;
    code < 0x20 || c == '\x7f' || (0x80..0xa0).contains(&code)
}

/// Display-column width of `s`, excluding terminal control bytes.
#[must_use]
pub fn display_cols(s: &str) -> usize {
    if s.chars().any(is_terminal_control_char) {
        let sanitized: String = s
            .chars()
            .filter(|c| !is_terminal_control_char(*c))
            .collect();
        UnicodeWidthStr::width(sanitized.as_str())
    } else {
        UnicodeWidthStr::width(s)
    }
}

/// Take the longest prefix of `s` whose display width fits inside
/// `max_cols`, skipping control bytes.
#[must_use]
pub fn take_display_cols(s: &str, max_cols: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut out = String::new();
    let mut used = 0usize;
    for c in s.chars() {
        if is_terminal_control_char(c) {
            continue;
        }
        let width = c.width().unwrap_or(0);
        if used + width > max_cols {
            break;
        }
        out.push(c);
        used += width;
    }
    out
}

/// Substring of `s` covering display columns `[skip, skip + width)`,
/// skipping terminal control bytes and preserving only complete characters.
#[must_use]
pub fn display_cols_slice(s: &str, skip: usize, width: usize) -> String {
    let mut out = String::new();
    display_cols_slice_into(s, skip, width, &mut out);
    out
}

/// Writes the display-column window of `s` into a reusable buffer.
///
/// `out` is cleared first. Control bytes and partial wide characters are
/// omitted using the same rules as [`display_cols_slice`].
pub fn display_cols_slice_into(s: &str, skip: usize, width: usize, out: &mut String) {
    use unicode_width::UnicodeWidthChar;
    let mut col = 0usize;
    out.clear();
    for ch in s.chars() {
        if is_terminal_control_char(ch) {
            continue;
        }
        let w = ch.width().unwrap_or(0);
        if col >= skip && col + w <= skip + width {
            out.push(ch);
        }
        col += w;
        if col >= skip + width {
            break;
        }
    }
}

/// Leading ASCII-space count for text rows that need symmetric trailing
/// scroll padding. Controls are ignored.
#[must_use]
pub fn leading_space_cols<S>(parts: impl IntoIterator<Item = S>) -> usize
where
    S: AsRef<str>,
{
    let mut count = 0;
    for part in parts {
        for ch in part.as_ref().chars() {
            if is_terminal_control_char(ch) {
                continue;
            }
            if ch != ' ' {
                return count;
            }
            count += 1;
        }
    }
    count
}

/// Display-column width for a row plus matching trailing indentation padding.
#[must_use]
pub fn padded_line_display_cols<I, S>(parts: I) -> usize
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<str>,
{
    parts
        .clone()
        .into_iter()
        .map(|part| display_cols(part.as_ref()))
        .sum::<usize>()
        + leading_space_cols(parts)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A source-byte segment and its measured display-column placement.
pub struct FixedPrefixSegment {
    /// Inclusive UTF-8 byte offset in the source string.
    pub start_byte: usize,
    /// Exclusive UTF-8 byte offset in the source string.
    pub end_byte: usize,
    /// Zero-based output display column.
    pub target_col: usize,
    /// Width of the segment in terminal display columns.
    pub display_cols: usize,
}

/// Visible byte ranges for a horizontally scrolled line whose prefix remains
/// fixed while the suffix scrolls by display columns.
#[must_use]
pub fn fixed_prefix_scroll_segments(
    text: &str,
    base_col: usize,
    fixed_prefix_cols: usize,
    scroll_cols: usize,
    viewport_cols: usize,
) -> Vec<FixedPrefixSegment> {
    use unicode_width::UnicodeWidthChar;

    let prefix_cols = fixed_prefix_cols.min(viewport_cols);
    let suffix_cols = viewport_cols.saturating_sub(prefix_cols);
    let suffix_start = fixed_prefix_cols.saturating_add(scroll_cols);
    let suffix_end = suffix_start.saturating_add(suffix_cols);
    let mut segments: Vec<FixedPrefixSegment> = Vec::new();
    let mut col = base_col;

    for (start_byte, ch) in text.char_indices() {
        if is_terminal_control_char(ch) {
            continue;
        }
        let end_byte = start_byte + ch.len_utf8();
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            if let Some(last) = segments.last_mut()
                && last.end_byte == start_byte
            {
                last.end_byte = end_byte;
            }
            continue;
        }

        let target_col = if col < prefix_cols && col + width <= prefix_cols {
            col
        } else if col >= suffix_start && col + width <= suffix_end {
            prefix_cols + (col - suffix_start)
        } else {
            col += width;
            continue;
        };
        if target_col + width <= viewport_cols {
            segments.push(FixedPrefixSegment {
                start_byte,
                end_byte,
                target_col,
                display_cols: width,
            });
        }
        col += width;
    }

    segments
}

/// Collapse a terminal-window title to one printable line.
#[must_use]
pub fn sanitize_terminal_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut prev_space = true;
    for ch in title.chars() {
        if ch.is_control() || ch == '\u{7f}' || ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reusable_display_slice_matches_allocating_variant() {
        let text = "a\u{1b}界bc";
        let mut out = String::from("stale capacity");
        display_cols_slice_into(text, 1, 3, &mut out);
        assert_eq!(out, display_cols_slice(text, 1, 3));
        assert_eq!(out, "界b");
    }

    #[test]
    fn display_width_handles_wide_combining_control_and_empty_text() {
        for (text, width) in [
            ("ascii", 5),
            ("日本語", 6),
            ("🧪", 2),
            ("e\u{301}", 1),
            ("a\u{1b}b", 2),
            ("", 0),
        ] {
            assert_eq!(display_cols(text), width, "{text:?}");
        }
    }

    #[test]
    fn display_prefix_never_splits_wide_characters() {
        for (text, width, expected) in [
            ("abc", 2, "ab"),
            ("日本", 3, "日"),
            ("🧪x", 2, "🧪"),
            ("e\u{301}x", 1, "e\u{301}"),
            ("a\u{7f}b", 2, "ab"),
            ("", 4, ""),
        ] {
            let taken = take_display_cols(text, width);
            assert_eq!(taken, expected, "{text:?} at {width}");
            assert!(display_cols(&taken) <= width);
        }
    }

    #[test]
    fn display_slices_drop_partial_wide_characters() {
        for (text, skip, width, expected) in [
            ("abcdef", 2, 3, "cde"),
            ("日本", 1, 1, ""),
            ("日本", 0, 2, "日"),
            ("🧪x", 0, 0, ""),
            ("abc", 0, 20, "abc"),
            ("", 0, 4, ""),
        ] {
            let slice = display_cols_slice(text, skip, width);
            assert_eq!(slice, expected, "{text:?} [{skip}..{}]", skip + width);
            assert!(display_cols(&slice) <= width);
        }
    }

    #[test]
    fn control_boundaries_match_terminal_ranges() {
        for (ch, expected) in [
            ('\u{1f}', true),
            ('\u{20}', false),
            ('\u{7e}', false),
            ('\u{7f}', true),
            ('\u{80}', true),
            ('\u{9f}', true),
            ('\u{a0}', false),
        ] {
            assert_eq!(
                is_terminal_control_char(ch),
                expected,
                "U+{:04X}",
                ch as u32
            );
        }
    }

    #[test]
    fn terminal_titles_collapse_controls_and_whitespace() {
        assert_eq!(
            sanitize_terminal_title(" \u{1b}build\u{7}\n\tready\u{9b} "),
            "build ready"
        );
        assert_eq!(sanitize_terminal_title("\u{1b}\u{7}\n"), "");
    }

    #[test]
    fn indentation_measurement_matches_trailing_padding_contract() {
        assert_eq!(leading_space_cols(["  one", "two"]), 2);
        assert_eq!(leading_space_cols(["", "   "]), 3);
        assert_eq!(padded_line_display_cols(["  one"]), 7);
    }

    #[test]
    fn fixed_prefix_segments_cover_scroll_and_combining_boundaries() {
        let fit = fixed_prefix_scroll_segments("ab", 0, 1, 0, 2);
        assert_eq!(fit.len(), 2);
        assert_eq!((fit[0].target_col, fit[1].target_col), (0, 1));

        let past_end = fixed_prefix_scroll_segments("ab", 0, 1, 10, 2);
        assert_eq!(past_end.len(), 1);
        assert_eq!(past_end[0].start_byte, 0);

        let combining = fixed_prefix_scroll_segments("e\u{301}x", 0, 1, 0, 2);
        assert_eq!(combining[0].end_byte, "e\u{301}".len());
        assert_eq!(combining[1].target_col, 1);

        let no_prefix = fixed_prefix_scroll_segments("ab", 0, 0, 1, 1);
        assert_eq!(no_prefix.len(), 1);
        assert_eq!(no_prefix[0].target_col, 0);
    }
}
